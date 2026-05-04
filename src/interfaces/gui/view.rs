use std::collections::HashSet;

use crate::domain::fanfiction::{Fanfiction, ReadingStatus};
use crate::domain::shelf::Shelf;

/// Which view the user is looking at — drives the header title and the
/// center panel's content + filtering.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum View {
    #[default]
    AllFics,
    ByStatus(ReadingStatus),
    Shelf(u64),
    Tasks,
    Settings,
}

impl View {
    /// Header title for the current view. For shelf views we look up the
    /// name from the loaded shelf list; if the shelf disappeared (deleted
    /// behind our back), fall back to a generic label.
    pub fn header_title(&self, shelves: &[Shelf]) -> String {
        match self {
            View::AllFics => "ALL FICTIONS".to_string(),
            View::ByStatus(status) => match status {
                ReadingStatus::InProgress => "IN PROGRESS".to_string(),
                ReadingStatus::Read => "READ".to_string(),
                ReadingStatus::PlanToRead => "PLAN TO READ".to_string(),
                ReadingStatus::Paused => "PAUSED".to_string(),
                ReadingStatus::Abandoned => "ABANDONED".to_string(),
            },
            View::Shelf(id) => shelves
                .iter()
                .find(|s| s.id == *id)
                .map(|s| s.name.to_uppercase())
                .unwrap_or_else(|| "SHELF".to_string()),
            View::Tasks => "TASKS".to_string(),
            View::Settings => "SETTINGS".to_string(),
        }
    }

    /// Whether a given fic should appear in the table when this view is active.
    /// `shelf_members` is consulted only for `View::Shelf(_)` and is expected
    /// to be the cached id-set the app maintains when a shelf view is selected.
    pub fn includes(&self, fic: &Fanfiction, shelf_members: &HashSet<u64>) -> bool {
        match self {
            View::AllFics => true,
            View::ByStatus(status) => fic.reading_status == *status,
            View::Shelf(_) => shelf_members.contains(&fic.id),
            View::Tasks | View::Settings => false,
        }
    }

    /// True when this view shows the library table at all (vs. a stub page).
    pub fn shows_library(&self) -> bool {
        matches!(self, View::AllFics | View::ByStatus(_) | View::Shelf(_))
    }
}
