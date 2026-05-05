use std::cmp::Ordering;

use egui::{Align, Color32, Layout, RichText, Sense, Stroke, Ui};
use egui_extras::{Column, TableBuilder};

use std::collections::HashSet;

use super::super::config::{ColumnKey, SortDirection, SortPref};
use crate::domain::fanfiction::{ArchiveWarnings, Fanfiction, Rating, ReadingStatus};

use super::super::format::{format_status, format_thousands};
use super::super::selection_controller::SelectionController;
use super::super::theme;
use super::super::view::View;

const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 28.0;

pub struct LibraryViewState<'a> {
    pub fics: &'a [Fanfiction],
    pub sort: &'a mut SortPref,
    pub search_query: &'a str,
    pub visible_columns: &'a [ColumnKey],
    pub selection: &'a mut SelectionController,
    pub view: &'a View,
    pub shelf_members: &'a HashSet<u64>,
}

/// Returns true if `sort` was mutated by a header click — caller persists.
pub fn draw(ui: &mut Ui, state: LibraryViewState<'_>) -> bool {
    let LibraryViewState {
        fics,
        sort,
        search_query,
        visible_columns,
        selection,
        view,
        shelf_members,
    } = state;

    let visible: Vec<&Fanfiction> = filter_and_sort(fics, search_query, *sort, view, shelf_members);
    draw_table(
        ui,
        fics,
        &visible,
        sort,
        visible_columns,
        selection,
        search_query,
    )
}

/// All fic ids that pass the current view filter + search query, in sort order.
/// Used by `Ctrl+A` to select-all-visible without library_view re-rendering.
pub fn visible_ids(
    fics: &[Fanfiction],
    view: &View,
    shelf_members: &HashSet<u64>,
    search_query: &str,
    sort: SortPref,
) -> Vec<u64> {
    filter_and_sort(fics, search_query, sort, view, shelf_members)
        .into_iter()
        .map(|f| f.id)
        .collect()
}

/// Count of fics that match the current view filter + search query. Cheaper
/// than `visible_ids().len()` since it skips the sort + Vec allocation, but
/// produces the same number — used by the central-panel header counter.
pub fn visible_count(
    fics: &[Fanfiction],
    view: &View,
    shelf_members: &HashSet<u64>,
    search_query: &str,
) -> usize {
    fics.iter()
        .filter(|f| view.includes(f, shelf_members))
        .filter(|f| matches_search(f, search_query))
        .count()
}

fn draw_table(
    ui: &mut Ui,
    all_fics: &[Fanfiction],
    fics: &[&Fanfiction],
    sort: &mut SortPref,
    visible_columns: &[ColumnKey],
    selection: &mut SelectionController,
    search_query: &str,
) -> bool {
    if visible_columns.is_empty() {
        ui.label(
            RichText::new("All columns are hidden — open the column picker to enable some.")
                .italics()
                .weak(),
        );
        return false;
    }
    if fics.is_empty() {
        let message = if all_fics.is_empty() {
            "No fanfictions yet. Click \u{201C}+ Add Fic\u{201D} in the header to add one."
        } else if !search_query.trim().is_empty() {
            "No fanfictions match your search."
        } else {
            "No fanfictions in this view yet."
        };
        ui.add_space(8.0);
        ui.label(RichText::new(message).italics().weak());
        return false;
    }

    // Compute auto-fit decision against the *outer* width — before any
    // ScrollArea wrapper expands `ui.available_width()` to infinity.
    let outer_avail = ui.available_width();
    let natural = natural_widths(ui, fics, visible_columns, *sort);
    let auto_fit = natural.iter().sum::<f32>() <= outer_avail;

    if auto_fit {
        build_table(ui, fics, sort, visible_columns, selection, &natural, true)
    } else {
        // Doesn't fit — wrap in a horizontal ScrollArea so the user can
        // pan to overflowed columns. Fixed-width columns only inside,
        // because `Column::remainder()` would expand to the scroll area's
        // unbounded inner width.
        egui::ScrollArea::horizontal()
            .show(ui, |ui| {
                build_table(ui, fics, sort, visible_columns, selection, &natural, false)
            })
            .inner
    }
}

/// Inner table-building. Called once for the auto-fit case (where the
/// table fills the available width) and once inside a horizontal
/// ScrollArea for the overflow case (where the table extends past the
/// viewport and the user pans to see the rest).
fn build_table(
    ui: &mut Ui,
    fics: &[&Fanfiction],
    sort: &mut SortPref,
    visible_columns: &[ColumnKey],
    selection: &mut SelectionController,
    natural: &[f32],
    auto_fit: bool,
) -> bool {
    let mut sort_changed = false;
    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        // `click_and_drag` so a quick click still selects but holding+moving
        // initiates a drag (used for dropping rows onto sidebar shelves).
        .sense(Sense::click_and_drag())
        .cell_layout(Layout::left_to_right(Align::Center));
    for (i, col) in visible_columns.iter().enumerate() {
        let is_last = i == visible_columns.len() - 1;
        let column = if auto_fit && is_last {
            // Last column eats the leftover so the table still fills the
            // panel — `at_least(natural)` keeps it from collapsing narrower
            // than its content needs.
            Column::remainder().at_least(natural[i]).clip(true)
        } else if auto_fit {
            Column::initial(natural[i]).at_least(20.0).clip(true)
        } else {
            // Overflow path: fixed initial widths for every column,
            // including the last, so the table's total width is the sum
            // of column widths and the wrapping ScrollArea can scroll.
            Column::initial(default_initial_width(*col))
                .at_least(default_at_least(*col))
                .clip(true)
        };
        builder = builder.column(column);
    }
    builder
        .header(HEADER_HEIGHT, |mut header| {
            for col in visible_columns {
                if header_cell(&mut header, *col, sort) {
                    sort_changed = true;
                }
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, fics.len(), |mut row| {
                let fic = fics[row.index()];
                let row_idx = row.index();
                row.set_selected(selection.contains(fic.id));
                for col in visible_columns {
                    row.col(|ui| render_cell(ui, fic, *col));
                }
                let resp = row.response();
                if resp.clicked() {
                    let mods = resp.ctx.input(|i| i.modifiers);
                    selection.handle_row_click(fics, row_idx, mods);
                }
                if resp.drag_started() {
                    // If the user starts dragging a row that's already part
                    // of the selection, we drag the whole selection. Else
                    // drag just that row (without changing the selection,
                    // which would feel surprising).
                    let drag_ids: Vec<u64> = if selection.contains(fic.id) {
                        selection.ids_vec()
                    } else {
                        vec![fic.id]
                    };
                    resp.dnd_set_drag_payload(drag_ids);
                }
            });
        });
    sort_changed
}

fn default_initial_width(col: ColumnKey) -> f32 {
    match col {
        ColumnKey::Title => 180.0,
        ColumnKey::Author => 110.0,
        ColumnKey::Fandom => 150.0,
        ColumnKey::Pairing => 150.0,
        ColumnKey::AO3Rating => 100.0,
        ColumnKey::Warnings => 130.0,
        ColumnKey::Status => 110.0,
        ColumnKey::Complete => 70.0,
        ColumnKey::LastChapter => 60.0,
        ColumnKey::Words => 50.0,
        ColumnKey::Kudos => 70.0,
        ColumnKey::Hits => 70.0,
        ColumnKey::Rating => 70.0,
        ColumnKey::Reads => 50.0,
        ColumnKey::Language => 80.0,
        ColumnKey::DatePublished => 90.0,
        ColumnKey::Updated => 90.0,
    }
}

fn default_at_least(col: ColumnKey) -> f32 {
    match col {
        ColumnKey::Reads => 30.0,
        ColumnKey::LastChapter
        | ColumnKey::Rating
        | ColumnKey::Words
        | ColumnKey::Kudos
        | ColumnKey::Hits
        | ColumnKey::Complete => 40.0,
        _ => 60.0,
    }
}

fn header_cell(
    header: &mut egui_extras::TableRow<'_, '_>,
    column: ColumnKey,
    sort: &mut SortPref,
) -> bool {
    // `selectable(false)` keeps the Label from swallowing the click
    // before it reaches the outer cell response (used for sort toggle).
    let (_rect, resp) = header.col(|ui| {
        let text = format!("{}{}", column.label(), sort_glyph(*sort, column));
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                ui.add(
                    egui::Label::new(RichText::new(text).strong().color(theme::ACCENT))
                        .selectable(false),
                );
            },
        );
    });
    if resp.clicked() {
        toggle_sort(sort, column);
        return true;
    }
    false
}

/// Longest of (header text, content text) per column, plus padding —
/// drives the auto-fit-vs-overflow decision.
fn natural_widths(
    ui: &Ui,
    fics: &[&Fanfiction],
    visible_columns: &[ColumnKey],
    sort: SortPref,
) -> Vec<f32> {
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    visible_columns
        .iter()
        .map(|col| {
            let header_text = format!("{}{}", col.label(), sort_glyph(sort, *col));
            let header_w = ui
                .painter()
                .layout_no_wrap(header_text, body_font.clone(), egui::Color32::WHITE)
                .size()
                .x;
            let content_w = fics
                .iter()
                .map(|f| {
                    ui.painter()
                        .layout_no_wrap(cell_text(f, *col), body_font.clone(), egui::Color32::WHITE)
                        .size()
                        .x
                })
                .fold(0.0f32, f32::max);
            header_w.max(content_w) + cell_padding(*col)
        })
        .collect()
}

/// Default 16px gutters; Status reserves extra for the pill's stroke +
/// inner margin (else the badge truncates), narrow numeric columns
/// shrink so short numbers don't waste width on whitespace.
fn cell_padding(col: ColumnKey) -> f32 {
    match col {
        ColumnKey::Status => 16.0 + 24.0,
        ColumnKey::Words | ColumnKey::Reads | ColumnKey::Hits | ColumnKey::Kudos => 8.0,
        _ => 16.0,
    }
}

fn cell_text(fic: &Fanfiction, column: ColumnKey) -> String {
    match column {
        ColumnKey::Title => fic.title.clone(),
        ColumnKey::Author => fic.authors.join(", "),
        ColumnKey::Fandom => first_or_dash(&fic.fandoms),
        ColumnKey::Pairing => fic
            .relationships
            .as_deref()
            .map(first_or_dash)
            .unwrap_or_else(|| "\u{2014}".to_string()),
        ColumnKey::AO3Rating => format_ao3_rating(&fic.rating).to_string(),
        ColumnKey::Warnings => fic
            .warnings
            .first()
            .map(|w| format_warning(w).to_string())
            .unwrap_or_else(|| "\u{2014}".to_string()),
        ColumnKey::Status => format_status(&fic.reading_status).to_string(),
        ColumnKey::Complete => if fic.complete { "Yes" } else { "No" }.to_string(),
        ColumnKey::LastChapter => format_last_chapter(fic),
        ColumnKey::Words => format_thousands(fic.words),
        ColumnKey::Kudos => format_thousands(fic.kudos),
        ColumnKey::Hits => format_thousands(fic.hits),
        ColumnKey::Rating => format_rating(fic),
        ColumnKey::Reads => fic.read_count.to_string(),
        ColumnKey::Language => fic.language.clone(),
        ColumnKey::DatePublished => fic.date_published.format("%Y-%m-%d").to_string(),
        ColumnKey::Updated => fic.date_updated.format("%Y-%m-%d").to_string(),
    }
}

fn first_or_dash(v: &[String]) -> String {
    v.first().cloned().unwrap_or_else(|| "\u{2014}".to_string())
}

fn format_ao3_rating(r: &Rating) -> &'static str {
    match r {
        Rating::NotRated => "Not Rated",
        Rating::General => "General",
        Rating::TeenAndUp => "Teen And Up",
        Rating::Mature => "Mature",
        Rating::Explicit => "Explicit",
    }
}

fn format_warning(w: &ArchiveWarnings) -> &'static str {
    match w {
        ArchiveWarnings::NoArchiveWarningsApply => "\u{2014}",
        ArchiveWarnings::ChooseNotToUse => "Choose Not To Warn",
        ArchiveWarnings::GraphicDepictionsOfViolence => "Graphic Violence",
        ArchiveWarnings::MajorCharacterDeath => "Major Death",
        ArchiveWarnings::RapeNonCon => "Rape/Non-Con",
        ArchiveWarnings::Underage => "Underage",
    }
}

fn render_cell(ui: &mut Ui, fic: &Fanfiction, column: ColumnKey) {
    if matches!(column, ColumnKey::Status) {
        render_status_pill(ui, &fic.reading_status);
        return;
    }
    // `selectable(false)`: a default `Label` swallows row-click events
    // for text-selection.
    let label = egui::Label::new(cell_text(fic, column))
        .truncate()
        .selectable(false);
    if is_centered_column(column) {
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                ui.add(label);
            },
        );
    } else {
        ui.add(label);
    }
}

fn is_centered_column(col: ColumnKey) -> bool {
    matches!(
        col,
        ColumnKey::Complete
            | ColumnKey::LastChapter
            | ColumnKey::Words
            | ColumnKey::Reads
            | ColumnKey::Hits
            | ColumnKey::Kudos
    )
}

/// Painted manually (no `Frame`) so the pill stays at natural size
/// regardless of column width — a `Frame`-based version stretches
/// horizontally with its content rect. Fill must be opaque so the
/// blue row-selection background doesn't bleed through and shift hue.
fn render_status_pill(ui: &mut Ui, status: &ReadingStatus) {
    let palette = status_palette(status);
    let text = format_status(status);
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui.fonts(|f| f.layout_no_wrap(text.to_string(), body_font, palette.accent));

    const INNER_X: f32 = 8.0;
    const INNER_Y: f32 = 4.0;
    let pill_size = galley.size() + egui::vec2(2.0 * INNER_X, 2.0 * INNER_Y);
    let avail = ui.available_rect_before_wrap();
    let pill_rect = egui::Rect::from_center_size(avail.center(), pill_size);

    let painter = ui.painter();
    painter.rect(
        pill_rect,
        egui::Rounding::same(10.0),
        palette.fill,
        Stroke::new(1.0, palette.accent),
    );
    let text_pos = pill_rect.center() - galley.size() / 2.0;
    painter.galley(text_pos, galley, palette.accent);
}

struct StatusPalette {
    /// Opaque dark tint for the pill background.
    fill: Color32,
    /// Saturated hue for the outline and text.
    accent: Color32,
}

fn status_palette(status: &ReadingStatus) -> StatusPalette {
    match status {
        ReadingStatus::InProgress => StatusPalette {
            fill: Color32::from_rgb(20, 30, 50),
            accent: Color32::from_rgb(59, 130, 246),
        },
        ReadingStatus::Read => StatusPalette {
            fill: Color32::from_rgb(20, 35, 25),
            accent: Color32::from_rgb(34, 197, 94),
        },
        ReadingStatus::PlanToRead => StatusPalette {
            fill: Color32::from_rgb(35, 25, 50),
            accent: Color32::from_rgb(168, 85, 247),
        },
        ReadingStatus::Paused => StatusPalette {
            fill: Color32::from_rgb(40, 30, 15),
            accent: Color32::from_rgb(245, 158, 11),
        },
        ReadingStatus::Abandoned => StatusPalette {
            fill: Color32::from_rgb(45, 20, 20),
            accent: Color32::from_rgb(239, 68, 68),
        },
    }
}

/// Toggle direction if same column, else switch column with default-desc.
fn toggle_sort(sort: &mut SortPref, column: ColumnKey) {
    if sort.column == column {
        sort.direction = match sort.direction {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
    } else {
        sort.column = column;
        sort.direction = SortDirection::Descending;
    }
}

fn sort_glyph(sort: SortPref, column: ColumnKey) -> &'static str {
    if sort.column != column {
        return "";
    }
    match sort.direction {
        // ▲ ascending (smallest first), ▼ descending — same convention
        // most table UIs use, just rendered with proper triangle glyphs
        // instead of caret/v stand-ins. Both live in the BMP Geometric
        // Shapes block so the bundled fonts cover them.
        SortDirection::Ascending => " \u{25B2}",
        SortDirection::Descending => " \u{25BC}",
    }
}

fn format_last_chapter(fic: &Fanfiction) -> String {
    match fic.last_chapter_read {
        Some(c) => format!("{} / {}", c, fic.chapters_published),
        None => format!("- / {}", fic.chapters_published),
    }
}

fn format_rating(fic: &Fanfiction) -> String {
    // Always render five stars: filled (★) up to the rating, empty (☆) for
    // the rest. Conveys "rating out of 5" at a glance regardless of value.
    let filled = fic.user_rating.map(|r| r as usize).unwrap_or(0);
    let mut out = String::with_capacity(15);
    for _ in 0..filled {
        out.push('\u{2605}');
    }
    for _ in filled..5 {
        out.push('\u{2606}');
    }
    out
}

fn filter_and_sort<'a>(
    fics: &'a [Fanfiction],
    query: &str,
    sort: SortPref,
    view: &View,
    shelf_members: &HashSet<u64>,
) -> Vec<&'a Fanfiction> {
    let mut visible: Vec<&Fanfiction> = fics
        .iter()
        .filter(|f| view.includes(f, shelf_members))
        .filter(|f| matches_search(f, query))
        .collect();
    visible.sort_by(|a, b| {
        let ord = compare(a, b, sort.column);
        match sort.direction {
            SortDirection::Ascending => ord,
            SortDirection::Descending => ord.reverse(),
        }
    });
    visible
}

fn matches_search(fic: &Fanfiction, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let needle = |s: &str| s.to_lowercase().contains(&q);
    needle(&fic.title)
        || fic.authors.iter().any(|s| needle(s))
        || fic.fandoms.iter().any(|s| needle(s))
        || fic
            .characters
            .as_deref()
            .is_some_and(|v| v.iter().any(|s| needle(s)))
        || fic
            .relationships
            .as_deref()
            .is_some_and(|v| v.iter().any(|s| needle(s)))
        || fic
            .tags
            .as_deref()
            .is_some_and(|v| v.iter().any(|s| needle(s)))
}

fn compare(a: &Fanfiction, b: &Fanfiction, column: ColumnKey) -> Ordering {
    match column {
        ColumnKey::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        ColumnKey::Author => a
            .authors
            .first()
            .map(|s| s.to_lowercase())
            .cmp(&b.authors.first().map(|s| s.to_lowercase())),
        ColumnKey::Fandom => first_lower(&a.fandoms).cmp(&first_lower(&b.fandoms)),
        ColumnKey::Pairing => a
            .relationships
            .as_deref()
            .map(first_lower)
            .cmp(&b.relationships.as_deref().map(first_lower)),
        ColumnKey::AO3Rating => ao3_rating_order(&a.rating).cmp(&ao3_rating_order(&b.rating)),
        ColumnKey::Warnings => a
            .warnings
            .first()
            .map(format_warning)
            .cmp(&b.warnings.first().map(format_warning)),
        ColumnKey::Status => status_order(&a.reading_status).cmp(&status_order(&b.reading_status)),
        ColumnKey::Complete => a.complete.cmp(&b.complete),
        ColumnKey::LastChapter => a.last_chapter_read.cmp(&b.last_chapter_read),
        ColumnKey::Words => a.words.cmp(&b.words),
        ColumnKey::Kudos => a.kudos.cmp(&b.kudos),
        ColumnKey::Hits => a.hits.cmp(&b.hits),
        ColumnKey::Rating => a
            .user_rating
            .map(|r| r as u8)
            .cmp(&b.user_rating.map(|r| r as u8)),
        ColumnKey::Reads => a.read_count.cmp(&b.read_count),
        ColumnKey::Language => a.language.to_lowercase().cmp(&b.language.to_lowercase()),
        ColumnKey::DatePublished => a.date_published.cmp(&b.date_published),
        ColumnKey::Updated => a.date_updated.cmp(&b.date_updated),
    }
}

fn first_lower(v: &[String]) -> Option<String> {
    v.first().map(|s| s.to_lowercase())
}

fn ao3_rating_order(r: &Rating) -> u8 {
    match r {
        Rating::NotRated => 0,
        Rating::General => 1,
        Rating::TeenAndUp => 2,
        Rating::Mature => 3,
        Rating::Explicit => 4,
    }
}

fn status_order(s: &ReadingStatus) -> u8 {
    match s {
        ReadingStatus::InProgress => 0,
        ReadingStatus::Read => 1,
        ReadingStatus::PlanToRead => 2,
        ReadingStatus::Paused => 3,
        ReadingStatus::Abandoned => 4,
    }
}
