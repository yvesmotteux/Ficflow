use std::path::Path;

use egui::Context;

use super::{ModalOutcome, action_row, show_modal};

pub fn draw_confirm(ctx: &Context, backup: &Path) -> ModalOutcome<()> {
    show_modal(ctx, "Restore backup", |ui| {
        ui.label("Replace your current library with this backup?");
        ui.label(
            egui::RichText::new(backup.display().to_string())
                .weak()
                .italics(),
        );
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Your current library will be overwritten.")
                .weak()
                .italics(),
        );
        ui.add_space(6.0);
        action_row(ui, "Restore", true, None, || ())
    })
}
