//! In-memory caches the GUI's panels read from + the `mutate()`
//! funnel that keeps them coherent after fic-shelf-changing ops.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::Connection;

use crate::application::{
    count_fics_per_shelf::count_fics_per_shelf, list_fics::list_fics, list_inbox::list_inbox,
    list_series::list_series, list_shelf_fics::list_shelf_fics, list_shelves::list_shelves,
    list_shelves_for_fic::list_shelves_for_fic,
};
use crate::domain::fanfiction::Fanfiction;
use crate::domain::inbox::InboxMessage;
use crate::domain::series::Series;
use crate::domain::shelf::{Shelf, ShelfKind};
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;

use super::auto_shelf::{self, DistinctValues};
use super::selection::Selection;
use super::view::View;

pub struct LibraryCache {
    pub fics: Vec<Fanfiction>,
    pub series: Vec<Series>,
    pub shelves: Vec<Shelf>,
    /// Fic ids in the active `View::Shelf(_)`; empty outside shelf views.
    pub shelf_members: HashSet<u64>,
    /// Shelf ids the `Selection::Single(_)` fic belongs to; empty otherwise.
    pub selection_shelf_ids: HashSet<u64>,
    /// Sidebar count per shelf; missing keys default to 0.
    pub shelf_counts: HashMap<u64, usize>,
    pub inbox: Vec<InboxMessage>,
    /// Live-computed membership for every auto-shelf, keyed by shelf id.
    /// Never backed by `fic_shelf` rows — recomputed from `fics`/`shelves`
    /// on every mutation, same as `shelf_counts`.
    pub auto_shelf_members: HashMap<u64, HashSet<u64>>,
    /// Changes on every change to `fics` or `shelf_members`, so per-frame
    /// views can tell whether derived data needs recomputing. Drawn from a
    /// process-wide counter so two caches never share a value.
    pub generation: u64,
    distinct_values: Option<(u64, DistinctValues)>,
}

impl LibraryCache {
    pub fn load(connection: &Connection) -> Self {
        let fics = load_fics_inner(connection);
        let shelves = load_shelves_inner(connection);
        let auto_shelf_members = compute_auto_shelf_members(&fics, &shelves);
        let mut shelf_counts = count_fics_per_shelf_inner(connection);
        overlay_auto_shelf_counts(&mut shelf_counts, &auto_shelf_members);
        Self {
            fics,
            series: load_series_inner(connection),
            shelves,
            shelf_members: HashSet::new(),
            selection_shelf_ids: HashSet::new(),
            shelf_counts,
            inbox: load_inbox_inner(connection),
            auto_shelf_members,
            generation: next_generation(),
            distinct_values: None,
        }
    }

    pub fn fic(&self, id: u64) -> Option<&Fanfiction> {
        self.fics.iter().find(|f| f.id == id)
    }

    pub fn series(&self, id: u64) -> Option<&Series> {
        self.series.iter().find(|s| s.id == id)
    }

    pub fn shelf(&self, id: u64) -> Option<&Shelf> {
        self.shelves.iter().find(|s| s.id == id)
    }

    /// Autocomplete suggestions for the auto-shelf builder, rebuilt only
    /// when the library changes.
    pub fn distinct_values(&mut self) -> &DistinctValues {
        if self
            .distinct_values
            .as_ref()
            .is_some_and(|(generation, _)| *generation != self.generation)
        {
            self.distinct_values = None;
        }
        let (_, values) = self.distinct_values.get_or_insert_with(|| {
            (
                self.generation,
                auto_shelf::build_distinct_values(&self.fics),
            )
        });
        values
    }

    pub fn reload_fics(&mut self, connection: &Connection) {
        self.fics = load_fics_inner(connection);
        self.series = load_series_inner(connection);
        self.refresh_auto_shelf_members();
    }

    pub fn reload_inbox(&mut self, connection: &Connection) {
        self.inbox = load_inbox_inner(connection);
    }

    pub fn unread_inbox_count(&self) -> usize {
        self.inbox.iter().filter(|m| m.is_unread()).count()
    }

    pub fn reload_shelves(&mut self, connection: &Connection) {
        self.shelves = load_shelves_inner(connection);
        self.refresh_auto_shelf_members();
    }

    /// Pure — recomputes `auto_shelf_members` (and the counts derived
    /// from it) from `self.fics`/`self.shelves`. Called after every
    /// mutation of either, never per-frame. Overlaying counts here too
    /// (not only in `refresh_shelf_counts`) matters because call sites
    /// like `replace_fic` (e.g. a status change) update membership
    /// without going through `refresh_shelf_counts`.
    pub fn refresh_auto_shelf_members(&mut self) {
        self.generation = next_generation();
        self.auto_shelf_members = compute_auto_shelf_members(&self.fics, &self.shelves);
        overlay_auto_shelf_counts(&mut self.shelf_counts, &self.auto_shelf_members);
    }

    pub fn refresh_shelf_members(
        &mut self,
        connection: &Connection,
        view: &View,
    ) -> Result<(), FicflowError> {
        self.generation = next_generation();
        self.shelf_members.clear();
        if let View::Shelf(id) = view {
            match self.shelf(*id).map(|s| &s.kind) {
                Some(ShelfKind::Auto(_)) => {
                    self.shelf_members =
                        self.auto_shelf_members.get(id).cloned().unwrap_or_default();
                }
                _ => {
                    let repo = SqliteRepository::new(connection);
                    self.shelf_members = list_shelf_fics(&repo, *id)?
                        .into_iter()
                        .map(|f| f.id)
                        .collect();
                }
            }
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
        overlay_auto_shelf_counts(&mut self.shelf_counts, &self.auto_shelf_members);
    }

    pub fn replace_fic(&mut self, updated: Fanfiction) {
        self.replace_fics(vec![updated]);
    }

    /// Batch form of `replace_fic` — updates every entry first, then
    /// recomputes auto-shelf membership once instead of once per fic
    /// (bulk operations like `bulk_set_status` can touch dozens of fics
    /// at a time, and each recompute is a full shelves × fics scan).
    pub fn replace_fics(&mut self, updated: Vec<Fanfiction>) {
        for updated in updated {
            if let Some(slot) = self.fics.iter_mut().find(|f| f.id == updated.id) {
                *slot = updated;
            }
        }
        self.refresh_auto_shelf_members();
    }

    pub fn remove_fics(&mut self, ids: &[u64]) {
        self.fics.retain(|f| !ids.contains(&f.id));
        self.refresh_auto_shelf_members();
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
        if matches!(view, View::Shelf(_))
            && let Err(e) = self.refresh_shelf_members(connection, view)
        {
            refresh_errors.push(e);
        }
        (result, refresh_errors)
    }
}

fn compute_auto_shelf_members(
    fics: &[Fanfiction],
    shelves: &[Shelf],
) -> HashMap<u64, HashSet<u64>> {
    shelves
        .iter()
        .filter_map(|s| match &s.kind {
            ShelfKind::Auto(criteria) => Some((
                s.id,
                fics.iter()
                    .filter(|f| criteria.matches(f))
                    .map(|f| f.id)
                    .collect(),
            )),
            ShelfKind::Normal => None,
        })
        .collect()
}

fn overlay_auto_shelf_counts(
    shelf_counts: &mut HashMap<u64, usize>,
    auto_shelf_members: &HashMap<u64, HashSet<u64>>,
) {
    for (id, members) in auto_shelf_members {
        shelf_counts.insert(*id, members.len());
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

fn load_series_inner(connection: &Connection) -> Vec<Series> {
    let repo = SqliteRepository::new(connection);
    list_series(&repo).unwrap_or_else(|err| {
        log::error!("Failed to load series: {}", err);
        Vec::new()
    })
}

fn load_inbox_inner(connection: &Connection) -> Vec<InboxMessage> {
    let repo = SqliteRepository::new(connection);
    match list_inbox(&repo) {
        Ok(messages) => messages,
        Err(err) => {
            log::error!("Failed to load inbox: {}", err);
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

fn next_generation() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::Relaxed)
}
