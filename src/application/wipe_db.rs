use crate::domain::db::DatabaseOps;
use std::error::Error;
use std::io::{self, Write};
use std::env;

pub fn wipe_database(db_ops: &dyn DatabaseOps) -> Result<(), Box<dyn Error>> {
    // Check if we're in non-interactive mode (for testing)
    if env::var("FICFLOW_NON_INTERACTIVE").is_ok() {
        // Skip confirmation in test mode
        db_ops.wipe_database()?;
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
        db_ops.wipe_database()?;
        
        println!("Database wiped successfully.");
        return Ok(());
    } else {
        println!("Operation cancelled.");
        return Ok(());
    }
}