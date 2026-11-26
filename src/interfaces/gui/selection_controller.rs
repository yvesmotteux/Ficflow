//! Selection state + the ctrl/shift row-click resolver.

use std::collections::HashSet;

use crate::domain::fanfiction::Fanfiction;

use super::selection::{RowKey, Selection};
use super::view::View;

#[derive(Default)]
pub struct SelectionController {
    selection: Selection,
    /// Anchor for shift-click range selection. Preserved across
    /// successive shift-clicks so they all anchor to the same row.
    last_clicked: Option<RowKey>,
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

    pub fn shows(&self, key: RowKey) -> bool {
        self.selection.shows(key)
    }

    pub fn ids_vec(&self) -> Vec<u64> {
        match &self.selection {
            Selection::Single(id) => vec![*id],
            Selection::Multi(ids) => ids.clone(),
            _ => Vec::new(),
        }
    }

    pub fn select_single(&mut self, id: u64) {
        self.select_row(RowKey::Fic(id));
    }

    pub fn select_row(&mut self, key: RowKey) {
        self.selection = key.into();
        self.last_clicked = Some(key);
    }

    pub fn select_many(&mut self, ids: &[u64]) {
        self.last_clicked = ids.last().copied().map(RowKey::Fic);
        self.selection = ids.to_vec().into();
    }

    pub fn clear(&mut self) {
        self.selection = Selection::None;
        self.last_clicked = None;
    }

    pub fn step(&mut self, visible: &[RowKey], down: bool) -> Option<usize> {
        let last = visible.len().checked_sub(1)?;
        let current = self
            .last_clicked
            .and_then(|key| visible.iter().position(|k| *k == key));
        let idx = match current {
            Some(i) if down => (i + 1).min(last),
            Some(i) => i.saturating_sub(1),
            None if down => 0,
            None => last,
        };
        self.select_row(visible[idx]);
        Some(idx)
    }

    /// `mods` from the click itself so ctrl/shift work right across
    /// platforms (`Modifiers::command` is Cmd on macOS, Ctrl elsewhere).
    pub fn handle_row_click(
        &mut self,
        visible: &[RowKey],
        clicked_idx: usize,
        mods: egui::Modifiers,
    ) {
        let clicked = visible[clicked_idx];
        let RowKey::Fic(clicked_id) = clicked else {
            self.select_row(clicked);
            return;
        };

        if mods.shift {
            let anchor_idx = self
                .last_clicked
                .and_then(|key| visible.iter().position(|k| *k == key))
                .unwrap_or(clicked_idx);
            let range = anchor_idx.min(clicked_idx)..=anchor_idx.max(clicked_idx);
            let mut ids: Vec<u64> = Vec::new();
            for key in &visible[range] {
                if let RowKey::Fic(id) = key
                    && !ids.contains(id)
                {
                    ids.push(*id);
                }
            }
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
            self.last_clicked = Some(clicked);
        } else {
            self.select_row(clicked);
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

        let row_selected = matches!(
            self.selection,
            Selection::Series(_) | Selection::Missing { .. }
        );
        if !view.shows_library() {
            self.clear();
        } else if !row_selected {
            let visible_ids: Vec<u64> = self
                .ids_vec()
                .into_iter()
                .filter(|id| {
                    fics.iter()
                        .any(|f| f.id == *id && view.includes(f, shelf_members))
                })
                .collect();
            self.selection = visible_ids.into();
            if matches!(self.selection, Selection::None) {
                self.last_clicked = None;
            }
        }

        self.selection != before
    }
}
