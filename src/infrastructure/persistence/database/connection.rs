use crate::error::FicflowError;
use crate::infrastructure::persistence::database::migration::run_migrations;
use dirs_next::data_local_dir;
use rusqlite::Connection;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

    open_configured_db(&db_path)
}

// Single canonical path to obtain a ready-to-use Connection: open, migrate,
// then enable FK enforcement and WAL journaling. SQLite FKs are off by
// default per-connection, so production and test code must go through here
// to keep cascades working. WAL mode lets the GUI thread's reads and the
// task-worker thread's writes proceed concurrently — without it, the
// worker can busy-wait for several seconds while the GUI keeps grabbing
// SHARED locks during render(), which manifests as tasks stuck on the
// `Running` state.
pub fn open_configured_db(path: &Path) -> Result<Connection, FicflowError> {
    let mut conn = Connection::open(path)?;
    run_migrations(&mut conn)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(conn)
}
