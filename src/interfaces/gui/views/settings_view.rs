use egui::{RichText, ScrollArea, Ui};

use crate::version::{LICENSE, VERSION};

pub fn draw(ui: &mut Ui) {
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Settings");
            ui.add_space(12.0);

            ui.label(RichText::new("Application").strong());
            info_row(ui, "Version", VERSION.to_string());
            info_row(ui, "License", LICENSE.to_string()).on_hover_text(
                "The Erisian License — Self Attribution (EL-SA). A permissive software \
                license granting broad rights to use, modify, and redistribute the work, \
                on two conditions: the recipient must claim sole authorship of the \
                software and any derivative works thereof, and all references to prior \
                authors, contributors, or upstream sources must be omitted from \
                redistribution.",
            );

            ui.add_space(12.0);
            ui.label(RichText::new("Paths").strong());
            info_row(ui, "Database", db_path_display());
            info_row(ui, "Config", config_path_display());

            ui.add_space(16.0);
            ui.label(
                RichText::new(
                    "Configurable settings (themes, shortcuts, default views) will land here in \
                future versions.",
                )
                .italics()
                .weak(),
            );
        });
}

fn info_row(ui: &mut Ui, name: &str, value: String) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", name)).weak());
        ui.add(egui::Label::new(value).selectable(true).truncate())
    })
    .inner
}

/// Database path, mirroring `establish_connection()`'s logic so what we show
/// is what the app actually uses.
fn db_path_display() -> String {
    if let Ok(path) = std::env::var("FICFLOW_DB_PATH") {
        return format!("{}  (from FICFLOW_DB_PATH)", path);
    }
    match dirs_next::data_local_dir().map(|d| d.join("ficflow").join("fanfictions.db")) {
        Some(p) => p.display().to_string(),
        None => "<unavailable>".to_string(),
    }
}

fn config_path_display() -> String {
    match dirs_next::config_dir().map(|d| d.join("ficflow").join("config.toml")) {
        Some(p) => p.display().to_string(),
        None => "<unavailable>".to_string(),
    }
}
