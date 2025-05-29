use rusqlite::Connection;
use std::error::Error;
use std::fs;
use std::env;
use std::path::PathBuf;
use dirs_next::data_local_dir;
use crate::infrastructure::persistence::database::migration::run_migrations;

pub fn establish_connection() -> Result<Connection, Box<dyn Error>> {
    let db_path = if let Ok(path) = env::var("FICFLOW_DB_PATH") {
        PathBuf::from(path)
    } else {
        // Default path in user's data directory
        let mut path = data_local_dir().ok_or("Failed to determine user directory")?;
        path.push("ficflow");
        fs::create_dir_all(&path)?;
        path.push("fanfictions.db");
        path
    };
    
    let mut conn = Connection::open(&db_path)?;
    run_migrations(&mut conn)?;
    Ok(conn)
}
