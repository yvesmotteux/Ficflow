//! Library-table selection state + the row-click resolver.
//!
//! `SelectionController` owns the `Selection` value plus the
//! shift-click anchor (`last_clicked_id`), and centralises every
//! transition between selection states. Before this lived in two
//! places: `FicflowApp` had `select_fic` / `select_fics` /
//! `clear_selection` / `prune_selection_to_view`, and `library_view`
//! had its own `handle_row_click` that mutated the `&mut Selection`
//! reference passed in. Pulling the row-click resolver into the
//! controller means there's exactly one module that knows how
//! ctrl/shift modifiers map to selection deltas.

use std::collections::HashSet;

use crate::domain::fanfiction::Fanfiction;

use super::selection::Selection;
use super::view::View;

#[derive(Default)]
pub struct SelectionController {
    selection: Selection,
    /// Anchor row id for shift-click range selection. Set on plain
    /// and ctrl-clicks; preserved across shift-click extensions so
    /// successive shift-clicks all anchor to the same row.
    last_clicked_id: Option<u64>,
}

impl SelectionController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> &Selection {
        &self.selection
    }

    /// True when the given fic id is part of the current selection.
    pub fn contains(&self, id: u64) -> bool {
        self.selection.contains(id)
    }

    /// Flatten the selection to a `Vec<u64>` in selection order.
    /// Used by the drag-payload code (multi-selection drags carry the
    /// whole set) and by bulk-action dispatch (selection bar).
    pub fn ids_vec(&self) -> Vec<u64> {
        match &self.selection {
            Selection::None => Vec::new(),
            Selection::Single(id) => vec![*id],
            Selection::Multi(ids) => ids.clone(),
        }
    }

    /// Mark a single fic as the selection. Mirrors a plain
    /// (no-modifier) row click.
    pub fn select_single(&mut self, id: u64) {
        self.selection = Selection::Single(id);
        self.last_clicked_id = Some(id);
    }

    /// Set the selection to the given list. Variant chosen by length
    /// (empty → None, one → Single, more → Multi). Used by Ctrl+A
    /// (select-all-visible) and the test harness.
    pub fn select_many(&mut self, ids: &[u64]) {
        self.last_clicked_id = ids.last().copied();
        self.selection = ids.to_vec().into();
    }

    /// Drop the selection and the shift-click anchor.
    pub fn clear(&mut self) {
        self.selection = Selection::None;
        self.last_clicked_id = None;
    }

    /// Resolve a click on a visible row into a new selection state.
    /// `mods` come from the click itself so we honour ctrl/shift
    /// correctly across platforms (egui's `Modifiers::command` maps
    /// to Cmd on macOS and Ctrl elsewhere).
    pub fn handle_row_click(
        &mut self,
        visible: &[&Fanfiction],
        clicked_idx: usize,
        mods: egui::Modifiers,
    ) {
        let clicked_id = visible[clicked_idx].id;

        if mods.shift {
            // Range select between anchor and clicked row.
            let anchor_id = self.last_clicked_id.unwrap_or(clicked_id);
            let anchor_idx = visible
                .iter()
                .position(|f| f.id == anchor_id)
                .unwrap_or(clicked_idx);
            let (start, end) = if anchor_idx <= clicked_idx {
                (anchor_idx, clicked_idx)
            } else {
                (clicked_idx, anchor_idx)
            };
            let ids: Vec<u64> = visible[start..=end].iter().map(|f| f.id).collect();
            self.selection = ids.into();
            // Anchor stays put across consecutive shift-clicks.
        } else if mods.command {
            let mut current = self.ids_vec();
            if let Some(pos) = current.iter().position(|&id| id == clicked_id) {
                current.remove(pos);
            } else {
                current.push(clicked_id);
            }
            self.selection = current.into();
            self.last_clicked_id = Some(clicked_id);
        } else {
            self.selection = Selection::Single(clicked_id);
            self.last_clicked_id = Some(clicked_id);
        }
    }

    /// Drop selected ids that aren't visible under `view` + the
    /// associated `shelf_members` filter; on a non-library view the
    /// selection is cleared entirely (Tasks/Settings have no rows).
    /// Returns true when the selection actually changed — caller
    /// uses the bool to decide whether to refresh derived caches.
    pub fn prune_to_view(
        &mut self,
        fics: &[Fanfiction],
        view: &View,
        shelf_members: &HashSet<u64>,
    ) -> bool {
        let before = self.selection.clone();

        if !view.shows_library() {
            self.clear();
        } else {
            let visible_ids: Vec<u64> = match &self.selection {
                Selection::None => Vec::new(),
                Selection::Single(id) => fics
                    .iter()
                    .find(|f| f.id == *id)
                    .filter(|f| view.includes(f, shelf_members))
                    .map(|f| f.id)
                    .into_iter()
                    .collect(),
                Selection::Multi(ids) => ids
                    .iter()
                    .filter_map(|id| {
                        fics.iter()
                            .find(|f| f.id == *id)
                            .filter(|f| view.includes(f, shelf_members))
                            .map(|f| f.id)
                    })
                    .collect(),
            };
            self.selection = visible_ids.into();
            if matches!(self.selection, Selection::None) {
                self.last_clicked_id = None;
            }
        }

        self.selection != before
    }
}
