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
                let outcome = process_add(&fetcher, &repo, &input);
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
                let outcome = check_fic_updates(&fetcher, &repo, fic_id);
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

fn process_add(
    fetcher: &Ao3Fetcher,
    repo: &SqliteRepository<'_>,
    input: &str,
) -> Result<String, FicflowError> {
    let fic_id = extract_ao3_id(input).map_err(FicflowError::InvalidInput)?;
    add_fanfiction(fetcher, repo, fic_id)
}
