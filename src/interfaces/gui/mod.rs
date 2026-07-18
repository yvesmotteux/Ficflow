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
pub use config::{AppConfig, ColumnKey, SortDirection, SortPref};
pub use selection::Selection;
pub use tasks::{TaskKind, TaskState, TaskStatus};
pub use view::View;

pub fn run_gui() -> ExitCode {
    // eframe multiplies `min_inner_size` by the *current* zoom factor when
    // enforcing it as the OS-level minimum window size (unlike `inner_size`,
    // which it clamps back down to the monitor's bounds itself) — so the
    // value handed to the window here is pre-divided by the persisted zoom,
    // keeping the OS-enforced physical floor at a constant ~600x400
    // regardless of `text_zoom`. `FicflowApp::apply_min_inner_size` reasserts
    // this same compensation whenever zoom changes at runtime.
    let zoom = AppConfig::load().text_zoom.clamp(
        *config::TEXT_ZOOM_RANGE.start(),
        *config::TEXT_ZOOM_RANGE.end(),
    );
    let min_inner_size = [
        config::BASE_MIN_INNER_SIZE[0] / zoom,
        config::BASE_MIN_INNER_SIZE[1] / zoom,
    ];

    // Borderless + transparent so the Art Nouveau chrome paints in
    // place of the OS title bar (`FicflowApp::clear_color` returns
    // `[0;4]` so the alpha channel reaches the compositor).
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Ficflow")
        .with_app_id("ficflow")
        .with_decorations(false)
        .with_transparent(true)
        .with_inner_size([1100.0, 700.0])
        .with_min_inner_size(min_inner_size);
    match eframe::icon_data::from_png_bytes(assets::ICON_PNG) {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(err) => log::warn!("Failed to decode window icon: {}", err),
    }
    let native_options = eframe::NativeOptions {
        viewport,
        // Window geometry, side-panel widths, and table column widths
        // persist across launches via egui's memory.
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
