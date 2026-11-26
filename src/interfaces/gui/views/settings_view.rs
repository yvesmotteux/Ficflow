use std::path::Path;

use chrono::{NaiveDate, Utc};
use egui::{RichText, ScrollArea, Ui};

use super::super::ao3_account::Ao3Account;
use super::super::config::{self, AppConfig, TEXT_ZOOM_RANGE, ThemeChoice};
use super::super::format::{erisian_date, relative_time};
use super::super::tasks::BackgroundStatus;
use crate::infrastructure::config::config_file_path;
use crate::infrastructure::external::github::Release;
use crate::version::{LICENSE, RELEASE_DATE, VERSION};

const ZOOM_STEP: f32 = 0.1;

/// A click on one of the Library buttons. The native file picker is opened
/// by the app layer (which owns the window handle needed to parent the
/// dialog), not here.
pub enum LibraryRequest {
    ChangeLocation,
    Restore,
}

pub struct SettingsOutcome {
    pub config_changed: bool,
    pub request: Option<LibraryRequest>,
    pub check_now: bool,
    pub import_subscriptions: Option<String>,
}

pub fn draw(
    ui: &mut Ui,
    config: &mut AppConfig,
    current_db_path: &Path,
    newer_release: Option<&Release>,
    account: &mut Ao3Account,
    background: &BackgroundStatus,
) -> SettingsOutcome {
    let mut changed = false;
    let mut request = None;
    let mut check_now = false;
    let mut import_subscriptions = None;

    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.add_space(8.0);

            ui.label(RichText::new("Application").strong());
            ui.horizontal(|ui| {
                info_row(ui, "Version", VERSION.to_string());
                if let Some(release) = newer_release {
                    let warning =
                        RichText::new(format!("New version available: {}", release.tag_name))
                            .color(ui.visuals().error_fg_color);
                    ui.hyperlink_to(warning, &release.html_url);
                }
            });
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
            ui.label(RichText::new("AO3 account").strong());
            import_subscriptions = draw_account(ui, account);

            ui.add_space(12.0);
            ui.label(RichText::new("Updates").strong());
            changed |= ui
                .checkbox(
                    &mut config.background_checks,
                    "Check for new chapters automatically",
                )
                .changed();
            ui.label(
                RichText::new(
                    "While Ficflow is open, unfinished fics are checked when it starts and then \
                    one every few minutes. Other fics get their stats refreshed every two weeks.",
                )
                .weak()
                .italics(),
            );
            changed |= ui
                .checkbox(
                    &mut config.desktop_notifications,
                    "Show a desktop notification when new chapters come out",
                )
                .changed();
            ui.horizontal(|ui| {
                let checking = background.remaining > 0;
                if ui
                    .add_enabled(!checking, egui::Button::new("Check for updates now"))
                    .clicked()
                {
                    check_now = true;
                }
                let status = if checking {
                    match background.remaining {
                        1 => "Checking, 1 fic left…".to_string(),
                        n => format!("Checking, {n} fics left…"),
                    }
                } else {
                    match background.last_full_check {
                        Some(when) => {
                            format!("Last full check {}", relative_time(when, Utc::now()))
                        }
                        None => "Not checked yet since Ficflow started".to_string(),
                    }
                };
                ui.label(RichText::new(status).weak());
            });

            ui.add_space(12.0);
            ui.label(RichText::new("Library").strong());
            info_row(ui, "Location", current_db_path.display().to_string());
            ui.horizontal(|ui| {
                if ui.button("Change location…").clicked() {
                    request = Some(LibraryRequest::ChangeLocation);
                }
                if ui.button("Restore from backup…").clicked() {
                    request = Some(LibraryRequest::Restore);
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
        request,
        check_now,
        import_subscriptions,
    }
}

fn draw_account(ui: &mut Ui, account: &mut Ao3Account) -> Option<String> {
    let mut import_subscriptions = None;
    if account.logging_in() {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Logging in…");
        });
    } else if let Some(username) = account.username().map(str::to_string) {
        ui.horizontal(|ui| {
            info_row(ui, "Logged in as", username.clone());
            if ui.button("Log out").clicked() {
                account.log_out();
            }
        });
        if ui
            .button("Import my subscriptions")
            .on_hover_text("Adds every work you're subscribed to on AO3")
            .clicked()
        {
            import_subscriptions = Some(username);
        }
    } else {
        let mut submit = false;
        egui::Grid::new("ao3_login").num_columns(2).show(ui, |ui| {
            ui.label(RichText::new("User name or email:").weak());
            ui.add(egui::TextEdit::singleline(&mut account.username_input).desired_width(220.0));
            ui.end_row();
            ui.label(RichText::new("Password:").weak());
            let password = ui.add(
                egui::TextEdit::singleline(&mut account.password_input)
                    .password(true)
                    .desired_width(220.0),
            );
            submit = password.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.end_row();
        });
        if ui.button("Log in").clicked() || submit {
            account.log_in(ui.ctx());
        }
        ui.label(
            RichText::new(
                "Needed to add restricted works. Your password is only used to log in and is \
                never saved; Ficflow keeps the AO3 login in your system keychain.",
            )
            .weak()
            .italics(),
        );
    }
    if let Some(error) = account.error() {
        ui.colored_label(ui.visuals().error_fg_color, error);
    }
    import_subscriptions
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
    config_file_path().map_or_else(|| "<unavailable>".to_string(), |p| p.display().to_string())
}
