use egui::{Context, Window};

use crate::domain::shelf::Shelf;

#[derive(Default)]
pub struct CreateState {
    pub name: String,
}

pub enum Outcome {
    None,
    Submit(String),
    Cancel,
}

pub fn draw_create(ctx: &Context, state: &mut CreateState) -> Outcome {
    let mut still_open = true;
    let mut outcome = Outcome::None;
    Window::new("Create shelf")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.label("Name:");
            let resp = ui.text_edit_singleline(&mut state.name);
            // Auto-focus the field the first frame so users can type immediately.
            if !resp.has_focus() && state.name.is_empty() {
                resp.request_focus();
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let submit_enabled = !state.name.trim().is_empty();
                let create_clicked = ui
                    .add_enabled(submit_enabled, egui::Button::new("Create"))
                    .clicked();
                let pressed_enter = resp.lost_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                    && submit_enabled;
                if create_clicked || pressed_enter {
                    outcome = Outcome::Submit(state.name.trim().to_string());
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = Outcome::Cancel;
    }
    outcome
}

pub enum DeleteOutcome {
    None,
    Confirm(u64),
    Cancel,
}

/// Draws the delete-shelf confirmation modal. Caller is responsible
/// for only invoking this when `ActiveModal::DeleteShelf(_)` is the
/// current modal — the early-return guard is gone.
pub fn draw_delete_confirm(ctx: &Context, shelf_id: u64, shelves: &[Shelf]) -> DeleteOutcome {
    let shelf_name = shelves
        .iter()
        .find(|s| s.id == shelf_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("(id {})", shelf_id));

    let mut still_open = true;
    let mut outcome = DeleteOutcome::None;
    Window::new("Delete shelf")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.label(format!("Delete shelf \u{201C}{}\u{201D}?", shelf_name));
            ui.label(
                egui::RichText::new("Fanfictions in the shelf are not deleted.")
                    .weak()
                    .italics(),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    outcome = DeleteOutcome::Confirm(shelf_id);
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
