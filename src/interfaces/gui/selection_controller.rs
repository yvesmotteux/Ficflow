//! Selection state + the ctrl/shift row-click resolver.

use std::collections::HashSet;

use crate::domain::fanfiction::Fanfiction;

use super::selection::Selection;
use super::view::View;

#[derive(Default)]
pub struct SelectionController {
    selection: Selection,
    /// Anchor for shift-click range selection. Preserved across
    /// successive shift-clicks so they all anchor to the same row.
    last_clicked_id: Option<u64>,
}

impl SelectionController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self) -> &Selection {
        &self.selection
    }

    pub fn contains(&self, id: u64) -> bool {
        self.selection.contains(id)
    }

    pub fn ids_vec(&self) -> Vec<u64> {
        match &self.selection {
            Selection::None => Vec::new(),
            Selection::Single(id) => vec![*id],
            Selection::Multi(ids) => ids.clone(),
        }
    }

    pub fn select_single(&mut self, id: u64) {
        self.selection = Selection::Single(id);
        self.last_clicked_id = Some(id);
    }

    pub fn select_many(&mut self, ids: &[u64]) {
        self.last_clicked_id = ids.last().copied();
        self.selection = ids.to_vec().into();
    }

    pub fn clear(&mut self) {
        self.selection = Selection::None;
        self.last_clicked_id = None;
    }

    /// `mods` from the click itself so ctrl/shift work right across
    /// platforms (`Modifiers::command` is Cmd on macOS, Ctrl elsewhere).
    pub fn handle_row_click(
        &mut self,
        visible: &[&Fanfiction],
        clicked_idx: usize,
        mods: egui::Modifiers,
    ) {
        let clicked_id = visible[clicked_idx].id;

        if mods.shift {
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
            // Don't move the anchor — keeps successive shift-clicks consistent.
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

    /// Returns true when the selection actually changed.
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
