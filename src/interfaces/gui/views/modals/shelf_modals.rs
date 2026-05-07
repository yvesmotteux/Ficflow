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
        .default_pos(ctx.content_rect().center())
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

pub struct RenameState {
    pub shelf_id: u64,
    pub name: String,
    focused_once: bool,
}

impl RenameState {
    pub fn new(shelf: &Shelf) -> Self {
        Self {
            shelf_id: shelf.id,
            name: shelf.name.clone(),
            focused_once: false,
        }
    }
}

pub enum RenameOutcome {
    None,
    Submit { shelf_id: u64, new_name: String },
    Cancel,
}

pub fn draw_rename(ctx: &Context, state: &mut RenameState) -> RenameOutcome {
    let mut still_open = true;
    let mut outcome = RenameOutcome::None;
    Window::new("Rename shelf")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.label("Name:");
            let resp = ui.text_edit_singleline(&mut state.name);
            // Auto-focus + select-all on the first frame so the user can
            // either type a fresh name or edit a tail of the existing one
            // without having to click first.
            if !state.focused_once {
                resp.request_focus();
                if let Some(mut tes) = egui::TextEdit::load_state(ctx, resp.id) {
                    let end = state.name.chars().count();
                    tes.cursor
                        .set_char_range(Some(egui::text::CCursorRange::two(
                            egui::text::CCursor::new(0),
                            egui::text::CCursor::new(end),
                        )));
                    tes.store(ctx, resp.id);
                }
                state.focused_once = true;
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let submit_enabled = !state.name.trim().is_empty();
                let rename_clicked = ui
                    .add_enabled(submit_enabled, egui::Button::new("Rename"))
                    .clicked();
                let pressed_enter = resp.lost_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                    && submit_enabled;
                if rename_clicked || pressed_enter {
                    outcome = RenameOutcome::Submit {
                        shelf_id: state.shelf_id,
                        new_name: state.name.trim().to_string(),
                    };
                }
                if ui.button("Cancel").clicked() {
                    outcome = RenameOutcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = RenameOutcome::Cancel;
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
        .default_pos(ctx.content_rect().center())
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
