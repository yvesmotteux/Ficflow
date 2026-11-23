use egui::Context;

use super::{ModalOutcome, action_row, show_modal};

/// Buffer for the in-progress AO3 URL or fic id. The "is the modal
/// open?" answer lives in the parent `ActiveModal` enum — when this
/// struct exists at all, the modal is open.
#[derive(Default)]
pub struct AddFicState {
    pub input: String,
}

pub fn draw(ctx: &Context, state: &mut AddFicState) -> ModalOutcome<String> {
    show_modal(ctx, "Add fanfiction", |ui| {
        ui.label("AO3 URL or fic ID:");
        let resp = ui.text_edit_singleline(&mut state.input);
        // Auto-focus on first frame so the user can type immediately.
        if !resp.has_focus() && state.input.is_empty() {
            resp.request_focus();
        }
        ui.add_space(6.0);
        let trimmed = state.input.trim();
        action_row(ui, "Add", !trimmed.is_empty(), Some(&resp), || {
            trimmed.to_string()
        })
    })
}
