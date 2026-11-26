use chrono::Utc;
use egui::{Align, Layout, RichText, ScrollArea, Ui};

use crate::domain::series::{Series, SeriesWork};

use super::super::format::{ao3_work_url, relative_time};
use super::details_panel::ao3_row;
use crate::interfaces::utils::formatter::format_thousands;

pub enum SeriesOutcome {
    None,
    Refresh,
    Add(Vec<u64>),
}

pub fn draw_series(ui: &mut Ui, series: &Series, missing: &[u64]) -> SeriesOutcome {
    let mut outcome = SeriesOutcome::None;
    header(ui, &series.title, &series.authors, &series_url(series.id));
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("SERIES").strong().small());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add_space(10.0);
                    let resp = ui
                        .small_button(RichText::new("\u{21BB}").size(14.0))
                        .on_hover_text("Refresh from AO3");
                    if resp.clicked() {
                        outcome = SeriesOutcome::Refresh;
                    }
                    ui.label(
                        RichText::new(format!(
                            "Updated {}",
                            relative_time(series.last_checked_date, Utc::now())
                        ))
                        .weak()
                        .small(),
                    );
                });
            });
            ui.add_space(6.0);
            let total = series.works.len().max(series.works_count as usize);
            let local = total.saturating_sub(missing.len());
            ao3_row(ui, "Parts", |ui| {
                ui.label(format!("{local} of {total} in your library"));
            });
            if !missing.is_empty() {
                ao3_row(ui, "", |ui| {
                    let label = match missing.len() {
                        1 => "Add the missing part".to_string(),
                        n => format!("Add the {n} missing parts"),
                    };
                    if ui.button(label).clicked() {
                        outcome = SeriesOutcome::Add(missing.to_vec());
                    }
                });
            }
            ao3_row(ui, "Complete", |ui| {
                ui.label(if series.complete { "Yes" } else { "No" });
            });
            ao3_row(ui, "Words", |ui| {
                ui.label(format_thousands(series.words));
            });
            for (label, date) in [
                ("Begun", series.date_begun),
                ("Updated", series.date_updated),
            ] {
                if let Some(date) = date {
                    ao3_row(ui, label, |ui| {
                        ui.label(date.format("%Y-%m-%d").to_string());
                    });
                }
            }
            ao3_row(ui, "Bookmarks", |ui| {
                ui.label(format_thousands(series.bookmarks));
            });
            for (label, text) in [
                ("Description", &series.description),
                ("Notes", &series.notes),
            ] {
                if let Some(text) = text {
                    ao3_row(ui, label, |ui| {
                        ui.add(egui::Label::new(text).wrap());
                    });
                }
            }
        });
    outcome
}

pub fn draw_missing(ui: &mut Ui, series: &Series, work: &SeriesWork) -> Option<u64> {
    let mut add = None;
    header(
        ui,
        &work.title,
        &work.authors,
        &ao3_work_url(work.work_id, None),
    );
    ui.add_space(6.0);
    ao3_row(ui, "Series", |ui| {
        ui.label(format!("Part {} of {}", work.part, series.title));
    });
    ao3_row(ui, "", |ui| {
        ui.label(RichText::new("This part isn't in your library yet.").weak());
        if ui.button("Add to library").clicked() {
            add = Some(work.work_id);
        }
    });
    add
}

pub fn draw_hidden(ui: &mut Ui, series: &Series, part: u32, logged_in: bool) -> SeriesOutcome {
    let mut outcome = SeriesOutcome::None;
    header(ui, &format!("Part {part}"), &[], &series_url(series.id));
    ui.add_space(6.0);
    ao3_row(ui, "Series", |ui| {
        ui.label(format!("Part {part} of {}", series.title));
    });
    let reason = if logged_in {
        "AO3 doesn't list this part, even for your account. Its author may have made it \
        private or deleted it."
    } else {
        "AO3 doesn't list this part. It may only be visible to logged-in AO3 users, or its \
        author may have made it private or deleted it. Log in to AO3 in Settings, then refresh \
        the series to see it."
    };
    ao3_row(ui, "", |ui| {
        ui.add(egui::Label::new(RichText::new(reason).weak()).wrap());
        if ui.button("Refresh the series").clicked() {
            outcome = SeriesOutcome::Refresh;
        }
    });
    outcome
}

fn header(ui: &mut Ui, title: &str, authors: &[String], url: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(title).heading().strong());
    ui.add_space(2.0);
    if !authors.is_empty() {
        ui.label(RichText::new(format!("by {}", authors.join(", "))).weak());
    }
    ui.add_space(4.0);
    ui.hyperlink_to(RichText::new(url).small(), url);
    ui.separator();
}

fn series_url(series_id: u64) -> String {
    format!("https://archiveofourown.org/series/{series_id}")
}
