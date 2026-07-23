use std::path::Path;

use egui::{Context, Window};

pub enum Outcome {
    None,
    Confirm,
    Cancel,
}

pub fn draw_confirm(ctx: &Context, backup: &Path) -> Outcome {
    let mut still_open = true;
    let mut outcome = Outcome::None;
    Window::new("Restore backup")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
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
            ui.horizontal(|ui| {
                if ui.button("Restore").clicked() {
                    outcome = Outcome::Confirm;
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
