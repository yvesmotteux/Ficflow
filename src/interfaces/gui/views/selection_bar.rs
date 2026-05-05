//! Bulk-action bar shown at the bottom of the central panel whenever
//! one or more fics are selected. Pure presentation: it reads the
//! selected fic count + view + shelf list, and emits a single
//! `Outcome` describing what the user clicked. The caller (`app.rs`'s
//! render dispatcher) routes that outcome through `FicflowApp`'s
//! control-surface methods. No `&Connection`, no `&mut Toasts`, no
//! direct calls into `application::*`.

use egui::{RichText, Ui};

use crate::domain::fanfiction::ReadingStatus;
use crate::domain::shelf::Shelf;

use super::super::format::format_status;
use super::super::view::View;

pub struct SelectionBarState<'a> {
    /// Read-only view of the selection's IDs (empty → bar isn't drawn).
    pub selection_ids: &'a [u64],
    pub current_view: &'a View,
    pub all_shelves: &'a [Shelf],
}

/// What the user clicked this frame. At most one outcome per frame —
/// the bar's controls are mutually-exclusive within a single click.
pub enum Outcome {
    None,
    SetStatus(ReadingStatus),
    AddToShelf(u64),
    /// Only emitted when the active view is a shelf view.
    RemoveFromShelf(u64),
    /// User clicked the trash. Caller opens the confirm modal.
    RequestDelete,
    ClearSelection,
}

pub fn draw(ui: &mut Ui, state: SelectionBarState<'_>) -> Outcome {
    let SelectionBarState {
        selection_ids,
        current_view,
        all_shelves,
    } = state;

    let mut outcome = Outcome::None;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} selected", selection_ids.len())).strong());
        ui.separator();

        ui.menu_button("Change status", |ui| {
            for status in [
                ReadingStatus::InProgress,
                ReadingStatus::Read,
                ReadingStatus::PlanToRead,
                ReadingStatus::Paused,
                ReadingStatus::Abandoned,
            ] {
                if ui.button(format_status(&status)).clicked() {
                    outcome = Outcome::SetStatus(status);
                    ui.close_menu();
                }
            }
        });

        ui.menu_button("Add to shelf", |ui| {
            if all_shelves.is_empty() {
                ui.label(RichText::new("(no shelves yet)").italics().weak());
            } else {
                for shelf in all_shelves {
                    if ui.button(&shelf.name).clicked() {
                        outcome = Outcome::AddToShelf(shelf.id);
                        ui.close_menu();
                    }
                }
            }
        });

        // "Remove from shelf" only makes sense when looking at a shelf.
        if let View::Shelf(shelf_id) = current_view {
            if ui.button("Remove from shelf").clicked() {
                outcome = Outcome::RemoveFromShelf(*shelf_id);
            }
        }

        // 🗑 — egui's NotoEmoji subset and the system DejaVu/Noto Symbols
        // fallback both cover U+1F5D1, so this renders without tofu on
        // every platform we support. Hover text keeps screen-reader and
        // discoverability behaviour intact.
        if ui.button("\u{1F5D1}").on_hover_text("Delete").clicked() {
            outcome = Outcome::RequestDelete;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear selection").clicked() {
                outcome = Outcome::ClearSelection;
            }
        });
    });
    ui.add_space(4.0);

    outcome
}
