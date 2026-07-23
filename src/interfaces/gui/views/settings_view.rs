use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use egui::{RichText, ScrollArea, Ui};

use super::super::config::{self, AppConfig, TEXT_ZOOM_RANGE, ThemeChoice};
use super::super::format::erisian_date;
use crate::version::{LICENSE, RELEASE_DATE, VERSION};

const ZOOM_STEP: f32 = 0.1;

pub enum LibraryAction {
    ChangeLocation(PathBuf),
    Restore(PathBuf),
}

pub struct SettingsOutcome {
    pub config_changed: bool,
    pub action: Option<LibraryAction>,
}

pub fn draw(ui: &mut Ui, config: &mut AppConfig, current_db_path: &Path) -> SettingsOutcome {
    let mut changed = false;
    let mut action = None;

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
                let can_shrink = config.text_zoom > *TEXT_ZOOM_RANGE.start();
                let can_grow = config.text_zoom < *TEXT_ZOOM_RANGE.end();
                if ui.add_enabled(can_shrink, egui::Button::new("−")).clicked() {
                    apply_zoom(config, &mut changed, ui.ctx(), config.text_zoom - ZOOM_STEP);
                }
                ui.label(format!("{:.0}%", config.text_zoom * 100.0));
                if ui.add_enabled(can_grow, egui::Button::new("+")).clicked() {
                    apply_zoom(config, &mut changed, ui.ctx(), config.text_zoom + ZOOM_STEP);
                }
                ui.label("Text size");
                let at_default = (config.text_zoom - 1.0).abs() <= f32::EPSILON;
                if ui
                    .add_enabled(!at_default, egui::Button::new("Reset"))
                    .clicked()
                {
                    apply_zoom(config, &mut changed, ui.ctx(), 1.0);
                }
            });
            ui.label(RichText::new("Or use Ctrl/Cmd +/-/0.").weak().italics());

            ui.add_space(12.0);
            ui.label(RichText::new("Theme").strong());
            ui.horizontal(|ui| {
                for choice in ThemeChoice::ALL {
                    if ui
                        .selectable_label(config.theme == choice, choice.label())
                        .clicked()
                        && config.theme != choice
                    {
                        config.theme = choice;
                        config::set_theme(ui.ctx(), choice);
                        changed = true;
                    }
                }
            });

            ui.add_space(12.0);
            ui.label(RichText::new("Library").strong());
            info_row(ui, "Location", current_db_path.display().to_string());
            let start_dir = current_db_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf();
            ui.horizontal(|ui| {
                if ui.button("Change location…").clicked()
                    && let Some(folder) = rfd::FileDialog::new()
                        .set_directory(&start_dir)
                        .pick_folder()
                {
                    action = Some(LibraryAction::ChangeLocation(folder));
                }
                if ui.button("Restore from backup…").clicked()
                    && let Some(file) = rfd::FileDialog::new()
                        .add_filter("SQLite database", &["db"])
                        .set_directory(&start_dir)
                        .pick_file()
                {
                    action = Some(LibraryAction::Restore(file));
                }
            });
            ui.label(
                RichText::new(
                    "Copy this file elsewhere to back up your library. Restore replaces your \
                    current library with a backup .db, copying it here without changing the \
                    location above. Changes take effect after a restart.",
                )
                .weak()
                .italics(),
            );

            ui.add_space(12.0);
            ui.label(RichText::new("Paths").strong());
            info_row(ui, "Config", config_path_display());
        });

    SettingsOutcome {
        config_changed: changed,
        action,
    }
}

fn apply_zoom(config: &mut AppConfig, changed: &mut bool, ctx: &egui::Context, zoom: f32) {
    config.text_zoom = config::set_zoom(ctx, zoom);
    *changed = true;
}

fn info_row(ui: &mut Ui, name: &str, value: String) -> egui::Response {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", name)).weak());
        ui.add(egui::Label::new(value).selectable(true).truncate())
    })
    .inner
}

fn config_path_display() -> String {
    match dirs_next::config_dir().map(|d| d.join("ficflow").join("config.toml")) {
        Some(p) => p.display().to_string(),
        None => "<unavailable>".to_string(),
    }
}
