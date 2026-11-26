use std::io;

use serde::{Deserialize, Serialize};

use crate::domain::fanfiction::ReadingStatus;
use crate::infrastructure::config::{self as shared_config, Config};

/// Serializable shadow of the library-facing `View` variants — the ones
/// worth restoring on next launch. `Tasks` and `Settings` are transient
/// pages, not "tabs", so they're deliberately excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PersistedView {
    AllFics,
    ByStatus(ReadingStatus),
    Shelf(u64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeChoice {
    Dark,
    Clear,
    #[default]
    System,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Clear, ThemeChoice::Dark];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "System",
            ThemeChoice::Clear => "Clear",
            ThemeChoice::Dark => "Dark",
        }
    }

    fn preference(self) -> egui::ThemePreference {
        match self {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Clear => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// eframe only persists fullscreen for us, not maximized — and
    /// even fullscreen tracking is fragile across WMs — so we record both
    /// here and re-apply on the first frame.
    #[serde(default)]
    pub window_maximized: bool,
    #[serde(default)]
    pub window_fullscreen: bool,
    /// The library/shelf tab that was open when the app last quit, so it
    /// reopens the same one next launch.
    #[serde(default)]
    pub last_view: Option<PersistedView>,
    /// Global UI zoom factor (egui's `zoom_factor`). Clamped defensively
    /// against a hand-edited config file.
    #[serde(default = "default_text_zoom")]
    pub text_zoom: f32,
    #[serde(default)]
    pub theme: ThemeChoice,
    #[serde(default = "default_background_checks")]
    pub background_checks: bool,
    #[serde(default = "default_desktop_notifications")]
    pub desktop_notifications: bool,
    #[serde(default)]
    pub collapsed_series: Vec<u64>,
    #[serde(flatten)]
    pub shared: Config,
}

pub const TEXT_ZOOM_RANGE: std::ops::RangeInclusive<f32> = 0.5..=2.0;

fn default_background_checks() -> bool {
    true
}

fn default_desktop_notifications() -> bool {
    true
}

fn default_text_zoom() -> f32 {
    1.0
}

pub fn clamp_zoom(zoom: f32) -> f32 {
    if zoom.is_finite() {
        zoom.clamp(*TEXT_ZOOM_RANGE.start(), *TEXT_ZOOM_RANGE.end())
    } else {
        default_text_zoom()
    }
}

pub fn set_zoom(ctx: &egui::Context, zoom: f32) -> f32 {
    let clamped = clamp_zoom(zoom);
    ctx.set_zoom_factor(clamped);
    clamped
}

pub fn set_theme(ctx: &egui::Context, choice: ThemeChoice) {
    ctx.set_theme(choice.preference());
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
            last_view: None,
            collapsed_series: Vec::new(),
            text_zoom: 1.0,
            theme: ThemeChoice::System,
            background_checks: true,
            desktop_notifications: true,
            shared: Config::default(),
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        shared_config::load_file()
    }

    pub fn reorder_visible_column(&mut self, col: ColumnKey, target: ColumnKey, place_after: bool) {
        if col == target {
            return;
        }
        let Some(from) = self.visible_columns.iter().position(|c| *c == col) else {
            return;
        };
        if !self.visible_columns.contains(&target) {
            return;
        }
        self.visible_columns.remove(from);
        let to = self
            .visible_columns
            .iter()
            .position(|c| *c == target)
            .expect("target still present after removing col");
        self.visible_columns
            .insert(to + usize::from(place_after), col);
    }

    pub fn save(&self) -> io::Result<()> {
        shared_config::save_file(self)
    }
}
