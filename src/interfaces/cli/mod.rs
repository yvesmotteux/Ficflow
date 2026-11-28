pub mod account;
pub mod command;
pub mod config;
pub mod executor;
pub mod views;

use std::process::ExitCode;

use crate::domain::fanfiction::{EpubStore, FanfictionFetcher};
use crate::domain::repository::Repository;

pub fn run_cli(
    command: command::CliCommand,
    fetcher: &dyn FanfictionFetcher,
    repository: &dyn Repository,
    epubs: &dyn EpubStore,
) -> ExitCode {
    let executor = executor::CliCommandExecutor::new(fetcher, repository, epubs);

    executor.execute_command(command)
}
