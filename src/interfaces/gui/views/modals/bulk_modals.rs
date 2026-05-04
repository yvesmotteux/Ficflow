use egui::{Context, Window};

use crate::domain::fanfiction::Fanfiction;

const MAX_TITLES_SHOWN: usize = 5;

pub enum DeleteOutcome {
    None,
    Confirm(Vec<u64>),
    Cancel,
}

/// Confirms a bulk-delete. Lists the first few titles by way of "are you sure",
/// summarises the rest as "+ N more". `pending` is cleared on Confirm or Cancel.
pub fn draw_delete_confirm(
    ctx: &Context,
    pending: &mut Option<Vec<u64>>,
    fics: &[Fanfiction],
) -> DeleteOutcome {
    let Some(ids) = pending.as_ref() else {
        return DeleteOutcome::None;
    };
    let id_list = ids.clone();
    let total = id_list.len();

    let mut still_open = true;
    let mut outcome = DeleteOutcome::None;
    Window::new("Delete fanfictions")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.label(format!("Delete {} fanfiction(s)?", total));
            ui.add_space(4.0);
            for id in id_list.iter().take(MAX_TITLES_SHOWN) {
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
                    outcome = DeleteOutcome::Confirm(id_list.clone());
                }
                if ui.button("Cancel").clicked() {
                    outcome = DeleteOutcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = DeleteOutcome::Cancel;
    }
    if matches!(outcome, DeleteOutcome::Confirm(_) | DeleteOutcome::Cancel) {
        *pending = None;
    }
    outcome
}
