pub mod command;
pub mod executor;
pub mod views;

use crate::domain::fanfiction::FanfictionFetcher;
use crate::domain::repository::Repository;
use executor::CommandExecutor;

pub fn run_cli(fetcher: &dyn FanfictionFetcher, repository: &dyn Repository) -> Result<(), ()> {
    let command = command::parse_cli_commands();
    let executor = executor::CliCommandExecutor::new(fetcher, repository);

    executor.execute_command(command)
}
