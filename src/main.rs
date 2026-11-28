#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::process::ExitCode;
use std::sync::Arc;

use ficflow::error::FicflowError;
use ficflow::infrastructure::config::Config;
use ficflow::infrastructure::external::ao3::fetcher::{ao3_urls_from_env, session_site_from_env};
use ficflow::infrastructure::external::ao3::session::KeychainSessionStore;
use ficflow::infrastructure::persistence::epub_folder::EpubFolder;
use ficflow::infrastructure::{Ao3Fetcher, SqliteRepository, open_configured_db};
use ficflow::interfaces::cli::{self, command::CliCommand};

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if bare_invocation() {
        // GUI builds its own connection / fetcher / worker thread
        // inside `FicflowApp::with_config(_, FicflowConfig::default())`,
        // so we don't pre-construct anything here.
        ficflow::interfaces::gui::run_gui()
    } else {
        match cli::command::parse_cli_commands() {
            CliCommand::Config(command) => cli::config::run(command),
            CliCommand::Restore { backup } => cli::config::restore(&backup),
            CliCommand::Login { username } => cli::account::login(username),
            CliCommand::Logout => cli::account::logout(),
            command => run_cli(command).unwrap_or_else(|err| {
                eprintln!("Error: {}", err);
                ExitCode::FAILURE
            }),
        }
    }
}

fn run_cli(command: CliCommand) -> Result<ExitCode, FicflowError> {
    let (urls, max_cycles) = ao3_urls_from_env();
    let fetcher = Ao3Fetcher::new(urls, max_cycles)?
        .with_session(Arc::new(KeychainSessionStore), session_site_from_env());
    let db_path = Config::load().resolved_db_path()?;
    let conn = open_configured_db(&db_path)?;
    let repository = SqliteRepository::new(&conn);
    let epubs = EpubFolder::for_library(&db_path);
    Ok(cli::run_cli(command, &fetcher, &repository, &epubs))
}

/// True when the binary was invoked with no positional arguments — the user
/// just typed `ficflow` and wants the GUI. Any subcommand routes to the CLI.
fn bare_invocation() -> bool {
    std::env::args().len() <= 1
}
