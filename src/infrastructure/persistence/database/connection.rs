use rusqlite::Connection;
use std::fs;
use std::env;
use std::path::PathBuf;
use dirs_next::data_local_dir;
use crate::error::FicflowError;
use crate::infrastructure::persistence::database::migration::run_migrations;

pub fn establish_connection() -> Result<Connection, FicflowError> {
    let db_path = if let Ok(path) = env::var("FICFLOW_DB_PATH") {
        PathBuf::from(path)
    } else {
        let mut path = data_local_dir()
            .ok_or_else(|| FicflowError::Other("Failed to determine user data directory".into()))?;
        path.push("ficflow");
        fs::create_dir_all(&path)?;
        path.push("fanfictions.db");
        path
    };

    let mut conn = Connection::open(&db_path)?;
    run_migrations(&mut conn)?;
    Ok(conn)
}
