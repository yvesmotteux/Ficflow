use std::io::{self, Write};
use std::process::ExitCode;

use crate::error::FicflowError;
use crate::infrastructure::external::ao3::fetcher::session_site_from_env;
use crate::infrastructure::external::ao3::session::{KeychainSessionStore, SessionStore, log_in};

pub fn login(username: Option<String>) -> ExitCode {
    match prompt_and_log_in(username) {
        Ok(username) => {
            println!("Logged in to AO3 as {username}.");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Error logging in to AO3: {err}");
            ExitCode::FAILURE
        }
    }
}

fn prompt_and_log_in(username: Option<String>) -> Result<String, FicflowError> {
    let username = match username {
        Some(name) => name,
        None => {
            print!("AO3 user name or email: ");
            io::stdout().flush()?;
            let mut name = String::new();
            io::stdin().read_line(&mut name)?;
            name.trim().to_string()
        }
    };
    if username.is_empty() {
        return Err(FicflowError::InvalidInput("a user name is required".into()));
    }
    let password = rpassword::prompt_password("AO3 password: ")?;
    let session = log_in(&session_site_from_env(), &username, &password)?;
    KeychainSessionStore.save(&session)?;
    Ok(session.username)
}

pub fn logout() -> ExitCode {
    match KeychainSessionStore.clear() {
        Ok(()) => {
            println!("Logged out of AO3.");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("Error logging out of AO3: {err}");
            ExitCode::FAILURE
        }
    }
}

pub fn status() -> String {
    KeychainSessionStore.load().map_or_else(
        || "not logged in".to_string(),
        |s| format!("logged in as {}", s.username),
    )
}
