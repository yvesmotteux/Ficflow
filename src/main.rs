use std::process::ExitCode;

use ficflow::infrastructure::external::ao3::fetcher::ao3_urls_from_env;
use ficflow::infrastructure::{establish_connection, Ao3Fetcher, SqliteRepository};

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if bare_invocation() {
        // GUI builds its own connection / fetcher / worker thread
        // inside `FicflowApp::with_config(_, FicflowConfig::default())`,
        // so we don't pre-construct anything here.
        ficflow::interfaces::gui::run_gui()
    } else {
        // CLI takes them by reference because the dispatcher is
        // synchronous and trait-object-based.
        let (urls, max_cycles) = ao3_urls_from_env();
        let fetcher = Ao3Fetcher::new(urls, max_cycles).expect("Failed to create Ao3Fetcher");
        let conn = establish_connection().expect("Failed to establish database connection");
        let repository = SqliteRepository::new(&conn);
        ficflow::interfaces::cli::run_cli(&fetcher, &repository)
    }
}

/// True when the binary was invoked with no positional arguments — the user
/// just typed `ficflow` and wants the GUI. Any subcommand routes to the CLI.
fn bare_invocation() -> bool {
    std::env::args().len() <= 1
}
