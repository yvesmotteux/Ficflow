use std::cmp::Ordering;

use egui::{Align, Layout, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use std::collections::HashSet;

use crate::domain::fanfiction::{Fanfiction, ReadingStatus};
use crate::infrastructure::config::{ColumnKey, SortDirection, SortPref};

use super::super::selection::Selection;
use super::super::view::View;

const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 22.0;

pub struct LibraryViewState<'a> {
    pub fics: &'a [Fanfiction],
    pub sort: &'a mut SortPref,
    pub search_query: &'a mut String,
    pub visible_columns: &'a [ColumnKey],
    pub selection: &'a mut Selection,
    pub view: &'a View,
    pub shelf_members: &'a HashSet<u64>,
    /// Anchor row id for shift-click range selection. Updated on plain and
    /// ctrl-clicks; preserved across shift-click extensions so successive
    /// shift-clicks all anchor to the same row.
    pub last_clicked_id: &'a mut Option<u64>,
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
        last_clicked_id,
    } = state;

    draw_search_bar(ui, search_query);
    ui.add_space(6.0);

    let visible: Vec<&Fanfiction> = filter_and_sort(fics, search_query, *sort, view, shelf_members);
    draw_table(
        ui,
        &visible,
        sort,
        visible_columns,
        selection,
        last_clicked_id,
    )
}

fn draw_search_bar(ui: &mut Ui, query: &mut String) {
    ui.add(
        egui::TextEdit::singleline(query)
            .hint_text("Search title, author, fandom, characters, relationships, tags…")
            .desired_width(f32::INFINITY),
    );
}

fn draw_table(
    ui: &mut Ui,
    fics: &[&Fanfiction],
    sort: &mut SortPref,
    visible_columns: &[ColumnKey],
    selection: &mut Selection,
    last_clicked_id: &mut Option<u64>,
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
        ui.label(
            RichText::new("No fanfictions match. Add one with the CLI: `ficflow add <fic_id>`.")
                .italics()
                .weak(),
        );
        return false;
    }

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
        builder = builder.column(column_spec(*col, is_last));
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
                    handle_row_click(selection, last_clicked_id, fics, row_idx, mods);
                }
                if resp.drag_started() {
                    // If the user starts dragging a row that's already part
                    // of the selection, we drag the whole selection. Else
                    // drag just that row (without changing the selection,
                    // which would feel surprising).
                    let drag_ids: Vec<u64> = if selection.contains(fic.id) {
                        selection_to_vec(selection)
                    } else {
                        vec![fic.id]
                    };
                    resp.dnd_set_drag_payload(drag_ids);
                }
            });
        });
    sort_changed
}

/// Resolve a row click into a new selection. `mods` come from the click
/// itself so we honour ctrl/shift correctly across platforms (egui's
/// `Modifiers::command` already maps to Cmd on macOS and Ctrl elsewhere).
fn handle_row_click(
    selection: &mut Selection,
    last_clicked_id: &mut Option<u64>,
    visible: &[&Fanfiction],
    clicked_idx: usize,
    mods: egui::Modifiers,
) {
    let clicked_id = visible[clicked_idx].id;

    if mods.shift {
        // Range select between anchor and clicked row.
        let anchor_id = last_clicked_id.unwrap_or(clicked_id);
        let anchor_idx = visible
            .iter()
            .position(|f| f.id == anchor_id)
            .unwrap_or(clicked_idx);
        let (start, end) = if anchor_idx <= clicked_idx {
            (anchor_idx, clicked_idx)
        } else {
            (clicked_idx, anchor_idx)
        };
        let ids: Vec<u64> = visible[start..=end].iter().map(|f| f.id).collect();
        *selection = match ids.len() {
            1 => Selection::Single(ids[0]),
            _ => Selection::Multi(ids),
        };
        // Anchor stays put across consecutive shift-clicks.
    } else if mods.command {
        let mut current = selection_to_vec(selection);
        if let Some(pos) = current.iter().position(|&id| id == clicked_id) {
            current.remove(pos);
        } else {
            current.push(clicked_id);
        }
        *selection = match current.len() {
            0 => Selection::None,
            1 => Selection::Single(current[0]),
            _ => Selection::Multi(current),
        };
        *last_clicked_id = Some(clicked_id);
    } else {
        *selection = Selection::Single(clicked_id);
        *last_clicked_id = Some(clicked_id);
    }
}

fn selection_to_vec(selection: &Selection) -> Vec<u64> {
    match selection {
        Selection::None => Vec::new(),
        Selection::Single(id) => vec![*id],
        Selection::Multi(ids) => ids.clone(),
    }
}

fn column_spec(col: ColumnKey, is_last: bool) -> Column {
    if is_last {
        return Column::remainder().at_least(60.0).clip(true);
    }
    let initial = match col {
        ColumnKey::Title => 180.0,
        ColumnKey::Author => 110.0,
        ColumnKey::Status => 90.0,
        ColumnKey::LastChapter => 60.0,
        ColumnKey::Rating => 70.0,
        ColumnKey::Reads => 50.0,
        ColumnKey::Updated => 90.0,
    };
    let at_least = match col {
        ColumnKey::Reads => 30.0,
        ColumnKey::LastChapter | ColumnKey::Rating => 40.0,
        _ => 60.0,
    };
    Column::initial(initial).at_least(at_least).clip(true)
}

fn header_cell(
    header: &mut egui_extras::TableRow<'_, '_>,
    column: ColumnKey,
    sort: &mut SortPref,
) -> bool {
    let mut clicked = false;
    header.col(|ui| {
        let text = format!("{}{}", column.label(), sort_glyph(*sort, column));
        let resp = ui.add(egui::Label::new(RichText::new(text).strong()).sense(Sense::click()));
        if resp.clicked() {
            toggle_sort(sort, column);
            clicked = true;
        }
    });
    clicked
}

fn render_cell(ui: &mut Ui, fic: &Fanfiction, column: ColumnKey) {
    let text: String = match column {
        ColumnKey::Title => fic.title.clone(),
        ColumnKey::Author => fic.authors.join(", "),
        ColumnKey::Status => format_status(&fic.reading_status).to_string(),
        ColumnKey::LastChapter => format_last_chapter(fic),
        ColumnKey::Rating => format_rating(fic),
        ColumnKey::Reads => fic.read_count.to_string(),
        ColumnKey::Updated => fic.date_updated.format("%Y-%m-%d").to_string(),
    };
    // `selectable(false)` is essential: by default a `Label` consumes click
    // events for text-selection (drag-to-highlight), which swallows the row's
    // click sense and blocks selecting fics via their text.
    ui.add(egui::Label::new(text).truncate().selectable(false));
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
        SortDirection::Ascending => " ^",
        SortDirection::Descending => " v",
    }
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

fn format_last_chapter(fic: &Fanfiction) -> String {
    match fic.last_chapter_read {
        Some(c) => format!("{}/{}", c, fic.chapters_published),
        None => format!("-/{}", fic.chapters_published),
    }
}

fn format_rating(fic: &Fanfiction) -> String {
    match fic.user_rating {
        Some(r) => "\u{2605}".repeat(r as usize),
        None => "-".to_string(),
    }
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
            .map_or(false, |v| v.iter().any(|s| needle(s)))
        || fic
            .relationships
            .as_deref()
            .map_or(false, |v| v.iter().any(|s| needle(s)))
        || fic
            .tags
            .as_deref()
            .map_or(false, |v| v.iter().any(|s| needle(s)))
}

fn compare(a: &Fanfiction, b: &Fanfiction, column: ColumnKey) -> Ordering {
    match column {
        ColumnKey::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        ColumnKey::Author => a
            .authors
            .first()
            .map(|s| s.to_lowercase())
            .cmp(&b.authors.first().map(|s| s.to_lowercase())),
        ColumnKey::Status => status_order(&a.reading_status).cmp(&status_order(&b.reading_status)),
        ColumnKey::LastChapter => a.last_chapter_read.cmp(&b.last_chapter_read),
        ColumnKey::Rating => a
            .user_rating
            .map(|r| r as u8)
            .cmp(&b.user_rating.map(|r| r as u8)),
        ColumnKey::Reads => a.read_count.cmp(&b.read_count),
        ColumnKey::Updated => a.date_updated.cmp(&b.date_updated),
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
