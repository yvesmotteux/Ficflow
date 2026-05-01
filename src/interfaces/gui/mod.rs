mod app;

use std::process::ExitCode;

use crate::domain::fanfiction::FanfictionFetcher;
use crate::domain::repository::Repository;

pub fn run_gui(_fetcher: &dyn FanfictionFetcher, _repository: &dyn Repository) -> ExitCode {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ficflow")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([600.0, 400.0]),
        // Persistence intentionally off: it caused window-size restore bugs
        // last attempt and we'll re-evaluate during the styling phase.
        persist_window: false,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Ficflow",
        native_options,
        Box::new(|_cc| Ok(Box::new(app::FicflowApp::default()))),
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            log::error!("GUI failed to start: {}", err);
            ExitCode::FAILURE
        }
    }
}
