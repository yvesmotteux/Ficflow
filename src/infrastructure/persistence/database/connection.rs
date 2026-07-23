use crate::error::FicflowError;
use crate::infrastructure::persistence::database::migration::run_migrations;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

// SQLite writes two sidecar files next to the database in WAL mode; any move
// or delete of the database has to account for them.
const SIDECAR_SUFFIXES: [&str; 2] = ["-wal", "-shm"];

// Single canonical path to obtain a ready-to-use Connection: create the parent
// directory, open, migrate, then enable FK enforcement and WAL journaling.
// SQLite FKs are off by default per-connection, so production and test code
// must go through here to keep cascades working. WAL mode lets the GUI
// thread's reads and the task-worker thread's writes proceed concurrently —
// without it, the worker can busy-wait for several seconds while the GUI keeps
// grabbing SHARED locks during render(), which manifests as tasks stuck on the
// `Running` state.
pub fn open_configured_db(path: &Path) -> Result<Connection, FicflowError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    run_migrations(&mut conn)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;\
         PRAGMA foreign_keys = ON;",
    )?;
    Ok(conn)
}

/// Moves the library file (and its `-wal`/`-shm` sidecars) from `from` to `to`.
/// On Linux a plain rename keeps a live connection's open descriptors valid
/// through the next restart; if the destination is on another filesystem the
/// rename fails, so we fall back to copy-then-remove.
pub fn relocate_library(from: &Path, to: &Path) -> Result<(), FicflowError> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    move_file(from, to)?;
    for suffix in SIDECAR_SUFFIXES {
        let from_sidecar = sidecar(from, suffix);
        if from_sidecar.exists() {
            move_file(&from_sidecar, &sidecar(to, suffix))?;
        }
    }
    Ok(())
}

/// Restores a backup by copying `backup` over the current library at `current`
/// and clearing any stale `-wal`/`-shm` sidecars so the restored file is read
/// cleanly on the next restart. The configured location is left untouched.
pub fn restore_backup(backup: &Path, current: &Path) -> Result<(), FicflowError> {
    if let Some(parent) = current.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(backup, current)?;
    for suffix in SIDECAR_SUFFIXES {
        let stale = sidecar(current, suffix);
        if stale.exists() {
            fs::remove_file(&stale)?;
        }
    }
    Ok(())
}

fn move_file(from: &Path, to: &Path) -> Result<(), FicflowError> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        // Cross-device rename isn't allowed; copy then delete the original.
        Err(_) => {
            fs::copy(from, to)?;
            fs::remove_file(from)?;
            Ok(())
        }
    }
}

fn sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}
