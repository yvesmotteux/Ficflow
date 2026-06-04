use egui::{Context, Window};

pub enum Outcome {
    None,
    Quit,
    Cancel,
}

pub fn draw_confirm(ctx: &Context, running_count: usize) -> Outcome {
    let mut still_open = true;
    let mut outcome = Outcome::None;
    Window::new("Quit Ficflow")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
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
            ui.horizontal(|ui| {
                if ui.button("Quit anyway").clicked() {
                    outcome = Outcome::Quit;
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
