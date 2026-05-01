use egui::{ComboBox, DragValue, RichText, ScrollArea, TextEdit, Ui};
use egui_notify::Toasts;
use rusqlite::Connection;

use crate::application::{
    update_chapters::update_last_chapter_read, update_note::update_personal_note,
    update_rating::update_user_rating, update_read_count::update_read_count,
    update_status::update_reading_status,
};
use crate::domain::fanfiction::{Fanfiction, ReadingStatus, UserRating};
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;

use super::super::selection::Selection;
use super::super::widgets::star_rating;

pub fn draw(
    ui: &mut Ui,
    selection: &Selection,
    fics: &mut Vec<Fanfiction>,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    ui.add_space(4.0);
    match selection {
        Selection::None => {
            ui.label(
                RichText::new("Select a fanfiction to see its details.")
                    .italics()
                    .weak(),
            );
        }
        Selection::Single(id) => match fics.iter().position(|f| f.id == *id) {
            Some(idx) => draw_fic(ui, fics, idx, conn, toasts),
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

fn draw_fic(
    ui: &mut Ui,
    fics: &mut Vec<Fanfiction>,
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let fic_id = fics[idx].id;
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading(&fics[idx].title);
            ui.label(RichText::new(format!("by {}", fics[idx].authors.join(", "))).weak());
            ui.hyperlink_to(
                "View on AO3",
                format!("https://archiveofourown.org/works/{}", fic_id),
            );

            section(ui, "Your reading", |ui| {
                draw_status(ui, fics, idx, conn, toasts);
                draw_chapter(ui, fics, idx, conn, toasts);
                draw_read_count(ui, fics, idx, conn, toasts);
                draw_rating(ui, fics, idx, conn, toasts);
                draw_note(ui, fics, idx, conn, toasts);
            });

            // Story metadata stays read-only — these come from AO3.
            section(ui, "Story", |ui| {
                let fic = &fics[idx];
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

            draw_metadata_lists(ui, &fics[idx]);
        });
}

fn draw_status(
    ui: &mut Ui,
    fics: &mut Vec<Fanfiction>,
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let current = fics[idx].reading_status;
    let mut chosen = current;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Status:").weak());
        ComboBox::from_id_salt("status-combo")
            .selected_text(format_status(&chosen))
            .show_ui(ui, |ui| {
                for status in [
                    ReadingStatus::InProgress,
                    ReadingStatus::Read,
                    ReadingStatus::PlanToRead,
                    ReadingStatus::Paused,
                    ReadingStatus::Abandoned,
                ] {
                    ui.selectable_value(&mut chosen, status, format_status(&status));
                }
            });
    });
    if chosen != current {
        let repo = SqliteRepository::new(conn);
        match update_reading_status(&repo, fics[idx].id, status_payload(chosen)) {
            Ok(updated) => fics[idx] = updated,
            Err(err) => toast_error(toasts, "Couldn't update status", err),
        }
    }
}

fn draw_chapter(
    ui: &mut Ui,
    fics: &mut Vec<Fanfiction>,
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let total = fics[idx].chapters_published;
    let mut chapter = fics[idx].last_chapter_read.unwrap_or(0);
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Last chapter read:").weak());
        changed = stepper(ui, &mut chapter, Some(total));
    });
    if changed {
        let repo = SqliteRepository::new(conn);
        match update_last_chapter_read(&repo, fics[idx].id, chapter) {
            Ok(updated) => fics[idx] = updated,
            Err(err) => toast_error(toasts, "Couldn't update chapter", err),
        }
    }
}

fn draw_read_count(
    ui: &mut Ui,
    fics: &mut Vec<Fanfiction>,
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let mut reads = fics[idx].read_count;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Read count:").weak());
        changed = stepper(ui, &mut reads, None);
    });
    if changed {
        let repo = SqliteRepository::new(conn);
        match update_read_count(&repo, fics[idx].id, reads) {
            Ok(updated) => fics[idx] = updated,
            Err(err) => toast_error(toasts, "Couldn't update read count", err),
        }
    }
}

/// Integer input with `[-]` / `[+]` buttons surrounding a `DragValue`. Drag
/// behaviour is preserved on the inner widget; the buttons step by 1 and
/// respect the optional max.
fn stepper(ui: &mut Ui, value: &mut u32, max: Option<u32>) -> bool {
    let mut changed = false;
    let cap = max.unwrap_or(u32::MAX);

    if ui.small_button("-").clicked() {
        *value = value.saturating_sub(1);
        changed = true;
    }

    let mut dv = DragValue::new(value);
    if let Some(m) = max {
        dv = dv.range(0..=m);
    }
    if ui.add(dv).changed() {
        changed = true;
    }

    if ui.small_button("+").clicked() && *value < cap {
        *value += 1;
        changed = true;
    }

    changed
}

fn draw_rating(
    ui: &mut Ui,
    fics: &mut Vec<Fanfiction>,
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let mut rating = fics[idx].user_rating;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Your rating:").weak());
        if star_rating::star_rating(ui, &mut rating) {
            let repo = SqliteRepository::new(conn);
            match update_user_rating(&repo, fics[idx].id, rating_payload(rating)) {
                Ok(updated) => fics[idx] = updated,
                Err(err) => toast_error(toasts, "Couldn't update rating", err),
            }
        }
    });
}

fn draw_note(
    ui: &mut Ui,
    fics: &mut Vec<Fanfiction>,
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    ui.add_space(2.0);
    ui.label(RichText::new("Note:").weak());
    let mut buf = fics[idx].personal_note.clone().unwrap_or_default();
    let resp = ui.add(
        TextEdit::multiline(&mut buf)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    // Reflect each keystroke into the in-memory fic so the next frame doesn't
    // overwrite `buf` with the stale stored value (otherwise the text appears
    // to vanish as you type).
    if resp.changed() {
        fics[idx].personal_note = if buf.is_empty() {
            None
        } else {
            Some(buf.clone())
        };
    }
    // Commit to disk on focus loss so we don't fire a DB write per keystroke.
    // Empty buffer → remove the note, matching the CLI's `note <id>` form.
    if resp.lost_focus() {
        let repo = SqliteRepository::new(conn);
        let payload = if buf.trim().is_empty() {
            None
        } else {
            Some(buf.as_str())
        };
        match update_personal_note(&repo, fics[idx].id, payload) {
            Ok(updated) => fics[idx] = updated,
            Err(err) => toast_error(toasts, "Couldn't update note", err),
        }
    }
}

fn draw_metadata_lists(ui: &mut Ui, fic: &Fanfiction) {
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

/// Canonical strings the application layer's parser recognises.
fn status_payload(status: ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "inprogress",
        ReadingStatus::Read => "read",
        ReadingStatus::PlanToRead => "plantoread",
        ReadingStatus::Paused => "paused",
        ReadingStatus::Abandoned => "abandoned",
    }
}

fn rating_payload(rating: Option<UserRating>) -> &'static str {
    match rating {
        Some(UserRating::One) => "1",
        Some(UserRating::Two) => "2",
        Some(UserRating::Three) => "3",
        Some(UserRating::Four) => "4",
        Some(UserRating::Five) => "5",
        None => "none",
    }
}

fn toast_error(toasts: &mut Toasts, prefix: &str, err: FicflowError) {
    let message = format!("{}: {}", prefix, err);
    log::warn!("{}", message);
    toasts.error(message);
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
