//! Floating windows opened on demand from the main UI: column picker,
//! add-fic input, shelf create/delete confirmations, and the bulk-delete
//! confirmation. They share no internal state with the views that own
//! their open/closed flags — the parent passes `&mut state` and the
//! modal returns an Outcome enum the parent dispatches on.

pub mod add_fic_dialog;
pub mod bulk_modals;
pub mod column_picker;
pub mod inbox_modal;
pub mod quit_modal;
pub mod restore_modal;
pub mod shelf_modals;

use egui::{Context, Response, Ui, Window};

pub enum ModalOutcome<T> {
    None,
    Submit(T),
    Cancel,
}

pub fn modal_window(ctx: &Context, title: &str) -> Window<'static> {
    Window::new(title.to_owned())
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
}

pub fn show_modal<T>(
    ctx: &Context,
    title: &str,
    body: impl FnOnce(&mut Ui) -> ModalOutcome<T>,
) -> ModalOutcome<T> {
    let mut still_open = true;
    let mut outcome = ModalOutcome::None;
    modal_window(ctx, title)
        .open(&mut still_open)
        .show(ctx, |ui| outcome = body(ui));
    if still_open {
        outcome
    } else {
        ModalOutcome::Cancel
    }
}

/// Submit + Cancel buttons. `enter_from` submits on Enter when that text
/// field loses focus, matching a click on the submit button.
pub fn action_row<T>(
    ui: &mut Ui,
    submit_label: &str,
    enabled: bool,
    enter_from: Option<&Response>,
    value: impl FnOnce() -> T,
) -> ModalOutcome<T> {
    ui.horizontal(|ui| {
        let clicked = ui
            .add_enabled(enabled, egui::Button::new(submit_label))
            .clicked();
        let pressed_enter = enter_from
            .is_some_and(|resp| resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        let cancelled = ui.button("Cancel").clicked();
        if enabled && (clicked || pressed_enter) {
            ModalOutcome::Submit(value())
        } else if cancelled {
            ModalOutcome::Cancel
        } else {
            ModalOutcome::None
        }
    })
    .inner
}
