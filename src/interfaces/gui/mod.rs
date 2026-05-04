pub mod app;
mod config;
mod fonts;
mod format;
mod library_cache;
mod selection;
mod selection_controller;
mod tasks;
mod view;
mod views;
mod widgets;

use std::process::ExitCode;

pub use app::{FicflowApp, FicflowConfig, InitError};
pub use config::{ColumnKey, SortDirection, SortPref};
pub use selection::Selection;
pub use tasks::{TaskKind, TaskState, TaskStatus};
pub use view::View;

/// Entry point for the GUI binary path. Builds its own connection,
/// fetcher, and worker thread inside `FicflowApp::with_config` — the
/// caller doesn't need to pre-construct anything.
pub fn run_gui() -> ExitCode {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Ficflow")
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([600.0, 400.0]),
        // Persistence on: window geometry, side-panel widths, and
        // egui_extras table column widths all live in egui's memory and are
        // serialised to the eframe storage path between launches.
        persist_window: true,
        ..Default::default()
    };

    let result = eframe::run_native(
        "Ficflow",
        native_options,
        Box::new(|cc| {
            let app = FicflowApp::new(cc).map_err(|e| {
                log::error!("Failed to initialise GUI: {}", e);
                Box::new(e) as Box<dyn std::error::Error + Send + Sync>
            })?;
            Ok(Box::new(app))
        }),
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            log::error!("GUI failed to start: {}", err);
            ExitCode::FAILURE
        }
    }
}
