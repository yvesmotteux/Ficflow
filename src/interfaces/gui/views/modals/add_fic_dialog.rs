use egui::{Context, Window};

pub struct AddFicState {
    pub open: bool,
    pub input: String,
}

impl AddFicState {
    pub fn new() -> Self {
        Self {
            open: false,
            input: String::new(),
        }
    }

    pub fn request_open(&mut self) {
        self.open = true;
        self.input.clear();
    }
}

pub enum Outcome {
    None,
    Submit(String),
    Cancel,
}

pub fn draw(ctx: &Context, state: &mut AddFicState) -> Outcome {
    if !state.open {
        return Outcome::None;
    }
    let mut still_open = true;
    let mut outcome = Outcome::None;
    Window::new("Add fanfiction")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.label("AO3 URL or fic ID:");
            let resp = ui.text_edit_singleline(&mut state.input);
            // Auto-focus on first frame so the user can type immediately.
            if !resp.has_focus() && state.input.is_empty() {
                resp.request_focus();
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let trimmed = state.input.trim().to_string();
                let enabled = !trimmed.is_empty();
                let pressed_enter =
                    resp.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) && enabled;
                let clicked = ui.add_enabled(enabled, egui::Button::new("Add")).clicked();
                if clicked || pressed_enter {
                    outcome = Outcome::Submit(trimmed);
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = Outcome::Cancel;
    }
    if matches!(outcome, Outcome::Submit(_) | Outcome::Cancel) {
        state.open = false;
        state.input.clear();
    }
    outcome
}
