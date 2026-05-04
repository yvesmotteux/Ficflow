use egui::{RichText, Ui};
use egui_notify::Toasts;
use rusqlite::Connection;

use crate::application::{
    add_to_shelf::add_to_shelf, remove_from_shelf::remove_from_shelf,
    update_status::update_reading_status,
};
use crate::domain::fanfiction::{Fanfiction, ReadingStatus};
use crate::domain::shelf::Shelf;
use crate::infrastructure::SqliteRepository;

use super::super::selection::Selection;
use super::super::view::View;

pub struct SelectionBarState<'a> {
    pub selection: &'a mut Selection,
    pub fics: &'a mut Vec<Fanfiction>,
    pub conn: &'a Connection,
    pub toasts: &'a mut Toasts,
    pub current_view: &'a View,
    pub all_shelves: &'a [Shelf],
    /// Set by a click on "Delete" — caller opens a confirm modal next frame.
    pub delete_pending: &'a mut Option<Vec<u64>>,
}

/// Returns `true` if any fic-shelf link changed (caller refreshes caches).
pub fn draw(ui: &mut Ui, state: SelectionBarState<'_>) -> bool {
    let SelectionBarState {
        selection,
        fics,
        conn,
        toasts,
        current_view,
        all_shelves,
        delete_pending,
    } = state;

    let ids = match selection {
        Selection::Multi(ids) => ids.clone(),
        Selection::Single(id) => vec![*id],
        Selection::None => return false,
    };

    let mut shelves_changed = false;
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{} selected", ids.len())).strong());
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
                    bulk_update_status(fics, &ids, status, conn, toasts);
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
                        bulk_add_to_shelf(&ids, shelf.id, conn, toasts);
                        shelves_changed = true;
                        ui.close_menu();
                    }
                }
            }
        });

        // "Remove from shelf" only makes sense when looking at a shelf.
        if let View::Shelf(shelf_id) = current_view {
            if ui.button("Remove from shelf").clicked() {
                bulk_remove_from_shelf(&ids, *shelf_id, conn, toasts);
                shelves_changed = true;
            }
        }

        // 🗑 — egui's NotoEmoji subset and the system DejaVu/Noto Symbols
        // fallback both cover U+1F5D1, so this renders without tofu on
        // every platform we support. Hover text keeps screen-reader and
        // discoverability behaviour intact.
        if ui.button("\u{1F5D1}").on_hover_text("Delete").clicked() {
            *delete_pending = Some(ids.clone());
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Clear selection").clicked() {
                *selection = Selection::None;
            }
        });
    });
    ui.add_space(4.0);

    shelves_changed
}

fn bulk_update_status(
    fics: &mut [Fanfiction],
    ids: &[u64],
    new_status: ReadingStatus,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let repo = SqliteRepository::new(conn);
    let payload = status_payload(new_status);
    let mut errors = 0usize;
    for id in ids {
        match update_reading_status(&repo, *id, payload) {
            Ok(updated) => {
                if let Some(slot) = fics.iter_mut().find(|f| f.id == *id) {
                    *slot = updated;
                }
            }
            Err(_) => errors += 1,
        }
    }
    report(toasts, ids.len(), errors, "Status updated");
}

fn bulk_add_to_shelf(ids: &[u64], shelf_id: u64, conn: &Connection, toasts: &mut Toasts) {
    let repo = SqliteRepository::new(conn);
    let mut errors = 0usize;
    for id in ids {
        if add_to_shelf(&repo, *id, shelf_id).is_err() {
            errors += 1;
        }
    }
    report(toasts, ids.len(), errors, "Added to shelf");
}

fn bulk_remove_from_shelf(ids: &[u64], shelf_id: u64, conn: &Connection, toasts: &mut Toasts) {
    let repo = SqliteRepository::new(conn);
    let mut errors = 0usize;
    for id in ids {
        if remove_from_shelf(&repo, *id, shelf_id).is_err() {
            errors += 1;
        }
    }
    report(toasts, ids.len(), errors, "Removed from shelf");
}

fn report(toasts: &mut Toasts, attempted: usize, errors: usize, success_action: &str) {
    if errors == 0 {
        toasts.success(format!("{}: {} fanfictions", success_action, attempted));
    } else if errors == attempted {
        toasts.error(format!("All {} updates failed", attempted));
    } else {
        toasts.error(format!("{}/{} updates failed", errors, attempted));
    }
}

fn format_status(status: &ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "In Progress",
        ReadingStatus::Read => "Read",
        ReadingStatus::PlanToRead => "Plan to Read",
        ReadingStatus::Paused => "Paused",
        ReadingStatus::Abandoned => "Abandoned",
    }
}

fn status_payload(status: ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "inprogress",
        ReadingStatus::Read => "read",
        ReadingStatus::PlanToRead => "plantoread",
        ReadingStatus::Paused => "paused",
        ReadingStatus::Abandoned => "abandoned",
    }
}
