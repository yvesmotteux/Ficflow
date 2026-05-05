//! In-memory caches the GUI's panels read from + the `mutate()`
//! funnel that keeps them coherent after fic-shelf-changing ops.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use crate::application::{
    count_fics_per_shelf::count_fics_per_shelf, list_fics::list_fics,
    list_shelf_fics::list_shelf_fics, list_shelves::list_shelves,
    list_shelves_for_fic::list_shelves_for_fic,
};
use crate::domain::fanfiction::Fanfiction;
use crate::domain::shelf::Shelf;
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;

use super::selection::Selection;
use super::view::View;

pub struct LibraryCache {
    pub fics: Vec<Fanfiction>,
    pub shelves: Vec<Shelf>,
    /// Fic ids in the active `View::Shelf(_)`; empty outside shelf views.
    pub shelf_members: HashSet<u64>,
    /// Shelf ids the `Selection::Single(_)` fic belongs to; empty otherwise.
    pub selection_shelf_ids: HashSet<u64>,
    /// Sidebar count per shelf; missing keys default to 0.
    pub shelf_counts: HashMap<u64, usize>,
}

impl LibraryCache {
    pub fn load(connection: &Connection) -> Self {
        Self {
            fics: load_fics_inner(connection),
            shelves: load_shelves_inner(connection),
            shelf_members: HashSet::new(),
            selection_shelf_ids: HashSet::new(),
            shelf_counts: count_fics_per_shelf_inner(connection),
        }
    }

    pub fn reload_fics(&mut self, connection: &Connection) {
        self.fics = load_fics_inner(connection);
    }

    pub fn reload_shelves(&mut self, connection: &Connection) {
        self.shelves = load_shelves_inner(connection);
    }

    pub fn refresh_shelf_members(
        &mut self,
        connection: &Connection,
        view: &View,
    ) -> Result<(), FicflowError> {
        self.shelf_members.clear();
        if let View::Shelf(id) = view {
            let repo = SqliteRepository::new(connection);
            self.shelf_members = list_shelf_fics(&repo, *id)?
                .into_iter()
                .map(|f| f.id)
                .collect();
        }
        Ok(())
    }

    pub fn refresh_selection_shelf_ids(
        &mut self,
        connection: &Connection,
        selection: &Selection,
    ) -> Result<(), FicflowError> {
        self.selection_shelf_ids.clear();
        if let Selection::Single(id) = selection {
            let repo = SqliteRepository::new(connection);
            self.selection_shelf_ids = list_shelves_for_fic(&repo, *id)?
                .into_iter()
                .map(|s| s.id)
                .collect();
        }
        Ok(())
    }

    /// Failures swallowed (empty map → every shelf shows 0) so a
    /// transient DB hiccup doesn't toast every frame.
    pub fn refresh_shelf_counts(&mut self, connection: &Connection) {
        self.shelf_counts = count_fics_per_shelf_inner(connection);
    }

    pub fn replace_fic(&mut self, updated: Fanfiction) {
        if let Some(slot) = self.fics.iter_mut().find(|f| f.id == updated.id) {
            *slot = updated;
        }
    }

    pub fn remove_fics(&mut self, ids: &[u64]) {
        self.fics.retain(|f| !ids.contains(&f.id));
    }

    /// Returns `(op_result, refresh_errors)` — refresh failures are
    /// surfaced separately so the caller can toast them without
    /// conflating with the op's own success/failure. Over-refreshing
    /// is intentional: invalidating everything every time is the
    /// hardest-to-break shape, and at a few-thousand-fic scale the
    /// extra queries are imperceptible.
    pub fn mutate<R>(
        &mut self,
        connection: &Connection,
        view: &View,
        selection: &Selection,
        op: impl FnOnce(&SqliteRepository<'_>) -> R,
    ) -> (R, Vec<FicflowError>) {
        let repo = SqliteRepository::new(connection);
        let result = op(&repo);
        let mut refresh_errors = Vec::new();
        if let Err(e) = self.refresh_selection_shelf_ids(connection, selection) {
            refresh_errors.push(e);
        }
        self.refresh_shelf_counts(connection);
        if matches!(view, View::Shelf(_)) {
            if let Err(e) = self.refresh_shelf_members(connection, view) {
                refresh_errors.push(e);
            }
        }
        (result, refresh_errors)
    }
}

fn load_fics_inner(connection: &Connection) -> Vec<Fanfiction> {
    let repo = SqliteRepository::new(connection);
    match list_fics(&repo) {
        Ok(fics) => fics,
        Err(err) => {
            log::error!("Failed to load fanfictions: {}", err);
            Vec::new()
        }
    }
}

fn load_shelves_inner(connection: &Connection) -> Vec<Shelf> {
    let repo = SqliteRepository::new(connection);
    match list_shelves(&repo) {
        Ok(shelves) => shelves,
        Err(err) => {
            log::error!("Failed to load shelves: {}", err);
            Vec::new()
        }
    }
}

fn count_fics_per_shelf_inner(connection: &Connection) -> HashMap<u64, usize> {
    let repo = SqliteRepository::new(connection);
    count_fics_per_shelf(&repo).unwrap_or_default()
}
