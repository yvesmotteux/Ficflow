use egui::{RichText, ScrollArea, Ui};

pub fn draw(ui: &mut Ui) {
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Settings");
            ui.add_space(12.0);

            ui.label(RichText::new("Application").strong());
            info_row(ui, "Version", env!("CARGO_PKG_VERSION").to_string());

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

fn info_row(ui: &mut Ui, name: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", name)).weak());
        ui.add(egui::Label::new(value).selectable(true).truncate());
    });
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
