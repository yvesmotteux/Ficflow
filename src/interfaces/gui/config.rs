//! Persistent GUI preferences stored as TOML at the platform's config
//! dir (`~/.config/ficflow/config.toml` on Linux). Holds visible
//! columns, the default sort for the library table, and the
//! maximized/fullscreen window state — read at startup, written when
//! the user changes them.
//!
//! Lives under `interfaces/gui/` because every field here is a GUI
//! concern: column visibility, sort direction, and window state have
//! no meaning to the CLI.

use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

const CONFIG_FILE: &str = "config.toml";
const APP_DIR: &str = "ficflow";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnKey {
    Title,
    Author,
    Fandom,
    Pairing,
    AO3Rating,
    Warnings,
    Status,
    Complete,
    LastChapter,
    Words,
    Kudos,
    Hits,
    Rating,
    Reads,
    Language,
    DatePublished,
    Updated,
}

impl ColumnKey {
    /// Canonical column order. Used by the picker and to keep visible
    /// columns in a deterministic spot when toggled. The default config
    /// only enables a subset (see `AppConfig::default`); the rest are
    /// opt-in via the column picker.
    pub const ALL: [ColumnKey; 17] = [
        ColumnKey::Title,
        ColumnKey::Author,
        ColumnKey::Fandom,
        ColumnKey::Pairing,
        ColumnKey::AO3Rating,
        ColumnKey::Warnings,
        ColumnKey::Status,
        ColumnKey::Complete,
        ColumnKey::LastChapter,
        ColumnKey::Words,
        ColumnKey::Kudos,
        ColumnKey::Hits,
        ColumnKey::Rating,
        ColumnKey::Reads,
        ColumnKey::Language,
        ColumnKey::DatePublished,
        ColumnKey::Updated,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColumnKey::Title => "Title",
            ColumnKey::Author => "Author",
            ColumnKey::Fandom => "Fandom",
            ColumnKey::Pairing => "Pairing",
            ColumnKey::AO3Rating => "AO3 Rating",
            ColumnKey::Warnings => "Warnings",
            ColumnKey::Status => "Status",
            ColumnKey::Complete => "Complete",
            ColumnKey::LastChapter => "Last Ch.",
            ColumnKey::Words => "Words",
            ColumnKey::Kudos => "Kudos",
            ColumnKey::Hits => "Hits",
            ColumnKey::Rating => "Rating",
            ColumnKey::Reads => "Reads",
            ColumnKey::Language => "Language",
            ColumnKey::DatePublished => "Published",
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
    /// Tracked alongside the eframe-managed window size/position so a
    /// maximized or fullscreen window comes back the same way next launch.
    /// eframe 0.29 only persists fullscreen for us, not maximized — and
    /// even fullscreen tracking is fragile across WMs — so we record both
    /// here and re-apply on the first frame.
    #[serde(default)]
    pub window_maximized: bool,
    #[serde(default)]
    pub window_fullscreen: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            // Curated default — the 7 most-used columns. Everything else
            // (Fandom, Pairing, AO3 Rating, Warnings, Words, Kudos, Hits,
            // Language, Published, …) is hidden by default and the user
            // opts in via the column picker.
            visible_columns: vec![
                ColumnKey::Title,
                ColumnKey::Author,
                ColumnKey::Status,
                ColumnKey::LastChapter,
                ColumnKey::Rating,
                ColumnKey::Reads,
                ColumnKey::Updated,
            ],
            default_sort: SortPref::default(),
            window_maximized: false,
            window_fullscreen: false,
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
