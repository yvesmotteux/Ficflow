pub mod command;
pub mod executor;
pub mod views;

use crate::domain::fanfiction::{FanfictionFetcher, FanfictionOps};
use crate::domain::shelf::ShelfOps;
use executor::CommandExecutor;

pub fn run_cli(
    fetcher: &dyn FanfictionFetcher,
    fanfiction_ops: &dyn FanfictionOps,
    shelf_ops: &dyn ShelfOps,
) {
    let command = command::parse_cli_commands();
    let executor = executor::CliCommandExecutor::new(fetcher, fanfiction_ops, shelf_ops);

    executor.execute_command(command);
}
