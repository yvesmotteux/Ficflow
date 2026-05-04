//! In-memory cache shared across the GUI's panels.
//!
//! `LibraryCache` owns the five caches that used to live as loose
//! fields on `FicflowApp` (`fics`, `shelves`, `shelf_members`,
//! `selection_shelf_ids`, `shelf_counts`) plus the `mutate()` funnel
//! that keeps them coherent after any operation that could change
//! fic-shelf membership. Pulling them out collapses one cluster of
//! invariants — every cache lives in one place and every refresh
//! shape lives in one method.
//!
//! Errors from refresh ops bubble back to the caller as `Result`
//! (rather than being toasted in-place) so this module stays free
//! of `egui_notify::Toasts`. `FicflowApp` wraps each refresh with
//! its own toast-on-error glue.

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
    /// All non-deleted fics, in `list_fics` order.
    pub fics: Vec<Fanfiction>,
    /// All non-deleted shelves.
    pub shelves: Vec<Shelf>,
    /// Fic ids belonging to the currently-active shelf view. Empty
    /// (and unused) outside of `View::Shelf(_)`.
    pub shelf_members: HashSet<u64>,
    /// Shelf ids the currently-selected fic belongs to. Empty unless
    /// the selection is `Single(_)`.
    pub selection_shelf_ids: HashSet<u64>,
    /// Per-shelf fic counts shown in the sidebar. Shelves with zero
    /// non-deleted fics are absent from the map; the sidebar defaults
    /// missing keys to 0.
    pub shelf_counts: HashMap<u64, usize>,
}

impl LibraryCache {
    /// Initial load. `fics` and `shelves` come from `list_fics` /
    /// `list_shelves`; `shelf_counts` from one aggregated GROUP BY.
    /// `shelf_members` and `selection_shelf_ids` start empty and get
    /// filled by `refresh_shelf_members` / `refresh_selection_shelf_ids`
    /// once the caller has a view + selection to pin them to.
    pub fn load(connection: &Connection) -> Self {
        Self {
            fics: load_fics_inner(connection),
            shelves: load_shelves_inner(connection),
            shelf_members: HashSet::new(),
            selection_shelf_ids: HashSet::new(),
            shelf_counts: count_fics_per_shelf_inner(connection),
        }
    }

    /// Re-fetch the full fic list. Used after a worker thread has
    /// added or refreshed a fic — the worker mutates the DB
    /// directly, so the in-memory cache catches up via this.
    pub fn reload_fics(&mut self, connection: &Connection) {
        self.fics = load_fics_inner(connection);
    }

    /// Re-fetch the full shelf list. Used after create_shelf /
    /// delete_shelf since those return the new full state via the
    /// next reload rather than incremental patching.
    pub fn reload_shelves(&mut self, connection: &Connection) {
        self.shelves = load_shelves_inner(connection);
    }

    /// Recompute the shelf-membership set for the active view. Empty
    /// outside of `View::Shelf(_)`. On error, leaves the field
    /// cleared (caller reports the error to the user).
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

    /// Recompute which shelves the currently-selected fic belongs
    /// to. Empty unless the selection is `Single(_)`. On error,
    /// leaves the field cleared.
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

    /// Recompute the sidebar's per-shelf fic counts. Failures are
    /// silently swallowed (an empty map → every shelf renders as
    /// 0) — a transient DB hiccup shouldn't replace the whole
    /// sidebar with an error toast on every frame.
    pub fn refresh_shelf_counts(&mut self, connection: &Connection) {
        self.shelf_counts = count_fics_per_shelf_inner(connection);
    }

    /// Replace the entry with the same id in `fics` with `updated`.
    /// No-op if no entry matches (transient cache/DB drift; the next
    /// `reload_fics` heals it).
    pub fn replace_fic(&mut self, updated: Fanfiction) {
        if let Some(slot) = self.fics.iter_mut().find(|f| f.id == updated.id) {
            *slot = updated;
        }
    }

    /// Drop fics with the given ids from the in-memory list. Used
    /// after a soft-delete batch — the DB rows are gone, the cache
    /// has to follow.
    pub fn remove_fics(&mut self, ids: &[u64]) {
        self.fics.retain(|f| !ids.contains(&f.id));
    }

    /// Mutation funnel: run `op` against a fresh `SqliteRepository`,
    /// then refresh every cache that a fic-shelf-link change could
    /// invalidate. Returns `(op_result, refresh_errors)` — the op's
    /// own success/failure is buried in the first slot; the second
    /// slot is *only* errors raised while re-reading caches after
    /// the op (rare and recover-on-next-call), separated out so the
    /// caller can toast them without conflating them with the op's
    /// own outcome. Over-refreshing is intentional — a single funnel
    /// that always invalidates everything is the hardest-to-break
    /// shape, and at a few-thousand-fic scale the extra queries are
    /// imperceptible.
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
