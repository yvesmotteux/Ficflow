use chrono::NaiveDate;
use egui::{RichText, ScrollArea, Ui};

use super::super::config::{AppConfig, BASE_MIN_INNER_SIZE, TEXT_ZOOM_RANGE};
use super::super::format::erisian_date;
use crate::version::{LICENSE, RELEASE_DATE, VERSION};

pub fn draw(ui: &mut Ui, config: &mut AppConfig) -> bool {
    let mut changed = false;

    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.add_space(8.0);
            ui.heading("Settings");
            ui.add_space(12.0);

            ui.label(RichText::new("Application").strong());
            info_row(ui, "Version", VERSION.to_string());
            if let Ok(date) = NaiveDate::parse_from_str(RELEASE_DATE, "%Y-%m-%d") {
                info_row(ui, "Released", erisian_date(date))
                    .on_hover_text(date.format("%-d %B %Y").to_string());
            }
            info_row(ui, "License", LICENSE.to_string()).on_hover_text(
                "The Erisian License — Self Attribution (EL-SA). A permissive software \
                license granting broad rights to use, modify, and redistribute the work, \
                on two conditions: the recipient must claim sole authorship of the \
                software and any derivative works thereof, and all references to prior \
                authors, contributors, or upstream sources must be omitted from \
                redistribution.",
            );

            ui.add_space(12.0);
            ui.label(RichText::new("Display").strong());
            ui.horizontal(|ui| {
                let resp = ui.add(
                    egui::Slider::new(&mut config.text_zoom, TEXT_ZOOM_RANGE)
                        .step_by(0.05)
                        .custom_formatter(|n, _| format!("{:.0}%", n * 100.0))
                        .custom_parser(|s| {
                            s.trim_end_matches('%')
                                .trim()
                                .parse::<f64>()
                                .ok()
                                .map(|p| p / 100.0)
                        })
                        .text("Text size"),
                );
                if resp.changed() {
                    apply_zoom(ui.ctx(), config.text_zoom);
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    changed = true;
                }
                if ui.button("Reset").clicked() {
                    config.text_zoom = 1.0;
                    apply_zoom(ui.ctx(), 1.0);
                    changed = true;
                }
            });

            ui.add_space(12.0);
            ui.label(RichText::new("Paths").strong());
            info_row(ui, "Database", db_path_display());
            info_row(ui, "Config", config_path_display());
        });

    changed
}

/// Sets the zoom factor and, in the same step, reasserts the OS-enforced
/// minimum window size compensated for it — see `FicflowApp::apply_min_inner_size`
/// for why eframe needs this counteracted on every zoom change.
fn apply_zoom(ctx: &egui::Context, zoom: f32) {
    ctx.set_zoom_factor(zoom);
    ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
        BASE_MIN_INNER_SIZE[0] / zoom,
        BASE_MIN_INNER_SIZE[1] / zoom,
    )));
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
