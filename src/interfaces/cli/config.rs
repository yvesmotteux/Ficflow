use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use super::account;
use super::command::ConfigCommand;
use super::executor::confirm;
use crate::error::FicflowError;
use crate::infrastructure::config::{Config, DB_PATH_ENV, config_file_path};
use crate::infrastructure::{move_library_into, restore_backup};

pub fn run(command: ConfigCommand) -> ExitCode {
    match command {
        ConfigCommand::Show => show(),
        ConfigCommand::LibraryLocation { folder } => match change_library_location(&folder) {
            Ok(path) => {
                println!("Library location set to {}", path.display());
                if env::var(DB_PATH_ENV).is_ok() {
                    println!("Note: {} is set and still takes precedence.", DB_PATH_ENV);
                }
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("Error changing the library location: {}", err);
                ExitCode::FAILURE
            }
        },
    }
}

fn change_library_location(folder: &Path) -> Result<PathBuf, FicflowError> {
    let mut config = Config::load();
    let current = config.resolved_db_path()?;
    let target = move_library_into(&current, &std::path::absolute(folder)?)?;
    config.library_path = Some(target.clone());
    config.save()?;
    Ok(target)
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
    println!("AO3 account: {}", account::status());
    ExitCode::SUCCESS
}

pub fn restore(backup: &Path) -> ExitCode {
    let library = match Config::load().resolved_db_path() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Error finding the library: {}", err);
            return ExitCode::FAILURE;
        }
    };
    let warning = format!(
        "WARNING: This replaces your library at {} with {}. \
         Anything not in the backup will be lost!",
        library.display(),
        backup.display()
    );
    if !confirm(&warning) {
        println!("Operation cancelled.");
        return ExitCode::SUCCESS;
    }
    match restore_backup(backup, &library) {
        Ok(()) => {
            println!("Backup restored.");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Error restoring the backup: {}", err);
            ExitCode::FAILURE
        }
    }
}
