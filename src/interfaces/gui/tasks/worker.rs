use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::application::add_fic::add_fanfiction;
use crate::error::FicflowError;
use crate::infrastructure::external::ao3::fetcher::Ao3Fetcher;
use crate::infrastructure::persistence::database::connection::establish_connection;
use crate::infrastructure::SqliteRepository;
use crate::interfaces::utils::url_parser::extract_ao3_id;

use super::{TaskStatus, WorkerCommand, WorkerInbox};

pub fn run(
    rx: Receiver<WorkerCommand>,
    inbox: Arc<WorkerInbox>,
    urls: Vec<String>,
    max_cycles: u32,
) {
    let conn = match establish_connection() {
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
