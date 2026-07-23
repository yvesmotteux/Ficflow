use egui::{Context, Window};

use crate::domain::fanfiction::Fanfiction;

const MAX_TITLES_SHOWN: usize = 5;

pub enum DeleteOutcome {
    None,
    Confirm(Vec<u64>),
    Cancel,
}

/// Confirms a bulk-delete. Lists the first few titles by way of "are
/// you sure", summarises the rest as "+ N more". Caller is responsible
/// for only invoking this when `ActiveModal::DeleteFics(_)` is the
/// current modal — the early-return guard is gone.
pub fn draw_delete_confirm(ctx: &Context, ids: &[u64], fics: &[Fanfiction]) -> DeleteOutcome {
    let total = ids.len();

    let mut still_open = true;
    let mut outcome = DeleteOutcome::None;
    Window::new("Delete fanfictions")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.label(format!("Delete {} fanfiction(s)?", total));
            ui.add_space(4.0);
            for id in ids.iter().take(MAX_TITLES_SHOWN) {
                let title = fics
                    .iter()
                    .find(|f| f.id == *id)
                    .map(|f| f.title.as_str())
                    .unwrap_or("(unknown)");
                ui.label(format!("• {}", title));
            }
            if total > MAX_TITLES_SHOWN {
                ui.label(
                    egui::RichText::new(format!("+ {} more", total - MAX_TITLES_SHOWN))
                        .italics()
                        .weak(),
                );
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    outcome = DeleteOutcome::Confirm(ids.to_vec());
                }
                if ui.button("Cancel").clicked() {
                    outcome = DeleteOutcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = DeleteOutcome::Cancel;
    }
    outcome
}

pub enum RemoveOrDeleteOutcome {
    None,
    RemoveFromShelf(Vec<u64>),
    DeleteEverywhere(Vec<u64>),
    Cancel,
}

pub fn draw_remove_or_delete(
    ctx: &Context,
    ids: &[u64],
    shelf_name: &str,
    fics: &[Fanfiction],
) -> RemoveOrDeleteOutcome {
    let total = ids.len();

    let mut still_open = true;
    let mut outcome = RemoveOrDeleteOutcome::None;
    Window::new("Remove or delete")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.label(format!("{} fanfiction(s) selected", total));
            ui.add_space(4.0);
            for id in ids.iter().take(MAX_TITLES_SHOWN) {
                let title = fics
                    .iter()
                    .find(|f| f.id == *id)
                    .map(|f| f.title.as_str())
                    .unwrap_or("(unknown)");
                ui.label(format!("• {}", title));
            }
            if total > MAX_TITLES_SHOWN {
                ui.label(
                    egui::RichText::new(format!("+ {} more", total - MAX_TITLES_SHOWN))
                        .italics()
                        .weak(),
                );
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Removing keeps them in Ficflow; deleting removes them everywhere.",
                )
                .italics()
                .weak(),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .button(format!("Remove from \"{}\"", shelf_name))
                    .clicked()
                {
                    outcome = RemoveOrDeleteOutcome::RemoveFromShelf(ids.to_vec());
                }
                if ui.button("Delete from Ficflow").clicked() {
                    outcome = RemoveOrDeleteOutcome::DeleteEverywhere(ids.to_vec());
                }
                if ui.button("Cancel").clicked() {
                    outcome = RemoveOrDeleteOutcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = RemoveOrDeleteOutcome::Cancel;
    }
    outcome
}
