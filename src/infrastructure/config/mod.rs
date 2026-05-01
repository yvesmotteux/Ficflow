//! Persistent app preferences stored as TOML at the platform's config dir
//! (`~/.config/ficflow/config.toml` on Linux). Holds visible columns and the
//! default sort for the library table — read at startup, written when the
//! user changes them.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "config.toml";
const APP_DIR: &str = "ficflow";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnKey {
    Title,
    Author,
    Status,
    LastChapter,
    Rating,
    Reads,
    Updated,
}

impl ColumnKey {
    pub const ALL: [ColumnKey; 7] = [
        ColumnKey::Title,
        ColumnKey::Author,
        ColumnKey::Status,
        ColumnKey::LastChapter,
        ColumnKey::Rating,
        ColumnKey::Reads,
        ColumnKey::Updated,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColumnKey::Title => "Title",
            ColumnKey::Author => "Author",
            ColumnKey::Status => "Status",
            ColumnKey::LastChapter => "Last Ch.",
            ColumnKey::Rating => "Rating",
            ColumnKey::Reads => "Reads",
            ColumnKey::Updated => "Updated",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortPref {
    pub column: ColumnKey,
    pub direction: SortDirection,
}

impl Default for SortPref {
    fn default() -> Self {
        Self {
            column: ColumnKey::Updated,
            direction: SortDirection::Descending,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub visible_columns: Vec<ColumnKey>,
    pub default_sort: SortPref,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            visible_columns: ColumnKey::ALL.to_vec(),
            default_sort: SortPref::default(),
        }
    }
}

impl AppConfig {
    /// Loads the config from disk. Returns `Default` if the file is missing
    /// or unparseable — never blocks startup on config issues.
    pub fn load() -> Self {
        let Some(path) = config_path() else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match toml::from_str(&text) {
            Ok(cfg) => cfg,
            Err(err) => {
                log::warn!("Ignoring unparseable config at {:?}: {}", path, err);
                Self::default()
            }
        }
    }

    /// Writes the config to disk, creating parent directories as needed.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = config_path() else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no config directory available on this platform",
            ));
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
        std::fs::write(&path, text)
    }
}

fn config_path() -> Option<PathBuf> {
    dirs_next::config_dir().map(|d| d.join(APP_DIR).join(CONFIG_FILE))
}
