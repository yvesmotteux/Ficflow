use std::collections::HashSet;

use chrono::{DateTime, Utc};
use egui::{
    Align, Color32, ComboBox, DragValue, Layout, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui,
};
use egui_notify::Toasts;
use rusqlite::Connection;

use crate::application::{
    add_to_shelf::add_to_shelf, delete_fic::delete_fic, remove_from_shelf::remove_from_shelf,
    update_chapters::update_last_chapter_read, update_note::update_personal_note,
    update_rating::update_user_rating, update_read_count::update_read_count,
    update_status::update_reading_status,
};
use crate::domain::fanfiction::{ArchiveWarnings, Categories, Fanfiction, Rating, ReadingStatus};
use crate::domain::shelf::Shelf;
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;

use super::super::format::{format_status, format_thousands, rating_payload, status_payload};
use super::super::selection::Selection;
use super::super::tasks::TaskExecutor;
use super::super::widgets::shelves_dropdown::{self, DropdownOutcome};
use super::super::widgets::star_rating;

pub struct DetailsState<'a> {
    pub selection: &'a Selection,
    pub fics: &'a mut Vec<Fanfiction>,
    pub conn: &'a Connection,
    pub toasts: &'a mut Toasts,
    pub all_shelves: &'a [Shelf],
    /// Shelves currently containing the selected fic. Caller maintains this
    /// cache and refreshes when this function returns `true`.
    pub selection_shelf_ids: &'a HashSet<u64>,
    pub task_executor: &'a TaskExecutor,
}

/// Returns true when a fic-shelf link was toggled OR the fic was deleted —
/// either way the caller refreshes its caches.
///
/// Caller (app.rs) only mounts this panel when `selection` is `Single(_)`.
pub fn draw(ui: &mut Ui, state: DetailsState<'_>) -> bool {
    let DetailsState {
        selection,
        fics,
        conn,
        toasts,
        all_shelves,
        selection_shelf_ids,
        task_executor,
    } = state;

    let Selection::Single(id) = selection else {
        return false;
    };
    let Some(idx) = fics.iter().position(|f| f.id == *id) else {
        ui.add_space(8.0);
        ui.label(
            RichText::new("Selected fanfiction not found.")
                .italics()
                .weak(),
        );
        return false;
    };

    let mut shelves_changed = false;
    let mut deleted = false;

    // Bottom-pinned: Your Info (status / chapter / reads / rating /
    // notes / shelves dropdown / Delete Fic). Always visible regardless
    // of where the AO3-metadata scroll area is.
    egui::TopBottomPanel::bottom("details-your-info")
        .resizable(true)
        .default_height(280.0)
        .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(8.0, 6.0)))
        .show_inside(ui, |ui| {
            let outcome = draw_your_info(
                ui,
                fics,
                idx,
                conn,
                toasts,
                all_shelves,
                selection_shelf_ids,
            );
            if outcome.shelves_changed {
                shelves_changed = true;
            }
            if outcome.deleted {
                deleted = true;
            }
        });

    // Top-pinned header (title, author with link, fic URL).
    egui::TopBottomPanel::top("details-header")
        .resizable(false)
        .show_separator_line(true)
        .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(8.0, 8.0)))
        .show_inside(ui, |ui| {
            draw_header(ui, &fics[idx]);
        });

    // Central scrollable: AO3 Metadata.
    egui::CentralPanel::default()
        .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(8.0, 4.0)))
        .show_inside(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    draw_ao3_metadata(ui, &fics[idx], task_executor);
                });
        });

    shelves_changed || deleted
}

// ---------------------------------------------------------------------------
// Header — title, author with AO3-author link, full fic URL
// ---------------------------------------------------------------------------

fn draw_header(ui: &mut Ui, fic: &Fanfiction) {
    ui.label(RichText::new(&fic.title).heading().strong());
    ui.add_space(2.0);

    // Authors with a small "open on AO3" arrow next to each.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new("by").weak());
        for (i, author) in fic.authors.iter().enumerate() {
            if i > 0 {
                ui.label(",");
            }
            ui.label(author);
            // ↗ NORTH EAST ARROW — universal "external link" indicator.
            // AO3 author URLs use the username verbatim; this works for
            // the vast majority of usernames (which are alphanumeric).
            ui.hyperlink_to(
                "\u{2197}",
                format!("https://archiveofourown.org/users/{}/works", author),
            );
        }
    });

    ui.add_space(4.0);
    let url = format!("https://archiveofourown.org/works/{}", fic.id);
    ui.hyperlink_to(RichText::new(&url).small(), &url);
}

// ---------------------------------------------------------------------------
// Your Info — status / chapter / reads / rating / notes / shelves / delete
// ---------------------------------------------------------------------------

struct YourInfoOutcome {
    shelves_changed: bool,
    deleted: bool,
}

fn draw_your_info(
    ui: &mut Ui,
    fics: &mut Vec<Fanfiction>,
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
    all_shelves: &[Shelf],
    selection_shelf_ids: &HashSet<u64>,
) -> YourInfoOutcome {
    let fic_id = fics[idx].id;
    let mut shelves_changed = false;
    let mut deleted = false;

    ui.label(RichText::new("YOUR INFO").strong().small());
    ui.add_space(4.0);

    kv_row(ui, "Status", |ui| draw_status(ui, fics, idx, conn, toasts));
    kv_row(ui, "Chapter", |ui| {
        draw_chapter(ui, fics, idx, conn, toasts)
    });
    kv_row(ui, "Reads", |ui| {
        draw_read_count(ui, fics, idx, conn, toasts)
    });
    kv_row(ui, "Rating", |ui| draw_rating(ui, fics, idx, conn, toasts));

    // Shelves dropdown: capture the toggle event into a local, then run
    // the DB write *after* the kv_row closure releases its borrow on
    // `conn` / `toasts`. Avoids nested-mut-borrow errors.
    let mut toggled: Option<(u64, bool)> = None;
    kv_row(ui, "Shelves", |ui| {
        let outcome = shelves_dropdown::shelves_dropdown(
            ui,
            "details-shelves",
            all_shelves,
            selection_shelf_ids,
        );
        if let DropdownOutcome::Toggled {
            shelf_id,
            now_selected,
        } = outcome
        {
            toggled = Some((shelf_id, now_selected));
        }
    });
    if let Some((shelf_id, now_selected)) = toggled {
        apply_shelf_toggle(conn, toasts, fic_id, shelf_id, now_selected);
        shelves_changed = true;
    }

    ui.add_space(4.0);
    ui.label(RichText::new("Notes").weak());
    draw_note(ui, fics, idx, conn, toasts);

    ui.add_space(8.0);
    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let delete_btn =
            egui::Button::new(RichText::new("\u{1F5D1}  Delete Fic").color(Color32::WHITE))
                .fill(Color32::from_rgb(150, 35, 35));
        if ui.add(delete_btn).clicked() {
            let repo = SqliteRepository::new(conn);
            match delete_fic(&repo, fic_id) {
                Ok(()) => {
                    fics.retain(|f| f.id != fic_id);
                    toasts.success("Fanfiction deleted");
                    deleted = true;
                }
                Err(err) => toast_error(toasts, "Couldn't delete fanfiction", err),
            }
        }
    });

    YourInfoOutcome {
        shelves_changed,
        deleted,
    }
}

fn apply_shelf_toggle(
    conn: &Connection,
    toasts: &mut Toasts,
    fic_id: u64,
    shelf_id: u64,
    now_selected: bool,
) {
    let repo = SqliteRepository::new(conn);
    let result = if now_selected {
        add_to_shelf(&repo, fic_id, shelf_id)
    } else {
        remove_from_shelf(&repo, fic_id, shelf_id)
    };
    if let Err(err) = result {
        toast_error(
            toasts,
            if now_selected {
                "Couldn't add to shelf"
            } else {
                "Couldn't remove from shelf"
            },
            err,
        );
    }
}

/// Aligned label / value row — label takes a fixed gutter on the left so
/// values line up vertically across the section.
fn kv_row(ui: &mut Ui, label: &str, value: impl FnOnce(&mut Ui)) {
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(80.0, 22.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.label(RichText::new(label).weak());
            },
        );
        value(ui);
    });
}

fn draw_status(
    ui: &mut Ui,
    fics: &mut [Fanfiction],
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let current = fics[idx].reading_status;
    let mut chosen = current;
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
    fics: &mut [Fanfiction],
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let published = fics[idx].chapters_published;
    let total = fics[idx].chapters_total;
    let mut chapter = fics[idx].last_chapter_read.unwrap_or(0);
    let resp = ui.add(DragValue::new(&mut chapter).range(0..=published));
    let total_str = total
        .map(|n| n.to_string())
        .unwrap_or_else(|| "\u{2014}".into());
    ui.label(RichText::new(format!("/ {}", total_str)).weak());
    if resp.changed() {
        let repo = SqliteRepository::new(conn);
        match update_last_chapter_read(&repo, fics[idx].id, chapter) {
            Ok(updated) => fics[idx] = updated,
            Err(err) => toast_error(toasts, "Couldn't update chapter", err),
        }
    }
}

fn draw_read_count(
    ui: &mut Ui,
    fics: &mut [Fanfiction],
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let mut reads = fics[idx].read_count;
    let resp = ui.add(DragValue::new(&mut reads));
    if resp.changed() {
        let repo = SqliteRepository::new(conn);
        match update_read_count(&repo, fics[idx].id, reads) {
            Ok(updated) => fics[idx] = updated,
            Err(err) => toast_error(toasts, "Couldn't update read count", err),
        }
    }
}

fn draw_rating(
    ui: &mut Ui,
    fics: &mut [Fanfiction],
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let mut rating = fics[idx].user_rating;
    if star_rating::star_rating(ui, &mut rating) {
        let repo = SqliteRepository::new(conn);
        match update_user_rating(&repo, fics[idx].id, rating_payload(rating)) {
            Ok(updated) => fics[idx] = updated,
            Err(err) => toast_error(toasts, "Couldn't update rating", err),
        }
    }
}

fn draw_note(
    ui: &mut Ui,
    fics: &mut [Fanfiction],
    idx: usize,
    conn: &Connection,
    toasts: &mut Toasts,
) {
    let mut buf = fics[idx].personal_note.clone().unwrap_or_default();
    let resp = ui.add(
        TextEdit::multiline(&mut buf)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
    // Mirror keystrokes into the in-memory fic so the next frame doesn't
    // overwrite `buf` with the stale stored value.
    if resp.changed() {
        fics[idx].personal_note = if buf.is_empty() {
            None
        } else {
            Some(buf.clone())
        };
    }
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

// ---------------------------------------------------------------------------
// AO3 metadata — header with refresh, key/value rows, expandable bubbles
// ---------------------------------------------------------------------------

fn draw_ao3_metadata(ui: &mut Ui, fic: &Fanfiction, task_executor: &TaskExecutor) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("AO3 METADATA").strong().small());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // ↻ refresh glyph — clicking enqueues a background refresh.
            let resp = ui
                .small_button(RichText::new("\u{21BB}").size(14.0))
                .on_hover_text("Refresh from AO3");
            if resp.clicked() {
                task_executor.enqueue_refresh(fic.id, fic.title.clone());
            }
            ui.label(
                RichText::new(format!("Updated {}", relative_time(fic.last_checked_date)))
                    .weak()
                    .small(),
            );
        });
    });
    ui.add_space(6.0);

    // Bubble lists — each can expand to show all entries when there's
    // more than fits in the default cap.
    if !fic.fandoms.is_empty() {
        ao3_row(ui, "Fandoms", |ui| bubble_list(ui, "fandoms", &fic.fandoms));
    }
    if let Some(rels) = fic.relationships.as_ref().filter(|v| !v.is_empty()) {
        ao3_row(ui, "Relationships", |ui| {
            bubble_list(ui, "relationships", rels)
        });
    }
    if let Some(chars) = fic.characters.as_ref().filter(|v| !v.is_empty()) {
        ao3_row(ui, "Characters", |ui| bubble_list(ui, "characters", chars));
    }
    if let Some(tags) = fic.tags.as_ref().filter(|v| !v.is_empty()) {
        ao3_row(ui, "Additional Tags", |ui| bubble_list(ui, "tags", tags));
    }

    // Single-value rows.
    ao3_row(ui, "Rating", |ui| {
        ui.label(format_ao3_rating(&fic.rating));
    });
    if !fic.warnings.is_empty() {
        let labels: Vec<String> = fic
            .warnings
            .iter()
            .map(|w| format_single_warning(w).to_string())
            .collect();
        ao3_row(ui, "Archive Warning", |ui| {
            bubble_list(ui, "warnings", &labels)
        });
    }
    if let Some(cats) = fic.categories.as_ref().filter(|v| !v.is_empty()) {
        let labels: Vec<String> = cats
            .iter()
            .map(|c| format_category(c).to_string())
            .collect();
        ao3_row(ui, "Category", |ui| bubble_list(ui, "categories", &labels));
    }
    ao3_row(ui, "Words", |ui| {
        ui.label(format_thousands(fic.words));
    });
    ao3_row(ui, "Chapters", |ui| {
        let total = fic
            .chapters_total
            .map(|n| n.to_string())
            .unwrap_or_else(|| "?".into());
        ui.label(format!("{}/{}", fic.chapters_published, total));
    });
    ao3_row(ui, "Kudos", |ui| {
        ui.label(format_thousands(fic.kudos));
    });
    ao3_row(ui, "Hits", |ui| {
        ui.label(format_thousands(fic.hits));
    });
    ao3_row(ui, "Language", |ui| {
        ui.label(&fic.language);
    });
    ao3_row(ui, "Completed", |ui| {
        ui.label(if fic.complete { "Yes" } else { "No" });
    });
    ao3_row(ui, "Published", |ui| {
        ui.label(fic.date_published.format("%Y-%m-%d").to_string());
    });
    ao3_row(ui, "Updated", |ui| {
        ui.label(fic.date_updated.format("%Y-%m-%d").to_string());
    });

    if !fic.summary.trim().is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("Summary").weak());
        ui.add_space(2.0);
        ui.label(&fic.summary);
    }
}

/// Two-column row for the AO3 metadata: 110-px label gutter on the left,
/// value(s) on the right. The wider gutter (vs Your Info's 80) makes
/// room for the longer labels like "Additional Tags". Labels are
/// right-aligned inside the gutter so they all sit flush against the
/// value column — easier to scan vertically than ragged-right labels.
fn ao3_row(ui: &mut Ui, label: &str, value: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(110.0, 22.0),
            Layout::right_to_left(Align::TOP),
            |ui| {
                ui.label(RichText::new(label).weak());
            },
        );
        ui.vertical(|ui| {
            value(ui);
        });
    });
    ui.add_space(4.0);
}

/// Bubble row with overflow expansion. Default-cap of `VISIBLE_DEFAULT`
/// items shown as bubbles; if there are more, the trailing bubble is
/// `+N more` and toggles to expanded on click. State is per-section
/// (keyed by `salt`) and lives in egui temp memory.
fn bubble_list(ui: &mut Ui, salt: &str, items: &[String]) {
    const VISIBLE_DEFAULT: usize = 5;
    let id = ui.id().with(("bubble-expand", salt));
    let mut expanded = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        let limit = if expanded {
            items.len()
        } else {
            items.len().min(VISIBLE_DEFAULT)
        };
        for item in &items[..limit] {
            bubble(ui, item);
        }
        if items.len() > VISIBLE_DEFAULT {
            let label = if expanded {
                "− less".to_string()
            } else {
                format!("+{}", items.len() - VISIBLE_DEFAULT)
            };
            if bubble_clickable(ui, &label).clicked() {
                expanded = !expanded;
            }
        }
    });

    ui.data_mut(|d| d.insert_temp(id, expanded));
}

/// One bubble. Rendered by hand (measure-then-allocate-then-paint) so it
/// behaves as an atomic block in `horizontal_wrapped` — using `Frame` here
/// caused the inner Label to inherit horizontal_wrapped's wrap mode and
/// fracture into one character per line whenever the bubble landed in a
/// tight remaining space.
///
/// Capping the text width at the row's max_rect lets the text wrap
/// inside the bubble when it would otherwise exceed the panel's right
/// edge — much better than overflowing.
fn bubble(ui: &mut Ui, text: &str) {
    let font = egui::FontId::proportional(12.0);
    let text_color = ui.visuals().text_color();
    let pad = egui::vec2(8.0, 2.0);
    let max_text_w = (ui.max_rect().width() - pad.x * 2.0).max(40.0);
    let galley = ui
        .painter()
        .layout(text.to_string(), font, text_color, max_text_w);
    let size = galley.size() + pad * 2.0;
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let fill = ui.visuals().widgets.inactive.weak_bg_fill;
    ui.painter().rect(rect, 10.0, fill, Stroke::NONE);
    ui.painter().galley(rect.min + pad, galley, text_color);
}

fn bubble_clickable(ui: &mut Ui, text: &str) -> egui::Response {
    let font = egui::FontId::proportional(12.0);
    let color = ui.visuals().weak_text_color();
    let pad = egui::vec2(8.0, 2.0);
    let max_text_w = (ui.max_rect().width() - pad.x * 2.0).max(40.0);
    let galley = ui
        .painter()
        .layout(text.to_string(), font, color, max_text_w);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let visuals = ui.style().interact(&resp);
    ui.painter()
        .rect(rect, 10.0, visuals.weak_bg_fill, visuals.bg_stroke);
    ui.painter().galley(rect.min + pad, galley, color);
    resp
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn relative_time(when: DateTime<Utc>) -> String {
    let now = Utc::now();
    let delta = now - when;
    let secs = delta.num_seconds().max(0);
    if secs < 5 {
        return "just now".into();
    }
    if secs < 60 {
        return format!("{}s ago", secs);
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{}m ago", mins);
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{}h ago", hours);
    }
    let days = hours / 24;
    if days < 30 {
        return format!("{}d ago", days);
    }
    let months = days / 30;
    if months < 12 {
        return format!("{}mo ago", months);
    }
    let years = months / 12;
    format!("{}y ago", years)
}

fn format_ao3_rating(r: &Rating) -> &'static str {
    match r {
        Rating::NotRated => "Not Rated",
        Rating::General => "General Audiences",
        Rating::TeenAndUp => "Teen And Up Audiences",
        Rating::Mature => "Mature",
        Rating::Explicit => "Explicit",
    }
}

fn format_single_warning(w: &ArchiveWarnings) -> &'static str {
    match w {
        ArchiveWarnings::NoArchiveWarningsApply => "No Archive Warnings Apply",
        ArchiveWarnings::ChooseNotToUse => "Choose Not To Warn",
        ArchiveWarnings::GraphicDepictionsOfViolence => "Graphic Depictions Of Violence",
        ArchiveWarnings::MajorCharacterDeath => "Major Character Death",
        ArchiveWarnings::RapeNonCon => "Rape/Non-Con",
        ArchiveWarnings::Underage => "Underage",
    }
}

/// AO3 categories with the canonical slash notation: F/F, F/M, M/M
/// (instead of egui's debug-default "FF", "FM", "MM"). Gen / Multi /
/// Other don't get slashes since they aren't pairings.
fn format_category(c: &Categories) -> &'static str {
    match c {
        Categories::FF => "F/F",
        Categories::FM => "F/M",
        Categories::MM => "M/M",
        Categories::Gen => "Gen",
        Categories::Multi => "Multi",
        Categories::Other => "Other",
    }
}

fn toast_error(toasts: &mut Toasts, prefix: &str, err: FicflowError) {
    let message = format!("{}: {}", prefix, err);
    log::warn!("{}", message);
    toasts.error(message);
}
