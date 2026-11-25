//! Background-task plumbing for AO3 fetches that would otherwise block the
//! UI thread (which `eframe` runs on). The worker thread owns its own
//! `rusqlite::Connection` (Connection is `!Send`) and `Ao3Fetcher`; the GUI
//! sees task progress through a shared `Vec<TaskState>` behind a Mutex.

pub mod worker;

use std::mem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use chrono::{DateTime, Utc};

use crate::infrastructure::external::ao3::session::SessionStore;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Add { input: String },
    Refresh { fic_id: u64 },
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
    /// Carries the original request so "Retry" can re-enqueue it.
    pub kind: TaskKind,
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

/// A panic elsewhere while holding one of these locks shouldn't take every
/// later frame down with it, so poisoning is ignored.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
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
    /// Worker terminates when `sender` is dropped (rx.recv returns Err);
    /// the handle is held only so the thread has a stable identity.
    _worker: thread::JoinHandle<()>,
}

impl TaskExecutor {
    /// Worker opens its own SQLite connection (`Connection: !Send`); it
    /// must point at the same file as the GUI's so writes are visible
    /// to subsequent reads, hence the shared `db_path`.
    pub fn spawn(
        urls: Vec<String>,
        max_cycles: u32,
        db_path: PathBuf,
        session_store: Arc<dyn SessionStore>,
        session_site: String,
    ) -> Self {
        let inbox = Arc::new(WorkerInbox::new());
        let (tx, rx) = mpsc::channel();
        let worker_inbox = Arc::clone(&inbox);
        let worker = thread::Builder::new()
            .name("ficflow-tasks".into())
            .spawn(move || {
                let fetcher_setup = worker::FetcherSetup {
                    urls,
                    max_cycles,
                    session_store,
                    session_site,
                };
                worker::run(rx, worker_inbox, fetcher_setup, db_path)
            })
            .expect("failed to spawn task-worker thread");
        Self {
            inbox,
            sender: tx,
            next_id: AtomicU64::new(1),
            _worker: worker,
        }
    }

    pub fn enqueue_add(&self, input: String) {
        let task_id = self.push_task(
            TaskKind::Add {
                input: input.clone(),
            },
            input.clone(),
        );
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
    /// fetch runs.
    pub fn enqueue_refresh(&self, fic_id: u64, display: String) {
        let task_id = self.push_task(TaskKind::Refresh { fic_id }, display);
        let _ = self
            .sender
            .send(WorkerCommand::RefreshFic { task_id, fic_id });
    }

    fn push_task(&self, kind: TaskKind, display: String) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.inbox.tasks).push(TaskState {
            id,
            kind,
            display,
            status: TaskStatus::Running,
            started_at: Utc::now(),
        });
        id
    }

    pub fn snapshot(&self) -> Vec<TaskState> {
        lock(&self.inbox.tasks).clone()
    }

    pub fn clear_completed(&self) {
        lock(&self.inbox.tasks).retain(|t| matches!(t.status, TaskStatus::Running));
    }

    /// Re-runs a failed task using its original input, dropping the failed
    /// entry. New entry gets a fresh task id. Dispatches by `kind` so a
    /// failed Refresh retries as a Refresh, not as an Add.
    pub fn retry(&self, task_id: u64) {
        let failed = {
            let mut tasks = lock(&self.inbox.tasks);
            let Some(i) = tasks
                .iter()
                .position(|t| t.id == task_id && matches!(t.status, TaskStatus::Failed(_)))
            else {
                return;
            };
            tasks.remove(i)
        };
        match failed.kind {
            TaskKind::Add { input } => self.enqueue_add(input),
            TaskKind::Refresh { fic_id } => self.enqueue_refresh(fic_id, failed.display),
        }
    }

    pub fn retry_all_failed(&self) {
        let failed_ids: Vec<u64> = {
            let tasks = lock(&self.inbox.tasks);
            tasks
                .iter()
                .filter_map(|t| matches!(t.status, TaskStatus::Failed(_)).then_some(t.id))
                .collect()
        };
        for id in failed_ids {
            self.retry(id);
        }
    }

    /// Drains the queue of titles for fics added since the last call.
    /// Caller should reload its `fics` cache when this returns a non-empty
    /// vec, and may surface one toast per title.
    pub fn take_completions(&self) -> Vec<String> {
        mem::take(&mut *lock(&self.inbox.recent_completions))
    }

    /// Drains the queue of fic IDs that were successfully refreshed. The
    /// GUI reloads its `fics` cache so the new metadata (and the bumped
    /// `last_checked_date`) are visible immediately.
    pub fn take_refreshes(&self) -> Vec<u64> {
        mem::take(&mut *lock(&self.inbox.recent_refreshes))
    }

    pub fn has_running(&self) -> bool {
        self.running_count() > 0
    }

    pub fn running_count(&self) -> usize {
        lock(&self.inbox.tasks)
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Running))
            .count()
    }
}
