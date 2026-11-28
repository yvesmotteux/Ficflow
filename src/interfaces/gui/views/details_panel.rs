//! Pure presentation: read-only view of the fic + shelves in,
//! single `Outcome` out. No `&Connection`, no `&mut Toasts`, no
//! `application::*` — caller dispatches the outcome.

use std::collections::HashSet;

use chrono::Utc;
use egui::{
    Align, Color32, ComboBox, DragValue, Layout, RichText, ScrollArea, Sense, Stroke, StrokeKind,
    TextEdit, Ui,
};

use crate::domain::fanfiction::{
    ArchiveWarnings, Categories, Fanfiction, Rating, ReadingStatus, UserRating,
};
use crate::domain::shelf::Shelf;

use super::super::format::{ao3_work_url, format_status, relative_time};
use super::super::widgets::shelves_dropdown::{self, DropdownOutcome};
use super::super::widgets::star_rating;
use super::modals::shelf_modals::ClauseFieldKind;
use crate::interfaces::utils::formatter::format_thousands;

pub struct DetailsState<'a> {
    pub fic: &'a Fanfiction,
    pub all_shelves: &'a [Shelf],
    pub selection_shelf_ids: &'a HashSet<u64>,
    pub has_epub: bool,
}

/// At most one outcome per frame — the panel's controls are
/// mutually exclusive within a click (the textarea emits `SetNote`
/// on focus loss).
pub enum Outcome {
    None,
    SetStatus(ReadingStatus),
    SetLastChapter(u32),
    SetReadCount(u32),
    SetUserRating(Option<UserRating>),
    /// `None` means "clear the note" (NULL in DB).
    SetNote(Option<String>),
    AddToShelf(u64),
    RemoveFromShelf(u64),
    RequestDelete,
    RequestRefresh,
    OpenEpub,
    CreateAutoShelfFromTag(ClauseFieldKind, String),
}

impl Outcome {
    fn replace_with(&mut self, other: Outcome) {
        if !matches!(other, Outcome::None) {
            *self = other;
        }
    }
}

pub fn draw(ui: &mut Ui, state: DetailsState<'_>) -> Outcome {
    let DetailsState {
        fic,
        all_shelves,
        selection_shelf_ids,
        has_epub,
    } = state;

    let mut outcome = Outcome::None;

    // Bottom-pinned: Delete Fic. Registered first so it claims the
    // lowest band and stays visible regardless of how the sections
    // above are sized or scrolled.
    egui::Panel::bottom("details-delete")
        .resizable(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let delete_btn =
                    egui::Button::new(RichText::new("\u{1F5D1}  Delete Fic").color(Color32::WHITE))
                        .fill(Color32::from_rgb(150, 35, 35));
                if ui.add(delete_btn).clicked() {
                    outcome = Outcome::RequestDelete;
                }
            });
        });

    // Bottom: Your Info (status / chapter / reads / rating / shelves /
    // notes). Starts compact so an empty note leaves little dead space,
    // stays user-resizable via the drag handle, and can be dragged up to
    // just below the fic header + "AO3 METADATA" heading — the reserve
    // keeps those visible so the section can't grow over them.
    let max_h = (ui.available_height() - 150.0).max(180.0);
    egui::Panel::bottom("details-your-info")
        .resizable(true)
        .default_size(180.0)
        .size_range(120.0..=max_h)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 6)))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    outcome.replace_with(draw_your_info(ui, fic, all_shelves, selection_shelf_ids));
                });
        });

    // Top-pinned header (title, author with link, fic URL).
    egui::Panel::top("details-header")
        .resizable(false)
        .show_separator_line(true)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 8)))
        .show(ui, |ui| {
            outcome.replace_with(draw_header(ui, fic, has_epub));
        });

    // Central scrollable: AO3 Metadata.
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(8, 4)))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    outcome.replace_with(draw_ao3_metadata(ui, fic));
                });
        });

    outcome
}

// ---------------------------------------------------------------------------
// Header — title, author with AO3-author link, full fic URL
// ---------------------------------------------------------------------------

fn draw_header(ui: &mut Ui, fic: &Fanfiction, has_epub: bool) -> Outcome {
    let mut outcome = Outcome::None;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&fic.title).heading().strong());
        if has_epub
            && ui
                .button("\u{1F4D6}")
                .on_hover_text("Open the saved EPUB")
                .clicked()
        {
            outcome = Outcome::OpenEpub;
        }
    });
    ui.add_space(2.0);

    // Authors with a small "open on AO3" arrow next to each.
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label(RichText::new("by").weak());
        for (i, author) in fic.authors.iter().enumerate() {
            if i > 0 {
                ui.label(",");
            }
            let resp = ui.add(egui::Label::new(author).sense(Sense::click()));
            resp.context_menu(|ui| {
                if ui.button("Create auto-shelf from this author").clicked() {
                    outcome =
                        Outcome::CreateAutoShelfFromTag(ClauseFieldKind::Author, author.clone());
                    ui.close();
                }
            });
            let orphaned = author == "orphan_account" || author.ends_with(" (orphan_account)");
            if author != "Anonymous" && !orphaned {
                // Anonymous and orphaned works have no browsable works page on AO3.
                ui.hyperlink_to(
                    "\u{2197}",
                    format!("https://archiveofourown.org/users/{}/works", author),
                );
            }
        }
    });

    ui.add_space(4.0);
    let url = ao3_work_url(fic.id, None);
    ui.hyperlink_to(RichText::new(&url).small(), &url);
    outcome
}

// ---------------------------------------------------------------------------
// Your Info — status / chapter / reads / rating / notes / shelves / delete
// ---------------------------------------------------------------------------

fn draw_your_info(
    ui: &mut Ui,
    fic: &Fanfiction,
    all_shelves: &[Shelf],
    selection_shelf_ids: &HashSet<u64>,
) -> Outcome {
    let mut outcome = Outcome::None;

    ui.label(RichText::new("YOUR INFO").strong().small());
    ui.add_space(4.0);

    kv_row(ui, "Status", |ui| {
        if let Some(status) = draw_status(ui, fic) {
            outcome = Outcome::SetStatus(status);
        }
    });
    kv_row(ui, "Chapter", |ui| {
        if let Some(n) = draw_chapter(ui, fic) {
            outcome = Outcome::SetLastChapter(n);
        }
    });
    kv_row(ui, "Reads", |ui| {
        if let Some(n) = draw_read_count(ui, fic) {
            outcome = Outcome::SetReadCount(n);
        }
    });
    kv_row(ui, "Rating", |ui| {
        if let Some(rating) = draw_rating(ui, fic) {
            outcome = Outcome::SetUserRating(rating);
        }
    });

    kv_row(ui, "Shelves", |ui| {
        let dropdown = shelves_dropdown::shelves_dropdown(
            ui,
            "details-shelves",
            all_shelves,
            selection_shelf_ids,
        );
        if let DropdownOutcome::Toggled {
            shelf_id,
            now_selected,
        } = dropdown
        {
            outcome = if now_selected {
                Outcome::AddToShelf(shelf_id)
            } else {
                Outcome::RemoveFromShelf(shelf_id)
            };
        }
    });

    ui.add_space(4.0);
    ui.label(RichText::new("Notes").weak());
    if let Some(value) = draw_note(ui, fic) {
        outcome = Outcome::SetNote(value);
    }

    outcome
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

fn draw_status(ui: &mut Ui, fic: &Fanfiction) -> Option<ReadingStatus> {
    let current = fic.reading_status;
    let mut chosen = current;
    ComboBox::from_id_salt("status-combo")
        .selected_text(format_status(&chosen))
        .show_ui(ui, |ui| {
            for status in ReadingStatus::ALL {
                ui.selectable_value(&mut chosen, status, format_status(&status));
            }
        });
    (chosen != current).then_some(chosen)
}

fn draw_chapter(ui: &mut Ui, fic: &Fanfiction) -> Option<u32> {
    let published = fic.chapters_published;
    let total = fic.chapters_total;
    let mut chapter = fic.last_chapter_read.unwrap_or(0);
    let resp = ui.add(DragValue::new(&mut chapter).range(0..=published));
    let total_str = total
        .map(|n| n.to_string())
        .unwrap_or_else(|| "\u{2014}".into());
    ui.label(RichText::new(format!("/ {}", total_str)).weak());
    resp.changed().then_some(chapter)
}

fn draw_read_count(ui: &mut Ui, fic: &Fanfiction) -> Option<u32> {
    let mut reads = fic.read_count;
    ui.add(DragValue::new(&mut reads))
        .changed()
        .then_some(reads)
}

fn draw_rating(ui: &mut Ui, fic: &Fanfiction) -> Option<Option<UserRating>> {
    let mut rating = fic.user_rating;
    star_rating::star_rating(ui, &mut rating).then_some(rating)
}

/// Buffer in egui temp memory keyed by `fic.id` so typing persists
/// across frames and resets when the user switches fic. Returns only
/// on focus-loss commit — the DB write is per-edit-session.
fn draw_note(ui: &mut Ui, fic: &Fanfiction) -> Option<Option<String>> {
    let id = ui.scope_id().with(("note-draft", fic.id));
    let mut buf: String = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| fic.personal_note.clone().unwrap_or_default());

    let resp = ui.add(
        TextEdit::multiline(&mut buf)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );

    if resp.changed() {
        ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    }

    resp.lost_focus().then(|| {
        // Drop the draft on commit so a future re-open re-initialises
        // from the DB rather than shadowing a fresh `personal_note`.
        ui.data_mut(|d| d.remove_temp::<String>(id));
        let trimmed = buf.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

// ---------------------------------------------------------------------------
// AO3 metadata — header with refresh, key/value rows, expandable bubbles
// ---------------------------------------------------------------------------

fn draw_ao3_metadata(ui: &mut Ui, fic: &Fanfiction) -> Outcome {
    let mut outcome = Outcome::None;

    ui.horizontal(|ui| {
        ui.label(RichText::new("AO3 METADATA").strong().small());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Gap so the refresh glyph isn't flush with the
            // vertical scrollbar.
            ui.add_space(10.0);
            let resp = ui
                .small_button(RichText::new("\u{21BB}").size(14.0))
                .on_hover_text("Refresh from AO3");
            if resp.clicked() {
                outcome = Outcome::RequestRefresh;
            }
            ui.label(
                RichText::new(format!(
                    "Updated {}",
                    relative_time(fic.last_checked_date, Utc::now())
                ))
                .weak()
                .small(),
            );
        });
    });
    ui.add_space(6.0);

    let tag_rows = [
        ("Fandoms", Some(&fic.fandoms), ClauseFieldKind::Fandom),
        (
            "Relationships",
            fic.relationships.as_ref(),
            ClauseFieldKind::Relationship,
        ),
        (
            "Characters",
            fic.characters.as_ref(),
            ClauseFieldKind::Character,
        ),
        ("Additional Tags", fic.tags.as_ref(), ClauseFieldKind::Tag),
    ];
    let mut tag_shelf_request = None;
    for (label, items, field) in tag_rows {
        if let Some(items) = items.filter(|v| !v.is_empty()) {
            ao3_row(ui, label, |ui| {
                if let Some(request) = bubble_list(ui, label, items, Some(field)) {
                    tag_shelf_request = Some(request);
                }
            });
        }
    }
    if let Some((field, value)) = tag_shelf_request {
        outcome = Outcome::CreateAutoShelfFromTag(field, value);
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
            bubble_list(ui, "warnings", &labels, None);
        });
    }
    if let Some(cats) = fic.categories.as_ref().filter(|v| !v.is_empty()) {
        let labels: Vec<String> = cats
            .iter()
            .map(|c| format_category(c).to_string())
            .collect();
        ao3_row(ui, "Category", |ui| {
            bubble_list(ui, "categories", &labels, None);
        });
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

    outcome
}

/// Two-column row for the AO3 metadata: 110-px label gutter on the left,
/// value(s) on the right. The wider gutter (vs Your Info's 80) makes
/// room for the longer labels like "Additional Tags". Labels are
/// right-aligned inside the gutter so they all sit flush against the
/// value column — easier to scan vertically than ragged-right labels.
pub(super) fn ao3_row(ui: &mut Ui, label: &str, value: impl FnOnce(&mut Ui)) {
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
fn bubble_list(
    ui: &mut Ui,
    salt: &str,
    items: &[String],
    field: Option<ClauseFieldKind>,
) -> Option<(ClauseFieldKind, String)> {
    let mut tag_shelf_request = None;
    const VISIBLE_DEFAULT: usize = 5;
    let id = ui.scope_id().with(("bubble-expand", salt));
    let mut expanded = ui.data(|d| d.get_temp::<bool>(id).unwrap_or(false));

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        let limit = if expanded {
            items.len()
        } else {
            items.len().min(VISIBLE_DEFAULT)
        };
        for item in &items[..limit] {
            let resp = bubble(ui, item, field.is_some());
            if let Some(field) = field {
                resp.context_menu(|ui| {
                    if ui.button("Create auto-shelf from this tag").clicked() {
                        tag_shelf_request = Some((field, item.clone()));
                        ui.close();
                    }
                });
            }
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
    tag_shelf_request
}

/// **Do NOT swap to `egui::Frame`** — the inner Label inherits
/// `horizontal_wrapped`'s wrap mode and fractures into one character
/// per line in tight space. The manual measure → allocate → paint
/// flow keeps the bubble atomic.
///
/// `break_anywhere = true` on the layout job: AO3 tags can contain
/// long unbreakable segments (slash-joined character names, etc.).
/// Without it the layouter renders the text wider than `max_text_w`,
/// the bubble allocates beyond the panel's clip rect.
fn bubble(ui: &mut Ui, text: &str, clickable: bool) -> egui::Response {
    let font = egui::FontId::proportional(12.0);
    let text_color = ui.visuals().text_color();
    let pad = egui::vec2(8.0, 2.0);
    let max_text_w = (ui.max_rect().width() - pad.x * 2.0).max(40.0);
    let galley = layout_break_anywhere(ui, text, font, text_color, max_text_w);
    let size = galley.size() + pad * 2.0;
    let sense = if clickable {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let fill = ui.visuals().widgets.inactive.weak_bg_fill;
    ui.painter()
        .rect(rect, 10.0, fill, Stroke::NONE, StrokeKind::Inside);
    ui.painter().galley(rect.min + pad, galley, text_color);
    resp
}

fn bubble_clickable(ui: &mut Ui, text: &str) -> egui::Response {
    let font = egui::FontId::proportional(12.0);
    let color = ui.visuals().weak_text_color();
    let pad = egui::vec2(8.0, 2.0);
    let max_text_w = (ui.max_rect().width() - pad.x * 2.0).max(40.0);
    let galley = layout_break_anywhere(ui, text, font, color, max_text_w);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let visuals = ui.style().interact(&resp);
    ui.painter().rect(
        rect,
        10.0,
        visuals.weak_bg_fill,
        visuals.bg_stroke,
        StrokeKind::Inside,
    );
    ui.painter().galley(rect.min + pad, galley, color);
    resp
}

fn layout_break_anywhere(
    ui: &Ui,
    text: &str,
    font: egui::FontId,
    color: Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::text::TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    job.wrap.max_width = max_width;
    job.wrap.break_anywhere = true;
    ui.fonts_mut(|f| f.layout_job(job))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
