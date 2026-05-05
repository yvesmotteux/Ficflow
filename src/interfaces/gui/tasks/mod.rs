//! Background-task plumbing for AO3 fetches that would otherwise block the
//! UI thread (which `eframe` runs on). The worker thread owns its own
//! `rusqlite::Connection` (Connection is `!Send`) and `Ao3Fetcher`; the GUI
//! sees task progress through a shared `Vec<TaskState>` behind a Mutex.

pub mod worker;

use std::mem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Add,
    Refresh,
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
    RefreshFic { task_id: u64, fic_id: u64 },
}

pub(super) struct WorkerInbox {
    pub tasks: Mutex<Vec<TaskState>>,
    /// Titles of fics that were just successfully added. The GUI drains this
    /// each frame to (a) reload its `fics` cache and (b) toast a confirmation
    /// per added fic.
    pub recent_completions: Mutex<Vec<String>>,
    /// IDs of fics that were just successfully refreshed. The GUI drains
    /// this to reload the in-memory cache so the new metadata + bumped
    /// `last_checked_date` show in the details panel.
    pub recent_refreshes: Mutex<Vec<u64>>,
}

impl WorkerInbox {
    fn new() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            recent_completions: Mutex::new(Vec::new()),
            recent_refreshes: Mutex::new(Vec::new()),
        }
    }
}

pub struct TaskExecutor {
    inbox: Arc<WorkerInbox>,
    sender: Sender<WorkerCommand>,
    next_id: AtomicU64,
    /// Keeps the worker thread alive for the lifetime of the
    /// `TaskExecutor` (the leading underscore signals "we never read
    /// it" — but it's load-bearing: dropping the handle would not stop
    /// the thread, however the thread itself terminates when `sender`
    /// is dropped and `rx.recv()` returns `Err`, so holding the handle
    /// here is mostly to give the OS a name to attach to in debuggers).
    /// No graceful shutdown — in-flight HTTP requests get torn down by
    /// the OS when the process exits.
    _worker: thread::JoinHandle<()>,
}

impl TaskExecutor {
    /// Spawns the worker thread. `db_path` matches the GUI's connection
    /// override (see `FicflowConfig::db_path`) — the worker opens its
    /// OWN connection (Connection isn't `Send`) but it must point at
    /// the same SQLite file so writes from the worker show up in the
    /// GUI's reads. `None` falls through to `establish_connection()`.
    pub fn spawn(urls: Vec<String>, max_cycles: u32, db_path: Option<PathBuf>) -> Self {
        let inbox = Arc::new(WorkerInbox::new());
        let (tx, rx) = mpsc::channel();
        let worker_inbox = Arc::clone(&inbox);
        let worker = thread::Builder::new()
            .name("ficflow-tasks".into())
            .spawn(move || worker::run(rx, worker_inbox, urls, max_cycles, db_path))
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
        // SendError only fires if the receiver was dropped, which only
        // happens when the worker thread exited — we catch panics inside
        // command processing so that's narrowed to (a) the executor
        // being torn down (app shutting down: swallow is fine) and (b)
        // a worker init failure at spawn time (open_configured_db /
        // Ao3Fetcher::new errored, logged at thread start). Either way
        // there's nothing we can do here; the task stays Running and
        // the user sees a stuck spinner — acceptable trade-off.
        let _ = self.sender.send(WorkerCommand::AddFic { task_id, input });
    }

    /// Enqueue a refresh of an existing fic. `display` carries the fic's
    /// title so the Tasks view shows something meaningful while the
    /// fetch runs. The fic id doubles as the task's "input" for retry.
    pub fn enqueue_refresh(&self, fic_id: u64, display: String) {
        let task_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let task = TaskState {
            id: task_id,
            kind: TaskKind::Refresh,
            input: fic_id.to_string(),
            display,
            status: TaskStatus::Running,
            started_at: Utc::now(),
        };
        self.inbox.tasks.lock().unwrap().push(task);
        let _ = self
            .sender
            .send(WorkerCommand::RefreshFic { task_id, fic_id });
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
    /// entry. New entry gets a fresh task id. Dispatches by `kind` so a
    /// failed Refresh retries as a Refresh, not as an Add.
    pub fn retry(&self, task_id: u64) {
        let snapshot = {
            let mut tasks = self.inbox.tasks.lock().unwrap();
            let pos = tasks.iter().position(|t| t.id == task_id);
            pos.and_then(|i| {
                if matches!(tasks[i].status, TaskStatus::Failed(_)) {
                    let removed = tasks.remove(i);
                    Some((removed.kind, removed.input, removed.display))
                } else {
                    None
                }
            })
        };
        let Some((kind, input, display)) = snapshot else {
            return;
        };
        match kind {
            TaskKind::Add => self.enqueue_add(input),
            TaskKind::Refresh => match input.parse::<u64>() {
                Ok(fic_id) => self.enqueue_refresh(fic_id, display),
                Err(_) => log::warn!("retry: refused to retry refresh task with non-numeric id"),
            },
        }
    }

    /// Drains the queue of titles for fics added since the last call.
    /// Caller should reload its `fics` cache when this returns a non-empty
    /// vec, and may surface one toast per title.
    pub fn take_completions(&self) -> Vec<String> {
        mem::take(&mut *self.inbox.recent_completions.lock().unwrap())
    }

    /// Drains the queue of fic IDs that were successfully refreshed. The
    /// GUI reloads its `fics` cache so the new metadata (and the bumped
    /// `last_checked_date`) are visible immediately.
    pub fn take_refreshes(&self) -> Vec<u64> {
        mem::take(&mut *self.inbox.recent_refreshes.lock().unwrap())
    }

    pub fn has_running(&self) -> bool {
        self.running_count() > 0
    }

    pub fn running_count(&self) -> usize {
        self.inbox
            .tasks
            .lock()
            .unwrap()
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Running))
            .count()
    }
}
