//! Background-task plumbing for AO3 fetches that would otherwise block the
//! UI thread (which `eframe` runs on). The worker thread owns its own
//! `rusqlite::Connection` (Connection is `!Send`) and `Ao3Fetcher`; the GUI
//! sees task progress through a shared `Vec<TaskState>` behind a Mutex.

pub mod worker;

use std::mem;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use chrono::{DateTime, Utc};

use crate::application::background_refresh::RefreshSchedule;
use crate::domain::inbox::InboxMessage;
use crate::interfaces::utils::url_parser::Ao3List;

pub use worker::FetcherSetup;

pub const IMPORT_LIMIT: usize = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskKind {
    Add { input: String, import: Option<u64> },
    Import { source: ImportSource },
    Refresh { fic_id: u64 },
    RefreshSeries { series_id: u64 },
    BackgroundCheck,
    EpubBackup { fic_id: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportSource {
    Ids(Vec<u64>),
    List(Ao3List),
    Sync(Ao3List),
}

impl ImportSource {
    pub fn label(&self) -> String {
        match self {
            ImportSource::Ids(ids) => format!("{} fic IDs", ids.len()),
            ImportSource::List(list) | ImportSource::Sync(list) => list.label.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TaskStatus {
    Running,
    Done,
    Failed(String),
    NeedsConfirmation(String),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportNotice {
    Finished(String),
    NeedsConfirmation(String),
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
    AddFic {
        task_id: u64,
        input: String,
    },
    Import {
        task_id: u64,
        source: ImportSource,
        confirmed: bool,
    },
    RefreshFic {
        task_id: u64,
        fic_id: u64,
    },
    RefreshSeries {
        task_id: u64,
        series_id: u64,
    },
    BackupEpub {
        task_id: u64,
        fic_id: u64,
    },
    CheckAllNow,
    ResumeBackground,
}

#[derive(Clone, Debug, Default)]
pub struct BackgroundStatus {
    pub remaining: usize,
    pub last_full_check: Option<DateTime<Utc>>,
}

/// A panic elsewhere while holding one of these locks shouldn't take every
/// later frame down with it, so poisoning is ignored.
pub(super) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) struct WorkerShared {
    pub tasks: Mutex<Vec<TaskState>>,
    /// Titles of fics that were just successfully added. The GUI drains this
    /// each frame to (a) reload its `fics` cache and (b) toast a confirmation
    /// per added fic.
    pub recent_completions: Mutex<Vec<String>>,
    /// IDs of fics that were just successfully refreshed. The GUI drains
    /// this to reload the in-memory cache so the new metadata + bumped
    /// `last_checked_date` show in the details panel.
    pub recent_refreshes: Mutex<Vec<u64>>,
    pub new_messages: Mutex<Vec<InboxMessage>>,
    pub library_changed: AtomicBool,
    pub background_enabled: AtomicBool,
    pub epub_backups: AtomicBool,
    pub background: Mutex<BackgroundStatus>,
    pub finished_manual_checks: Mutex<Vec<usize>>,
    pub import_notices: Mutex<Vec<ImportNotice>>,
    next_id: AtomicU64,
}

impl WorkerShared {
    fn new(background_enabled: bool, epub_backups: bool) -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            recent_completions: Mutex::new(Vec::new()),
            recent_refreshes: Mutex::new(Vec::new()),
            new_messages: Mutex::new(Vec::new()),
            library_changed: AtomicBool::new(false),
            background_enabled: AtomicBool::new(background_enabled),
            epub_backups: AtomicBool::new(epub_backups),
            background: Mutex::new(BackgroundStatus::default()),
            finished_manual_checks: Mutex::new(Vec::new()),
            import_notices: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
        }
    }

    pub fn push_task(&self, kind: TaskKind, display: String, status: TaskStatus) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.tasks).push(TaskState {
            id,
            kind,
            display,
            status,
            started_at: Utc::now(),
        });
        id
    }
}

pub struct TaskExecutor {
    shared: Arc<WorkerShared>,
    sender: Sender<WorkerCommand>,
    /// Worker terminates when `sender` is dropped (rx.recv returns Err);
    /// the handle is held only so the thread has a stable identity.
    _worker: thread::JoinHandle<()>,
}

impl TaskExecutor {
    /// Worker opens its own SQLite connection (`Connection: !Send`); it
    /// must point at the same file as the GUI's so writes are visible
    /// to subsequent reads, hence the shared `db_path`.
    pub fn spawn(
        fetcher_setup: FetcherSetup,
        db_path: PathBuf,
        schedule: Option<RefreshSchedule>,
        background_enabled: bool,
        epub_backups: bool,
    ) -> Self {
        let shared = Arc::new(WorkerShared::new(background_enabled, epub_backups));
        let (tx, rx) = mpsc::channel();
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("ficflow-tasks".into())
            .spawn(move || worker::run(rx, worker_shared, fetcher_setup, db_path, schedule))
            .expect("failed to spawn task-worker thread");
        Self {
            shared,
            sender: tx,
            _worker: worker,
        }
    }

    pub fn enqueue_add(&self, input: String) {
        let task_id = self.push_task(
            TaskKind::Add {
                input: input.clone(),
                import: None,
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

    pub fn enqueue_import(&self, source: ImportSource) {
        self.send_import(source, false);
    }

    fn send_import(&self, source: ImportSource, confirmed: bool) {
        let task_id = self.push_task(
            TaskKind::Import {
                source: source.clone(),
            },
            source.label(),
        );
        let _ = self.sender.send(WorkerCommand::Import {
            task_id,
            source,
            confirmed,
        });
    }

    pub fn enqueue_sync(&self, list: Ao3List) {
        self.send_import(ImportSource::Sync(list), true);
    }

    pub fn is_syncing(&self) -> bool {
        lock(&self.shared.tasks).iter().any(|t| {
            matches!(t.status, TaskStatus::Running)
                && matches!(
                    t.kind,
                    TaskKind::Import {
                        source: ImportSource::Sync(_)
                    }
                )
        })
    }

    pub fn confirm_import(&self, task_id: u64) {
        let Some(task) = self.remove_task(task_id, |status| {
            matches!(status, TaskStatus::NeedsConfirmation(_))
        }) else {
            return;
        };
        if let TaskKind::Import { source } = task.kind {
            self.send_import(source, true);
        }
    }

    pub fn cancel_import(&self, task_id: u64) {
        let mut tasks = lock(&self.shared.tasks);
        for task in tasks.iter_mut() {
            let in_import = task.id == task_id
                || matches!(task.kind, TaskKind::Add { import: Some(parent), .. } if parent == task_id);
            let pending = matches!(
                task.status,
                TaskStatus::Running | TaskStatus::NeedsConfirmation(_)
            );
            if in_import && pending {
                task.status = TaskStatus::Cancelled;
            }
        }
    }

    fn remove_task(
        &self,
        task_id: u64,
        status_matches: impl Fn(&TaskStatus) -> bool,
    ) -> Option<TaskState> {
        let mut tasks = lock(&self.shared.tasks);
        let i = tasks
            .iter()
            .position(|t| t.id == task_id && status_matches(&t.status))?;
        Some(tasks.remove(i))
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

    pub fn enqueue_series_refresh(&self, series_id: u64, display: String) {
        let task_id = self.push_task(TaskKind::RefreshSeries { series_id }, display);
        let _ = self
            .sender
            .send(WorkerCommand::RefreshSeries { task_id, series_id });
    }

    fn push_task(&self, kind: TaskKind, display: String) -> u64 {
        self.shared.push_task(kind, display, TaskStatus::Running)
    }

    pub fn check_all_now(&self) {
        let _ = self.sender.send(WorkerCommand::CheckAllNow);
    }

    pub fn set_background_enabled(&self, enabled: bool) {
        self.shared
            .background_enabled
            .store(enabled, Ordering::Relaxed);
    }

    pub fn set_epub_backups(&self, enabled: bool) {
        self.shared.epub_backups.store(enabled, Ordering::Relaxed);
    }

    fn enqueue_epub_backup(&self, fic_id: u64, display: String) {
        let task_id = self.push_task(TaskKind::EpubBackup { fic_id }, display);
        let _ = self
            .sender
            .send(WorkerCommand::BackupEpub { task_id, fic_id });
    }

    pub fn background_status(&self) -> BackgroundStatus {
        lock(&self.shared.background).clone()
    }

    pub fn take_finished_manual_checks(&self) -> Vec<usize> {
        mem::take(&mut *lock(&self.shared.finished_manual_checks))
    }

    pub fn take_library_changed(&self) -> bool {
        self.shared.library_changed.swap(false, Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> Vec<TaskState> {
        lock(&self.shared.tasks).clone()
    }

    pub fn clear_completed(&self) {
        lock(&self.shared.tasks).retain(|t| {
            matches!(
                t.status,
                TaskStatus::Running | TaskStatus::NeedsConfirmation(_)
            )
        });
    }

    /// Re-runs a failed task using its original input, dropping the failed
    /// entry. New entry gets a fresh task id. Dispatches by `kind` so a
    /// failed Refresh retries as a Refresh, not as an Add.
    pub fn retry(&self, task_id: u64) {
        let Some(failed) =
            self.remove_task(task_id, |status| matches!(status, TaskStatus::Failed(_)))
        else {
            return;
        };
        match failed.kind {
            TaskKind::Add { input, .. } => self.enqueue_add(input),
            TaskKind::Import {
                source: ImportSource::Sync(list),
            } => self.enqueue_sync(list),
            TaskKind::Import { source } => self.enqueue_import(source),
            TaskKind::Refresh { fic_id } => self.enqueue_refresh(fic_id, failed.display),
            TaskKind::RefreshSeries { series_id } => {
                self.enqueue_series_refresh(series_id, failed.display)
            }
            TaskKind::BackgroundCheck => {
                let _ = self.sender.send(WorkerCommand::ResumeBackground);
            }
            TaskKind::EpubBackup { fic_id } => self.enqueue_epub_backup(fic_id, failed.display),
        }
    }

    pub fn retry_all_failed(&self) {
        let failed_ids: Vec<u64> = {
            let tasks = lock(&self.shared.tasks);
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
        mem::take(&mut *lock(&self.shared.recent_completions))
    }

    /// Drains the queue of fic IDs that were successfully refreshed. The
    /// GUI reloads its `fics` cache so the new metadata (and the bumped
    /// `last_checked_date`) are visible immediately.
    pub fn take_refreshes(&self) -> Vec<u64> {
        mem::take(&mut *lock(&self.shared.recent_refreshes))
    }

    pub fn take_import_notices(&self) -> Vec<ImportNotice> {
        mem::take(&mut *lock(&self.shared.import_notices))
    }

    pub fn take_new_messages(&self) -> Vec<InboxMessage> {
        mem::take(&mut *lock(&self.shared.new_messages))
    }

    pub fn has_running(&self) -> bool {
        self.running_count() > 0
    }

    pub fn failed_count(&self) -> usize {
        lock(&self.shared.tasks)
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Failed(_)))
            .count()
    }

    pub fn running_count(&self) -> usize {
        lock(&self.shared.tasks)
            .iter()
            .filter(|t| {
                matches!(
                    t.status,
                    TaskStatus::Running | TaskStatus::NeedsConfirmation(_)
                )
            })
            .count()
    }

    pub fn waiting_count(&self) -> usize {
        lock(&self.shared.tasks)
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::NeedsConfirmation(_)))
            .count()
    }
}
