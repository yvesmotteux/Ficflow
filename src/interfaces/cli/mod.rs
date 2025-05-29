pub mod command;
pub mod executor;
pub mod views;

use crate::domain::{db::DatabaseOps, fic::FanfictionFetcher};
use executor::CommandExecutor;

pub fn run_cli(fetcher: &dyn FanfictionFetcher, database: &dyn DatabaseOps) {
    let command = command::parse_cli_commands();
    let executor = executor::CliCommandExecutor::new(fetcher, database);
    
    executor.execute_command(command);
}
