use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};

use chrono::Utc;

use crate::application::add_fic::add_fanfiction;
use crate::application::background_refresh::{BackgroundRefresher, FailureReport, RefreshSchedule};
use crate::application::check_updates::check_fic_updates;
use crate::application::list_fics::list_fics;
use crate::domain::fanfiction::Fanfiction;
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;
use crate::infrastructure::external::ao3::fetcher::Ao3Fetcher;
use crate::infrastructure::external::ao3::session::SessionStore;
use crate::infrastructure::persistence::database::connection::open_configured_db;
use crate::interfaces::utils::url_parser::extract_ao3_id;

use super::{TaskKind, TaskStatus, WorkerCommand, WorkerShared, lock};

pub struct FetcherSetup {
    pub urls: Vec<String>,
    pub max_cycles: u32,
    pub session_store: Arc<dyn SessionStore>,
    pub session_site: String,
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
    let fetcher = match Ao3Fetcher::new(setup.urls, setup.max_cycles) {
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

    loop {
        let cmd = match rx.try_recv() {
            Ok(cmd) => cmd,
            Err(TryRecvError::Disconnected) => break,
            Err(TryRecvError::Empty) => {
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
            WorkerCommand::RefreshFic { task_id, fic_id } => {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    check_fic_updates(&fetcher, &repo, &repo, fic_id)
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
            WorkerCommand::CheckAllNow => {
                background.refresher.resume();
                background.start_sweep(&shared, &repo, true);
            }
            WorkerCommand::ResumeBackground => background.refresher.resume(),
        }
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
            check_fic_updates(fetcher, repo, repo, fic_id)
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
    add_fanfiction(fetcher, repo, fic_id)
}
