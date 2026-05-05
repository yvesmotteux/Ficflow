pub mod app;
mod assets;
mod chrome;
mod config;
mod format;
mod library_cache;
mod selection;
mod selection_controller;
mod tasks;
mod theme;
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
    // Decode the bundled PNG into RGBA bytes once at startup and hand
    // it to the viewport builder. eframe forwards it to winit which
    // sets the window-list / taskbar / alt-tab icon. If decoding ever
    // fails, fall back to no icon rather than refusing to start.
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Ficflow")
        // Phase 12: borderless + transparent so we can paint the
        // Art Nouveau chrome ourselves. `clear_color` in
        // `FicflowApp` returns `[0;4]` to keep the alpha channel
        // through to the compositor.
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size([600.0, 400.0]);
    match eframe::icon_data::from_png_bytes(assets::ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(err) => log::warn!("Failed to decode window icon: {}", err),
    }
    let native_options = eframe::NativeOptions {
        viewport,
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
