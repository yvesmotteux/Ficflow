pub mod command;
pub mod config;
pub mod executor;
pub mod views;

use std::process::ExitCode;

use crate::domain::fanfiction::FanfictionFetcher;
use crate::domain::repository::Repository;

pub fn run_cli(
    command: command::CliCommand,
    fetcher: &dyn FanfictionFetcher,
    repository: &dyn Repository,
) -> ExitCode {
    let executor = executor::CliCommandExecutor::new(fetcher, repository);

    executor.execute_command(command)
}
