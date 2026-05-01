#[derive(Default)]
pub struct FicflowApp;

impl eframe::App for FicflowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Ficflow");
            ui.label("Bare scaffolding — features land in upcoming phases.");
        });
    }
}
