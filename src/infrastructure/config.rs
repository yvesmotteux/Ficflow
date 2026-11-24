use std::env;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::FicflowError;

const APP_DIR: &str = "ficflow";
const CONFIG_FILE: &str = "config.toml";
const DB_FILE: &str = "fanfictions.db";
const DB_PATH_ENV: &str = "FICFLOW_DB_PATH";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub library_path: Option<PathBuf>,
    #[serde(flatten)]
    pub other: toml::Table,
}

impl Config {
    pub fn load() -> Self {
        load_file()
    }

    pub fn save(&self) -> io::Result<()> {
        save_file(self)
    }

    pub fn resolved_db_path(&self) -> Result<PathBuf, FicflowError> {
        resolve_db_path(self.library_path.as_deref())
    }
}

pub fn config_file_path() -> Option<PathBuf> {
    dirs_next::config_dir().map(|d| d.join(APP_DIR).join(CONFIG_FILE))
}

/// Missing or unparseable files fall back to `Default` so a bad config never blocks startup.
pub fn load_file<T: DeserializeOwned + Default>() -> T {
    let Some(path) = config_file_path() else {
        return T::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return T::default();
    };
    match toml::from_str(&text) {
        Ok(cfg) => cfg,
        Err(err) => {
            log::warn!("Ignoring unparseable config at {:?}: {}", path, err);
            T::default()
        }
    }
}

pub fn save_file<T: Serialize>(value: &T) -> io::Result<()> {
    let Some(path) = config_file_path() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no config directory available on this platform",
        ));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    std::fs::write(&path, text)
}

/// The effective library path, in precedence order: the `FICFLOW_DB_PATH`
/// env override, then the configured location, then the platform default
/// (`~/.local/share/ficflow/fanfictions.db` on Linux).
fn resolve_db_path(configured: Option<&Path>) -> Result<PathBuf, FicflowError> {
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
