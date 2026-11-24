use egui::Context;

use super::{ModalOutcome, action_row, show_modal};

pub fn draw_confirm(ctx: &Context, running_count: usize) -> ModalOutcome<()> {
    show_modal(ctx, "Quit Ficflow", |ui| {
        let noun = if running_count == 1 {
            "task is"
        } else {
            "tasks are"
        };
        ui.label(format!("{} {} still running.", running_count, noun));
        ui.label(
            egui::RichText::new("Unfinished tasks will be lost.")
                .weak()
                .italics(),
        );
        ui.add_space(6.0);
        action_row(ui, "Quit anyway", true, None, || ())
    })
}
