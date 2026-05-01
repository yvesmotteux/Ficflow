use egui::{RichText, ScrollArea, Ui};

use crate::domain::fanfiction::{Fanfiction, ReadingStatus, UserRating};

use super::super::selection::Selection;

pub fn draw(ui: &mut Ui, selection: &Selection, fics: &[Fanfiction]) {
    ui.add_space(4.0);
    match selection {
        Selection::None => {
            ui.label(
                RichText::new("Select a fanfiction to see its details.")
                    .italics()
                    .weak(),
            );
        }
        Selection::Single(id) => match fics.iter().find(|f| f.id == *id) {
            Some(fic) => draw_fic(ui, fic),
            None => {
                ui.label(
                    RichText::new("Selected fanfiction not found.")
                        .italics()
                        .weak(),
                );
            }
        },
        Selection::Multi(ids) => {
            ui.label(format!("Multiple items selected ({})", ids.len()));
        }
    }
}

fn draw_fic(ui: &mut Ui, fic: &Fanfiction) {
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading(&fic.title);
            ui.label(RichText::new(format!("by {}", fic.authors.join(", "))).weak());
            ui.hyperlink_to(
                "View on AO3",
                format!("https://archiveofourown.org/works/{}", fic.id),
            );

            section(ui, "Your reading", |ui| {
                field(ui, "Status", format_status(&fic.reading_status));
                field(
                    ui,
                    "Last chapter read",
                    fic.last_chapter_read
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "-".to_string()),
                );
                field(ui, "Read count", fic.read_count.to_string());
                field(ui, "Your rating", format_user_rating(fic.user_rating));
                if let Some(note) = &fic.personal_note {
                    ui.add_space(2.0);
                    ui.label(RichText::new("Note").weak());
                    ui.label(note);
                }
            });

            section(ui, "Story", |ui| {
                field(ui, "Words", format_thousands(fic.words));
                field(
                    ui,
                    "Chapters",
                    format_chapters(fic.chapters_published, fic.chapters_total),
                );
                field(ui, "Complete", if fic.complete { "Yes" } else { "No" });
                field(ui, "Language", &fic.language);
                field(ui, "AO3 rating", format!("{:?}", fic.rating));
                field(ui, "Hits", format_thousands(fic.hits));
                field(ui, "Kudos", format_thousands(fic.kudos));
                field(
                    ui,
                    "Published",
                    fic.date_published.format("%Y-%m-%d").to_string(),
                );
                field(
                    ui,
                    "Updated",
                    fic.date_updated.format("%Y-%m-%d").to_string(),
                );
            });

            if let Some(cats) = fic.categories.as_ref().filter(|v| !v.is_empty()) {
                section(ui, "Categories", |ui| {
                    let joined = cats
                        .iter()
                        .map(|c| format!("{:?}", c))
                        .collect::<Vec<_>>()
                        .join(", ");
                    ui.label(joined);
                });
            }

            if !fic.fandoms.is_empty() {
                section(ui, "Fandoms", |ui| {
                    ui.label(fic.fandoms.join(", "));
                });
            }
            if let Some(rels) = fic.relationships.as_ref().filter(|v| !v.is_empty()) {
                section(ui, "Relationships", |ui| {
                    ui.label(rels.join(", "));
                });
            }
            if let Some(chars) = fic.characters.as_ref().filter(|v| !v.is_empty()) {
                section(ui, "Characters", |ui| {
                    ui.label(chars.join(", "));
                });
            }
            if let Some(tags) = fic.tags.as_ref().filter(|v| !v.is_empty()) {
                section(ui, "Tags", |ui| {
                    ui.label(tags.join(", "));
                });
            }
            if !fic.warnings.is_empty() {
                section(ui, "Warnings", |ui| {
                    let joined = fic
                        .warnings
                        .iter()
                        .map(|w| format!("{:?}", w))
                        .collect::<Vec<_>>()
                        .join(", ");
                    ui.label(joined);
                });
            }

            if !fic.summary.trim().is_empty() {
                section(ui, "Summary", |ui| {
                    ui.label(&fic.summary);
                });
            }
        });
}

fn section(ui: &mut Ui, title: &str, contents: impl FnOnce(&mut Ui)) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).strong());
    ui.add_space(2.0);
    contents(ui);
}

fn field(ui: &mut Ui, name: &str, value: impl Into<String>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", name)).weak());
        ui.label(value.into());
    });
}

fn format_status(status: &ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "In Progress",
        ReadingStatus::Read => "Read",
        ReadingStatus::PlanToRead => "Plan to Read",
        ReadingStatus::Paused => "Paused",
        ReadingStatus::Abandoned => "Abandoned",
    }
}

fn format_user_rating(rating: Option<UserRating>) -> String {
    match rating {
        Some(r) => format!("{}/5 {}", r as u8, "*".repeat(r as usize)),
        None => "-".to_string(),
    }
}

fn format_chapters(published: u32, total: Option<u32>) -> String {
    match total {
        Some(t) => format!("{}/{}", published, t),
        None => format!("{}/?", published),
    }
}

fn format_thousands(n: u32) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}
