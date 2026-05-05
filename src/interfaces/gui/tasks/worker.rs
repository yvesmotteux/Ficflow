use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::application::add_fic::add_fanfiction;
use crate::application::check_updates::check_fic_updates;
use crate::error::FicflowError;
use crate::infrastructure::external::ao3::fetcher::Ao3Fetcher;
use crate::infrastructure::persistence::database::connection::{
    establish_connection, open_configured_db,
};
use crate::infrastructure::SqliteRepository;
use crate::interfaces::utils::url_parser::extract_ao3_id;

use super::{TaskStatus, WorkerCommand, WorkerInbox};

pub fn run(
    rx: Receiver<WorkerCommand>,
    inbox: Arc<WorkerInbox>,
    urls: Vec<String>,
    max_cycles: u32,
    db_path: Option<PathBuf>,
) {
    // Mirror the GUI's connection override so the worker writes land in
    // the same SQLite file the GUI reads from.
    let conn_result = match db_path {
        Some(ref path) => open_configured_db(path),
        None => establish_connection(),
    };
    let conn = match conn_result {
        Ok(c) => c,
        Err(err) => {
            log::error!("task worker couldn't open DB: {}", err);
            return;
        }
    };
    let fetcher = match Ao3Fetcher::new(urls, max_cycles) {
        Ok(f) => f,
        Err(err) => {
            log::error!("task worker couldn't build Ao3Fetcher: {}", err);
            return;
        }
    };
    let repo = SqliteRepository::new(&conn);

    while let Ok(cmd) = rx.recv() {
        match cmd {
            WorkerCommand::AddFic { task_id, input } => {
                // Catch panics inside the AO3 scraper / DB save so a
                // bug in HTML parsing (e.g. an unwrap in a malformed-
                // page edge case) doesn't kill the worker thread —
                // which would leave THIS task stuck on Running and
                // every subsequent command silently dropped.
                let outcome =
                    catch_unwind(AssertUnwindSafe(|| process_add(&fetcher, &repo, &input)))
                        .unwrap_or_else(|payload| Err(panic_to_error(payload)));

                let mut tasks = inbox.tasks.lock().unwrap();
                if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
                    match &outcome {
                        Ok(title) => {
                            task.display = title.clone();
                            task.status = TaskStatus::Done;
                        }
                        Err(err) => {
                            task.status = TaskStatus::Failed(err.to_string());
                        }
                    }
                }
                drop(tasks);
                if let Ok(title) = outcome {
                    inbox.recent_completions.lock().unwrap().push(title);
                }
            }
            WorkerCommand::RefreshFic { task_id, fic_id } => {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    check_fic_updates(&fetcher, &repo, fic_id)
                }))
                .unwrap_or_else(|payload| Err(panic_to_error(payload)));

                let mut tasks = inbox.tasks.lock().unwrap();
                if let Some(task) = tasks.iter_mut().find(|t| t.id == task_id) {
                    match &outcome {
                        Ok((_has_new, fic)) => {
                            task.display = fic.title.clone();
                            task.status = TaskStatus::Done;
                        }
                        Err(err) => {
                            task.status = TaskStatus::Failed(err.to_string());
                        }
                    }
                }
                drop(tasks);
                if outcome.is_ok() {
                    inbox.recent_refreshes.lock().unwrap().push(fic_id);
                }
            }
        }
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
