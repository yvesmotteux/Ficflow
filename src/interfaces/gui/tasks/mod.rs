//! Background-task plumbing for AO3 fetches that would otherwise block the
//! UI thread (which `eframe` runs on). The worker thread owns its own
//! `rusqlite::Connection` (Connection is `!Send`) and `Ao3Fetcher`; the GUI
//! sees task progress through a shared `Vec<TaskState>` behind a Mutex.

pub mod worker;

use std::mem;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Add,
}

#[derive(Clone, Debug)]
pub enum TaskStatus {
    Running,
    Done,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct TaskState {
    pub id: u64,
    pub kind: TaskKind,
    /// Original user input — kept so "Retry" can re-enqueue the same request.
    pub input: String,
    /// What the user sees in the Tasks view: starts as the input, becomes the
    /// fic title once an add succeeds.
    pub display: String,
    pub status: TaskStatus,
    pub started_at: DateTime<Utc>,
}

pub(super) enum WorkerCommand {
    AddFic { task_id: u64, input: String },
}

pub(super) struct WorkerInbox {
    pub tasks: Mutex<Vec<TaskState>>,
    /// Titles of fics that were just successfully added. The GUI drains this
    /// each frame to (a) reload its `fics` cache and (b) toast a confirmation
    /// per added fic.
    pub recent_completions: Mutex<Vec<String>>,
}

impl WorkerInbox {
    fn new() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            recent_completions: Mutex::new(Vec::new()),
        }
    }
}

pub struct TaskExecutor {
    inbox: Arc<WorkerInbox>,
    sender: Sender<WorkerCommand>,
    next_id: AtomicU64,
    _worker: thread::JoinHandle<()>,
}

impl TaskExecutor {
    pub fn spawn(urls: Vec<String>, max_cycles: u32) -> Self {
        let inbox = Arc::new(WorkerInbox::new());
        let (tx, rx) = mpsc::channel();
        let worker_inbox = Arc::clone(&inbox);
        let worker = thread::Builder::new()
            .name("ficflow-tasks".into())
            .spawn(move || worker::run(rx, worker_inbox, urls, max_cycles))
            .expect("failed to spawn task-worker thread");
        Self {
            inbox,
            sender: tx,
            next_id: AtomicU64::new(1),
            _worker: worker,
        }
    }

    pub fn enqueue_add(&self, input: String) {
        let task_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let task = TaskState {
            id: task_id,
            kind: TaskKind::Add,
            input: input.clone(),
            display: input.clone(),
            status: TaskStatus::Running,
            started_at: Utc::now(),
        };
        self.inbox.tasks.lock().unwrap().push(task);
        // SendError only happens if the worker is dead; in that case the app
        // is already shutting down so swallowing is safe.
        let _ = self.sender.send(WorkerCommand::AddFic { task_id, input });
    }

    pub fn snapshot(&self) -> Vec<TaskState> {
        self.inbox.tasks.lock().unwrap().clone()
    }

    pub fn clear_completed(&self) {
        self.inbox
            .tasks
            .lock()
            .unwrap()
            .retain(|t| matches!(t.status, TaskStatus::Running));
    }

    /// Re-runs a failed task using its original input, dropping the failed
    /// entry. New entry gets a fresh task id.
    pub fn retry(&self, task_id: u64) {
        let input = {
            let mut tasks = self.inbox.tasks.lock().unwrap();
            let pos = tasks.iter().position(|t| t.id == task_id);
            pos.and_then(|i| {
                if matches!(tasks[i].status, TaskStatus::Failed(_)) {
                    Some(tasks.remove(i).input)
                } else {
                    None
                }
            })
        };
        if let Some(input) = input {
            self.enqueue_add(input);
        }
    }

    /// Drains the queue of titles for fics added since the last call.
    /// Caller should reload its `fics` cache when this returns a non-empty
    /// vec, and may surface one toast per title.
    pub fn take_completions(&self) -> Vec<String> {
        mem::take(&mut *self.inbox.recent_completions.lock().unwrap())
    }

    pub fn has_running(&self) -> bool {
        self.inbox
            .tasks
            .lock()
            .unwrap()
            .iter()
            .any(|t| matches!(t.status, TaskStatus::Running))
    }
}
