use std::env;
use std::process::ExitCode;

use super::command::ConfigCommand;
use crate::infrastructure::config::{Config, DB_PATH_ENV, config_file_path};

pub fn run(command: ConfigCommand) -> ExitCode {
    match command {
        ConfigCommand::Show => show(),
    }
}

fn show() -> ExitCode {
    let config = Config::load();
    let config_file =
        config_file_path().map_or_else(|| "<unavailable>".to_string(), |p| p.display().to_string());
    println!("Config file: {}", config_file);
    match config.resolved_db_path() {
        Ok(path) if env::var(DB_PATH_ENV).is_ok() => {
            println!("Library:     {} (set by {})", path.display(), DB_PATH_ENV)
        }
        Ok(path) => println!("Library:     {}", path.display()),
        Err(err) => {
            eprintln!("Error finding the library: {}", err);
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
