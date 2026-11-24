use std::env;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::FicflowError;

const APP_DIR: &str = "ficflow";
const CONFIG_FILE: &str = "config.toml";
const DB_FILE: &str = "fanfictions.db";
const DB_PATH_ENV: &str = "FICFLOW_DB_PATH";

pub fn config_file_path() -> Option<PathBuf> {
    dirs_next::config_dir().map(|d| d.join(APP_DIR).join(CONFIG_FILE))
}

/// The effective library path, in precedence order: the `FICFLOW_DB_PATH`
/// env override, then the configured location, then the platform default
/// (`~/.local/share/ficflow/fanfictions.db` on Linux).
pub fn resolve_db_path(configured: Option<&Path>) -> Result<PathBuf, FicflowError> {
    if let Ok(path) = env::var(DB_PATH_ENV) {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = configured {
        return Ok(path.to_path_buf());
    }
    let data_dir = dirs_next::data_local_dir()
        .ok_or_else(|| FicflowError::Other("Failed to determine user data directory".into()))?;
    Ok(data_dir.join(APP_DIR).join(DB_FILE))
}

/// Reads just `library_path` from the config file, so the CLI can find the
/// library without depending on the GUI's preferences.
pub fn configured_library_path() -> Option<PathBuf> {
    #[derive(Deserialize)]
    struct LibraryOnly {
        library_path: Option<PathBuf>,
    }
    let text = std::fs::read_to_string(config_file_path()?).ok()?;
    toml::from_str::<LibraryOnly>(&text).ok()?.library_path
}
