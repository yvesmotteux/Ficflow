use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use crate::application::add_fic::add_fanfiction;
use crate::application::check_updates::check_fic_updates;
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;
use crate::infrastructure::external::ao3::fetcher::Ao3Fetcher;
use crate::infrastructure::external::ao3::session::SessionStore;
use crate::infrastructure::persistence::database::connection::open_configured_db;
use crate::interfaces::utils::url_parser::extract_ao3_id;

use super::{TaskStatus, WorkerCommand, WorkerShared, lock};

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

    while let Ok(cmd) = rx.recv() {
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
                    lock(&shared.recent_refreshes).push(fic_id);
                }
            }
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
