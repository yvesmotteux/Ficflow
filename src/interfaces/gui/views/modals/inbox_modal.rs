use egui::Context;

use super::{ModalOutcome, action_row, show_modal};

pub fn draw_clear_confirm(ctx: &Context, count: usize) -> ModalOutcome<()> {
    show_modal(ctx, "Clear inbox", |ui| {
        ui.label(match count {
            1 => "Delete the message in your inbox?".to_string(),
            n => format!("Delete all {n} messages in your inbox?"),
        });
        ui.add_space(6.0);
        action_row(ui, "Clear all", true, None, || ())
    })
}
