use crate::domain::db::DatabaseOps;
use crate::infrastructure::db::wipe_database as infra_wipe_database;
use rusqlite::Connection;
use std::error::Error;
use std::io::{self, Write};
use std::env;

pub fn wipe_database(_db_ops: &dyn DatabaseOps, conn: &mut Connection) -> Result<(), Box<dyn Error>> {
    // Check if we're in non-interactive mode (for testing)
    if env::var("FICFLOW_NON_INTERACTIVE").is_ok() {
        // Skip confirmation in test mode
        infra_wipe_database(conn)?;
        println!("Database wiped successfully.");
        return Ok(());
    }

    // Ask for confirmation with a warning
    print!("WARNING: This action will delete ALL fanfictions from the database. This process CANNOT be reversed!\nAre you sure you want to continue? (y/N): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    // Check if the user confirmed
    if input.trim().to_lowercase() == "y" {
        // Call the infrastructure function to wipe the database
        infra_wipe_database(conn)?;
        
        println!("Database wiped successfully.");
        return Ok(());
    } else {
        println!("Operation cancelled.");
        return Ok(());
    }
}