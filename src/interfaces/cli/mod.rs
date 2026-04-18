pub mod command;
pub mod executor;
pub mod views;

use crate::domain::fanfiction::{DatabaseOps, FanfictionFetcher};
use crate::domain::shelf::ShelfOps;
use executor::CommandExecutor;

pub fn run_cli(
    fetcher: &dyn FanfictionFetcher,
    database_ops: &dyn DatabaseOps,
    shelf_ops: &dyn ShelfOps,
) {
    let command = command::parse_cli_commands();
    let executor = executor::CliCommandExecutor::new(fetcher, database_ops, shelf_ops);

    executor.execute_command(command);
}
