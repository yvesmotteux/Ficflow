use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Duration;

use chrono::Utc;

use crate::application::add_fic::add_fanfiction;
use crate::application::background_refresh::{BackgroundRefresher, FailureReport, RefreshSchedule};
use crate::application::check_updates::check_fic_updates;
use crate::application::import_fics::{ImportFrom, ImportPlan, plan_import};
use crate::application::list_fics::list_fics;
use crate::application::sync_series::sync_series;
use crate::domain::fanfiction::Fanfiction;
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;
use crate::infrastructure::external::ao3::ao3_client::DEFAULT_MIN_REQUEST_GAP;
use crate::infrastructure::external::ao3::fetcher::Ao3Fetcher;
use crate::infrastructure::external::ao3::session::SessionStore;
use crate::infrastructure::persistence::database::connection::open_configured_db;
use crate::interfaces::utils::formatter::format_thousands;
use crate::interfaces::utils::url_parser::extract_ao3_id;

use super::{
    IMPORT_LIMIT, ImportNotice, ImportSource, TaskKind, TaskStatus, WorkerCommand, WorkerShared,
    lock,
};

pub struct FetcherSetup {
    pub urls: Vec<String>,
    pub max_cycles: u32,
    pub session_store: Arc<dyn SessionStore>,
    pub session_site: String,
    pub min_gap: Duration,
}

struct QueuedAdd {
    task_id: u64,
    fic_id: u64,
    import: u64,
}

struct ImportProgress {
    label: String,
    total: usize,
    already: usize,
    added: usize,
    failed: usize,
}

pub fn run(
    rx: Receiver<WorkerCommand>,
    shared: Arc<WorkerShared>,
    setup: FetcherSetup,
    db_path: PathBuf,
    schedule: Option<RefreshSchedule>,
) {
    // Open the same SQLite file the GUI reads from so the worker's writes
    // are visible to subsequent reads.
    let conn = match open_configured_db(&db_path) {
        Ok(c) => c,
        Err(err) => {
            log::error!("task worker couldn't open DB: {}", err);
            return;
        }
    };
    let fetcher = match Ao3Fetcher::with_min_gap(
        setup.urls,
        setup.max_cycles,
        setup.min_gap,
        Duration::from_secs(2),
    ) {
        Ok(f) => f.with_session(setup.session_store, setup.session_site),
        Err(err) => {
            log::error!("task worker couldn't build Ao3Fetcher: {}", err);
            return;
        }
    };
    let repo = SqliteRepository::new(&conn);
    let mut background = Background {
        refresher: BackgroundRefresher::new(schedule.unwrap_or_default(), Utc::now()),
        automatic: schedule.is_some(),
        sweep: None,
    };
    if background.is_on(&shared) {
        background.start_sweep(&shared, &repo, false);
    }
    let mut queued: VecDeque<QueuedAdd> = VecDeque::new();
    let mut imports: HashMap<u64, ImportProgress> = HashMap::new();

    loop {
        let cmd = match rx.try_recv() {
            Ok(cmd) => cmd,
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {
                if let Some(add) = queued.pop_front() {
                    run_queued_add(&fetcher, &repo, &shared, &mut imports, add);
                    continue;
                }
                if background.run_next(&fetcher, &repo, &shared) {
                    continue;
                }
                match rx.recv_timeout(background.refresher.schedule().idle_poll) {
                    Ok(cmd) => cmd,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        };
        // Panics inside the AO3 scraper / DB save are caught so a parsing
        // bug on a malformed page doesn't kill the worker thread, which
        // would leave this task stuck on Running and drop every later
        // command.
        match cmd {
            WorkerCommand::AddFic { task_id, input } => {
                let outcome =
                    catch_unwind(AssertUnwindSafe(|| process_add(&fetcher, &repo, &input)))
                        .unwrap_or_else(|payload| Err(panic_to_error(payload)));
                if let Some(title) = record(&shared, task_id, outcome) {
                    lock(&shared.recent_completions).push(title);
                }
            }
            WorkerCommand::Import {
                task_id,
                source,
                confirmed,
            } => {
                let limit = (!confirmed).then_some(IMPORT_LIMIT);
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    let from = match &source {
                        ImportSource::Ids(ids) => ImportFrom::Ids(ids),
                        ImportSource::List(list) => ImportFrom::List(&list.path),
                    };
                    plan_import(&fetcher, &repo, from, limit)
                }))
                .unwrap_or_else(|payload| Err(panic_to_error(payload)));
                let label = source.label();
                match outcome {
                    Ok(ImportPlan::TooMany { count }) => {
                        ask_confirmation(&shared, task_id, &label, count)
                    }
                    Ok(ImportPlan::Add { new_ids, already }) => {
                        if task_is_running(&shared, task_id) {
                            let progress = ImportProgress {
                                label,
                                total: new_ids.len(),
                                already,
                                added: 0,
                                failed: 0,
                            };
                            for fic_id in new_ids {
                                let child = shared.push_task(
                                    TaskKind::Add {
                                        input: fic_id.to_string(),
                                        import: Some(task_id),
                                    },
                                    fic_id.to_string(),
                                    TaskStatus::Running,
                                );
                                queued.push_back(QueuedAdd {
                                    task_id: child,
                                    fic_id,
                                    import: task_id,
                                });
                            }
                            publish_progress(&shared, task_id, &progress);
                            imports.insert(task_id, progress);
                        }
                    }
                    Err(err) => {
                        record(&shared, task_id, Err(err));
                    }
                }
            }
            WorkerCommand::RefreshFic { task_id, fic_id } => {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    check_fic_updates(&fetcher, &repo, &repo, &repo, fic_id)
                }))
                .unwrap_or_else(|payload| Err(panic_to_error(payload)))
                .map(|(fic, message)| {
                    lock(&shared.new_messages).extend(message);
                    fic.title
                });
                if record(&shared, task_id, outcome).is_some() {
                    background.refresher.forgive(fic_id);
                    lock(&shared.recent_refreshes).push(fic_id);
                }
            }
            WorkerCommand::RefreshSeries { task_id, series_id } => {
                let outcome =
                    catch_unwind(AssertUnwindSafe(|| sync_series(&fetcher, &repo, series_id)))
                        .unwrap_or_else(|payload| Err(panic_to_error(payload)))
                        .map(|series| series.title);
                if record(&shared, task_id, outcome).is_some() {
                    shared.library_changed.store(true, Ordering::Relaxed);
                }
            }
            WorkerCommand::CheckAllNow => {
                background.refresher.resume();
                background.start_sweep(&shared, &repo, true);
            }
            WorkerCommand::ResumeBackground => background.refresher.resume(),
        }
    }
}

fn task_is_running(shared: &WorkerShared, task_id: u64) -> bool {
    lock(&shared.tasks)
        .iter()
        .any(|t| t.id == task_id && matches!(t.status, TaskStatus::Running))
}

fn run_queued_add(
    fetcher: &Ao3Fetcher,
    repo: &SqliteRepository<'_>,
    shared: &WorkerShared,
    imports: &mut HashMap<u64, ImportProgress>,
    add: QueuedAdd,
) {
    if !task_is_running(shared, add.task_id) {
        return;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        add_fanfiction(fetcher, repo, repo, add.fic_id)
    }))
    .unwrap_or_else(|payload| Err(panic_to_error(payload)));
    let added = record(shared, add.task_id, outcome).is_some();
    if added {
        shared.library_changed.store(true, Ordering::Relaxed);
    }
    let Some(progress) = imports.get_mut(&add.import) else {
        return;
    };
    if added {
        progress.added += 1;
    } else {
        progress.failed += 1;
    }
    publish_progress(shared, add.import, progress);
    if progress.added + progress.failed == progress.total {
        imports.remove(&add.import);
    }
}

fn publish_progress(shared: &WorkerShared, import_id: u64, progress: &ImportProgress) {
    let finished = progress.added + progress.failed == progress.total;
    let mut parts = Vec::new();
    if finished {
        parts.push(match progress.added {
            1 => "1 fic added".to_string(),
            n => format!("{n} fics added"),
        });
    } else {
        parts.push(format!("{} of {} added", progress.added, progress.total));
    }
    if progress.already > 0 {
        parts.push(format!("{} already in library", progress.already));
    }
    if progress.failed > 0 {
        parts.push(format!("{} failed", progress.failed));
    }
    let summary = format!("{}: {}", progress.label, parts.join(", "));
    let mut tasks = lock(&shared.tasks);
    let Some(task) = tasks
        .iter_mut()
        .find(|t| t.id == import_id && matches!(t.status, TaskStatus::Running))
    else {
        return;
    };
    task.display = summary.clone();
    if finished {
        task.status = TaskStatus::Done;
        drop(tasks);
        lock(&shared.import_notices).push(ImportNotice::Finished(summary));
    }
}

fn ask_confirmation(shared: &WorkerShared, task_id: u64, label: &str, count: Option<usize>) {
    let message = match count {
        Some(count) => format!(
            "{label} has {} fics. Adding them all would take about {}.",
            format_thousands(count as u32),
            estimate(count)
        ),
        None => format!("{label} has more than {IMPORT_LIMIT} fics."),
    };
    let mut tasks = lock(&shared.tasks);
    let Some(task) = tasks
        .iter_mut()
        .find(|t| t.id == task_id && matches!(t.status, TaskStatus::Running))
    else {
        return;
    };
    task.status = TaskStatus::NeedsConfirmation(message.clone());
    drop(tasks);
    lock(&shared.import_notices).push(ImportNotice::NeedsConfirmation(message));
}

fn estimate(count: usize) -> String {
    let minutes = (count as u64 * DEFAULT_MIN_REQUEST_GAP.as_secs()).div_ceil(60);
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

struct Sweep {
    manual: bool,
    found: usize,
}

struct Background {
    refresher: BackgroundRefresher,
    automatic: bool,
    sweep: Option<Sweep>,
}

impl Background {
    fn is_on(&self, shared: &WorkerShared) -> bool {
        self.automatic && shared.background_enabled.load(Ordering::Relaxed)
    }

    fn start_sweep(&mut self, shared: &WorkerShared, repo: &SqliteRepository<'_>, manual: bool) {
        let fics = list_fics(repo).unwrap_or_default();
        self.refresher.start_sweep(&fics, Utc::now(), !manual);
        self.sweep = Some(Sweep { manual, found: 0 });
        self.publish_status(shared);
    }

    fn run_next(
        &mut self,
        fetcher: &Ao3Fetcher,
        repo: &SqliteRepository<'_>,
        shared: &WorkerShared,
    ) -> bool {
        if self.sweep.is_none() && !self.is_on(shared) {
            return false;
        }
        let fics = list_fics(repo).unwrap_or_default();
        let now = Utc::now();
        let next = if self.is_on(shared) {
            self.refresher.next_due(&fics, now)
        } else {
            self.refresher.next_in_sweep(&fics, now)
        };
        let Some(fic_id) = next else {
            self.finish_sweep(shared);
            return false;
        };
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            check_fic_updates(fetcher, repo, repo, repo, fic_id)
        }))
        .unwrap_or_else(|payload| Err(panic_to_error(payload)));
        match outcome {
            Ok((_, message)) => {
                self.refresher.record_success(fic_id);
                if let Some(message) = message {
                    if let Some(sweep) = &mut self.sweep {
                        sweep.found += 1;
                    }
                    lock(&shared.new_messages).push(message);
                }
                shared.library_changed.store(true, Ordering::Relaxed);
            }
            Err(err) => {
                if let Some(report) = self.refresher.record_failure(fic_id, &err, Utc::now()) {
                    surface_failure(shared, &fics, report);
                }
            }
        }
        self.publish_status(shared);
        true
    }

    fn finish_sweep(&mut self, shared: &WorkerShared) {
        if self.refresher.sweep_remaining() > 0 {
            return;
        }
        if let Some(sweep) = self.sweep.take() {
            if sweep.manual {
                lock(&shared.finished_manual_checks).push(sweep.found);
            }
            lock(&shared.background).last_full_check = Some(Utc::now());
            self.publish_status(shared);
        }
    }

    fn publish_status(&self, shared: &WorkerShared) {
        lock(&shared.background).remaining = self.refresher.sweep_remaining();
    }
}

fn surface_failure(shared: &WorkerShared, fics: &[Fanfiction], report: FailureReport) {
    match report {
        FailureReport::GaveUp { fic_id, error } => {
            let title = fics
                .iter()
                .find(|f| f.id == fic_id)
                .map_or_else(|| fic_id.to_string(), |f| f.title.clone());
            shared.push_task(
                TaskKind::Refresh { fic_id },
                title,
                TaskStatus::Failed(format!("Automatic update check failed: {error}")),
            );
        }
        FailureReport::Paused { error } => {
            shared.push_task(
                TaskKind::BackgroundCheck,
                "Automatic update checks paused for a while".to_string(),
                TaskStatus::Failed(error),
            );
        }
    }
}

/// Marks the task Done (showing the fic title) or Failed; returns the
/// title on success.
fn record(
    shared: &WorkerShared,
    task_id: u64,
    outcome: Result<String, FicflowError>,
) -> Option<String> {
    let mut tasks = lock(&shared.tasks);
    let task = tasks.iter_mut().find(|t| t.id == task_id);
    match (task, outcome) {
        (Some(task), Ok(title)) => {
            task.display = title.clone();
            task.status = TaskStatus::Done;
            Some(title)
        }
        (Some(task), Err(err)) => {
            task.status = TaskStatus::Failed(err.to_string());
            None
        }
        (None, outcome) => outcome.ok(),
    }
}

/// Decode a `panic!` / `unwrap` / `expect` payload into a user-facing
/// error message. Non-string payloads get a generic placeholder.
fn panic_to_error(payload: Box<dyn std::any::Any + Send>) -> FicflowError {
    let msg = if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "panic with non-string payload".to_string()
    };
    FicflowError::Other(msg)
}

fn process_add(
    fetcher: &Ao3Fetcher,
    repo: &SqliteRepository<'_>,
    input: &str,
) -> Result<String, FicflowError> {
    let fic_id = extract_ao3_id(input).map_err(FicflowError::InvalidInput)?;
    add_fanfiction(fetcher, repo, repo, fic_id)
}
