use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use egui::{Align, Color32, Key, Layout, Modifiers, RichText, Sense, Stroke, StrokeKind, Ui};
use egui_extras::{Column, TableBuilder};

use std::collections::HashSet;

use super::super::config::{ColumnKey, SortDirection, SortPref};
use crate::domain::fanfiction::{ArchiveWarnings, Fanfiction, Rating, ReadingStatus};
use crate::domain::series::Series;

use super::super::format::format_status;
use super::super::selection::{RowKey, Selection};
use super::super::selection_controller::SelectionController;
use super::super::theme;
use super::super::view::View;
use super::library_rows::{LibraryRow, RowInputs, build_rows, fic_ids, series_fic_ids};
use crate::interfaces::utils::formatter::format_thousands;

const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 28.0;

pub struct LibraryViewState<'a> {
    pub fics: &'a [Fanfiction],
    pub series: &'a [Series],
    pub collapsed: &'a [u64],
    pub generation: u64,
    pub sort: &'a mut SortPref,
    pub search_query: &'a str,
    pub visible_columns: &'a [ColumnKey],
    pub selection: &'a mut SelectionController,
    pub view: &'a View,
    pub shelf_members: &'a HashSet<u64>,
    pub keyboard_nav: bool,
}

#[derive(Default)]
pub struct TableOutcome {
    pub sort_changed: bool,
    pub column_reorder: Option<(ColumnKey, ColumnKey, bool)>,
    pub column_removed: Option<ColumnKey>,
    pub collapse: Option<(u64, bool)>,
}

/// Filtered + sorted row indices and natural column widths, kept in egui
/// temp memory so they're only recomputed when their inputs change rather
/// than every frame.
struct TableMemo {
    rows_key: u64,
    widths_key: u64,
    rows: Vec<LibraryRow>,
    widths: Vec<f32>,
}

const MEMO_ID: &str = "ficflow-table-memo";

fn rows_key(generation: u64, query: &str, sort: SortPref, view: &View, collapsed: &[u64]) -> u64 {
    let mut hasher = DefaultHasher::new();
    (generation, query.trim(), sort, view, collapsed).hash(&mut hasher);
    hasher.finish()
}

fn widths_key(rows_key: u64, columns: &[ColumnKey], pixels_per_point: f32) -> u64 {
    let mut hasher = DefaultHasher::new();
    (rows_key, columns, pixels_per_point.to_bits()).hash(&mut hasher);
    hasher.finish()
}

fn cached_memo(ctx: &egui::Context) -> Option<Arc<TableMemo>> {
    ctx.data(|d| d.get_temp::<Arc<TableMemo>>(egui::Id::unique(MEMO_ID)))
}

pub fn draw(ui: &mut Ui, state: LibraryViewState<'_>) -> TableOutcome {
    let LibraryViewState {
        fics,
        series,
        collapsed,
        generation,
        sort,
        search_query,
        visible_columns,
        selection,
        view,
        shelf_members,
        keyboard_nav,
    } = state;

    let rows_key = rows_key(generation, search_query, *sort, view, collapsed);
    let widths_key = widths_key(rows_key, visible_columns, ui.ctx().pixels_per_point());
    let memo = match cached_memo(ui.ctx()) {
        Some(memo) if memo.widths_key == widths_key => memo,
        cached => {
            let rows = match cached {
                Some(memo) if memo.rows_key == rows_key => memo.rows.clone(),
                _ => build_rows(&RowInputs {
                    fics,
                    series,
                    view,
                    query: search_query,
                    sort: *sort,
                    collapsed,
                    shelf_members,
                }),
            };
            let cells = Cells { fics, series };
            let widths = natural_widths(ui, &cells, &rows, visible_columns, *sort);
            let memo = Arc::new(TableMemo {
                rows_key,
                widths_key,
                rows,
                widths,
            });
            ui.data_mut(|d| d.insert_temp(egui::Id::unique(MEMO_ID), Arc::clone(&memo)));
            memo
        }
    };

    if visible_columns.is_empty() {
        ui.label(
            RichText::new("All columns are hidden — open the column picker to enable some.")
                .italics()
                .weak(),
        );
        return TableOutcome::default();
    }
    if memo.rows.is_empty() {
        let empty_message = if fics.is_empty() {
            "No fanfictions yet. Click \u{201C}+ Add Fic(s)\u{201D} in the header to add one."
        } else if !search_query.trim().is_empty() {
            "No fanfictions match your search."
        } else {
            "No fanfictions in this view yet."
        };
        ui.add_space(8.0);
        ui.label(RichText::new(empty_message).italics().weak());
        return TableOutcome::default();
    }

    let cells = Cells { fics, series };
    let keys: Vec<RowKey> = memo.rows.iter().map(|r| r.key(fics, series)).collect();
    let mut collapse = None;
    let scroll_to = if keyboard_nav {
        match arrow_key(ui.ctx()) {
            Some(Arrow::Vertical(down)) => selection.step(&keys, down),
            Some(Arrow::Horizontal(open)) => {
                if let Selection::Series(id) = selection.current() {
                    collapse = Some((*id, !open));
                }
                None
            }
            None => None,
        }
    } else {
        None
    };
    let mut outcome = draw_table(
        ui,
        &TableRows {
            cells: &cells,
            rows: &memo.rows,
            keys: &keys,
        },
        sort,
        visible_columns,
        selection,
        &memo.widths,
        scroll_to,
    );
    outcome.collapse = outcome.collapse.or(collapse);
    outcome
}

struct Cells<'a> {
    fics: &'a [Fanfiction],
    series: &'a [Series],
}

struct TableRows<'a> {
    cells: &'a Cells<'a>,
    rows: &'a [LibraryRow],
    keys: &'a [RowKey],
}

/// All fic ids that pass the current view filter + search query, in sort order.
/// Used by `Ctrl+A` to select-all-visible without library_view re-rendering.
pub fn visible_ids(
    fics: &[Fanfiction],
    series: &[Series],
    view: &View,
    shelf_members: &HashSet<u64>,
    search_query: &str,
    sort: SortPref,
) -> Vec<u64> {
    let rows = build_rows(&RowInputs {
        fics,
        series,
        view,
        query: search_query,
        sort,
        collapsed: &[],
        shelf_members,
    });
    fic_ids(&rows, fics)
}

/// Count of fics that match the current view filter + search query. Reads
/// the table memo when it's current, so the header doesn't refilter the
/// library every frame.
pub fn visible_count(
    ctx: &egui::Context,
    fics: &[Fanfiction],
    generation: u64,
    view: &View,
    shelf_members: &HashSet<u64>,
    search_query: &str,
    sort: SortPref,
) -> usize {
    let key = rows_key(generation, search_query, sort, view, &[]);
    match cached_memo(ctx) {
        Some(memo) if memo.rows_key == key => fic_ids(&memo.rows, fics).len(),
        _ => {
            let query = search_query.trim().to_lowercase();
            fics.iter()
                .filter(|f| view.includes(f, shelf_members))
                .filter(|f| matches_search(f, &query))
                .count()
        }
    }
}

fn draw_table(
    ui: &mut Ui,
    table: &TableRows,
    sort: &mut SortPref,
    visible_columns: &[ColumnKey],
    selection: &mut SelectionController,
    natural: &[f32],
    scroll_to: Option<usize>,
) -> TableOutcome {
    // Compute auto-fit decision against the *outer* width — before any
    // ScrollArea wrapper expands `ui.available_width()` to infinity.
    let outer_avail = ui.available_width();
    let auto_fit = natural.iter().sum::<f32>() <= outer_avail;

    if auto_fit {
        build_table(
            ui,
            table,
            sort,
            visible_columns,
            selection,
            Some(natural),
            scroll_to,
        )
    } else {
        // Doesn't fit — wrap in a horizontal ScrollArea so the user can
        // pan to overflowed columns. Fixed-width columns only inside,
        // because `Column::remainder()` would expand to the scroll area's
        // unbounded inner width.
        egui::ScrollArea::horizontal()
            .show(ui, |ui| {
                build_table(ui, table, sort, visible_columns, selection, None, scroll_to)
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
    table: &TableRows,
    sort: &mut SortPref,
    visible_columns: &[ColumnKey],
    selection: &mut SelectionController,
    auto_fit: Option<&[f32]>,
    scroll_to: Option<usize>,
) -> TableOutcome {
    let mut outcome = TableOutcome::default();
    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        // `click_and_drag` so a quick click still selects but holding+moving
        // initiates a drag (used for dropping rows onto sidebar shelves).
        .sense(Sense::click_and_drag())
        .cell_layout(Layout::left_to_right(Align::Center));
    for (i, col) in visible_columns.iter().enumerate() {
        let is_last = i == visible_columns.len() - 1;
        let column = if let Some(natural) = auto_fit
            && is_last
        {
            // Last column eats the leftover so the table still fills the
            // panel — `at_least(natural)` keeps it from collapsing narrower
            // than its content needs.
            Column::remainder().at_least(natural[i]).clip(true)
        } else if let Some(natural) = auto_fit {
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
    if let Some(row) = scroll_to {
        builder = builder.scroll_to_row(row, None);
    }
    builder
        .header(HEADER_HEIGHT, |mut header| {
            for col in visible_columns {
                header_cell(&mut header, *col, sort, &mut outcome);
            }
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, table.rows.len(), |mut row| {
                let row_idx = row.index();
                let library_row = table.rows[row_idx];
                let key = table.keys[row_idx];
                row.set_selected(selection.shows(key));
                let mut arrow = None;
                for col in visible_columns {
                    row.col(|ui| {
                        if let Some(rect) = render_cell(ui, table.cells, library_row, *col) {
                            arrow = Some(rect);
                        }
                    });
                }
                let resp = row.response();
                if resp.clicked() {
                    let on_arrow = arrow
                        .zip(resp.interact_pointer_pos())
                        .is_some_and(|(rect, pos)| rect.expand(4.0).contains(pos));
                    match (library_row, key) {
                        (LibraryRow::Series { collapsed, .. }, RowKey::Series(id)) if on_arrow => {
                            outcome.collapse = Some((id, !collapsed));
                        }
                        _ => {
                            let mods = resp.ctx.input(|i| i.modifiers);
                            selection.handle_row_click(table.keys, row_idx, mods);
                        }
                    }
                }
                if resp.drag_started() {
                    // If the user starts dragging a row that's already part
                    // of the selection, we drag the whole selection. Else
                    // drag just that row (without changing the selection,
                    // which would feel surprising).
                    let drag_ids: Vec<u64> = match key {
                        RowKey::Fic(id) if selection.contains(id) => selection.ids_vec(),
                        RowKey::Fic(id) => vec![id],
                        RowKey::Series(_) => match library_row {
                            LibraryRow::Series { series, .. } => {
                                series_fic_ids(&table.cells.series[series], table.cells.fics)
                            }
                            _ => Vec::new(),
                        },
                        RowKey::Missing { .. } => Vec::new(),
                    };
                    if !drag_ids.is_empty() {
                        resp.dnd_set_drag_payload(drag_ids);
                    }
                }
            });
        });
    outcome
}

enum Arrow {
    Vertical(bool),
    Horizontal(bool),
}

fn arrow_key(ctx: &egui::Context) -> Option<Arrow> {
    if ctx.egui_wants_keyboard_input() {
        return None;
    }
    ctx.input_mut(|i| {
        [
            (Key::ArrowDown, Arrow::Vertical(true)),
            (Key::ArrowUp, Arrow::Vertical(false)),
            (Key::ArrowRight, Arrow::Horizontal(true)),
            (Key::ArrowLeft, Arrow::Horizontal(false)),
        ]
        .into_iter()
        .find(|(key, _)| i.consume_key(Modifiers::NONE, *key))
        .map(|(_, arrow)| arrow)
    })
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
    outcome: &mut TableOutcome,
) {
    // `selectable(false)` keeps the Label from swallowing the click
    // before it reaches the outer cell response (used for sort toggle).
    let (rect, resp) = header.col(|ui| {
        let text = format!("{}{}", column.label(), sort_glyph(*sort, column));
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(text)
                            .strong()
                            .color(theme::accent(ui.visuals())),
                    )
                    .selectable(false),
                );
            },
        );
    });
    if resp.clicked() {
        toggle_sort(sort, column);
        outcome.sort_changed = true;
    }
    if resp.drag_started() {
        resp.dnd_set_drag_payload(column);
    }
    let place_after = resp
        .ctx
        .input(|i| i.pointer.hover_pos())
        .is_some_and(|pos| pos.x > rect.center().x);
    if let Some(dragged) = resp.dnd_hover_payload::<ColumnKey>()
        && *dragged != column
    {
        let x = if place_after {
            rect.right()
        } else {
            rect.left()
        };
        resp.ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::unique("column-reorder-indicator"),
            ))
            .vline(
                x,
                rect.y_range(),
                Stroke::new(2.0_f32, theme::accent(&resp.ctx.global_style().visuals)),
            );
    }
    if let Some(dragged) = resp.dnd_release_payload::<ColumnKey>()
        && *dragged != column
    {
        outcome.column_reorder = Some((*dragged, column, place_after));
    }
    resp.context_menu(|ui| {
        if ui.button("Remove column").clicked() {
            outcome.column_removed = Some(column);
            ui.close();
        }
    });
}

/// Longest of (header text, content text) per column, plus padding —
/// drives the auto-fit-vs-overflow decision.
fn natural_widths(
    ui: &Ui,
    cells: &Cells,
    rows: &[LibraryRow],
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
            let content_w = rows
                .iter()
                .map(|row| {
                    let text = row_text(cells, *row, *col);
                    let indent = if *col == ColumnKey::Title {
                        row_indent(*row)
                    } else {
                        0.0
                    };
                    ui.painter()
                        .layout_no_wrap(text, body_font.clone(), egui::Color32::WHITE)
                        .size()
                        .x
                        + indent
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

const ARROW_WIDTH: f32 = 18.0;
const NESTED_INDENT: f32 = 22.0;

fn row_indent(row: LibraryRow) -> f32 {
    match row {
        LibraryRow::Series { .. } => ARROW_WIDTH,
        LibraryRow::Fic { part: Some(_), .. } | LibraryRow::Missing { .. } => NESTED_INDENT,
        LibraryRow::Fic { part: None, .. } => 0.0,
    }
}

fn row_text(cells: &Cells, row: LibraryRow, column: ColumnKey) -> String {
    match row {
        LibraryRow::Fic { fic, part } => {
            let text = cell_text(&cells.fics[fic], column);
            match part {
                Some((part, total)) if column == ColumnKey::Title => {
                    format!("{part}/{total} \u{00B7} {text}")
                }
                _ => text,
            }
        }
        LibraryRow::Series {
            series,
            local,
            total,
            ..
        } => {
            let series = &cells.series[series];
            match column {
                ColumnKey::Title => format!("{} ({local}/{total})", series.title),
                ColumnKey::Author => series.authors.join(", "),
                ColumnKey::Complete => if series.complete { "Yes" } else { "No" }.to_string(),
                ColumnKey::Words => format_thousands(series.words),
                ColumnKey::DatePublished => format_date(series.date_begun),
                ColumnKey::Updated => format_date(series.date_updated),
                _ => String::new(),
            }
        }
        LibraryRow::Missing {
            series,
            work,
            total,
        } => {
            let work = &cells.series[series].works[work];
            match column {
                ColumnKey::Title => format!("{}/{total} \u{00B7} {}", work.part, work.title),
                ColumnKey::Author => work.authors.join(", "),
                _ => String::new(),
            }
        }
    }
}

fn format_date(date: Option<chrono::DateTime<chrono::Utc>>) -> String {
    date.map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

/// Returns the collapse arrow's rect when this cell drew one.
fn render_cell(
    ui: &mut Ui,
    cells: &Cells,
    row: LibraryRow,
    column: ColumnKey,
) -> Option<egui::Rect> {
    let mut arrow = None;
    if column == ColumnKey::Title {
        match row {
            LibraryRow::Series { collapsed, .. } => {
                let glyph = if collapsed { "\u{25B6}" } else { "\u{25BC}" };
                let resp = ui.add_sized(
                    [ARROW_WIDTH, ui.available_height()],
                    egui::Label::new(RichText::new(glyph).small()).selectable(false),
                );
                arrow = Some(resp.rect);
            }
            _ => ui.add_space(row_indent(row)),
        }
    }
    match row {
        LibraryRow::Fic { fic, .. } if column == ColumnKey::Status => {
            render_status_pill(ui, &cells.fics[fic].reading_status);
            return arrow;
        }
        _ => {}
    }
    let mut text = RichText::new(row_text(cells, row, column));
    match row {
        LibraryRow::Series { .. } => text = text.strong(),
        LibraryRow::Missing { .. } => text = text.italics().weak(),
        LibraryRow::Fic { .. } => {}
    }
    // `selectable(false)`: a default `Label` swallows row-click events
    // for text-selection.
    let label = egui::Label::new(text).truncate().selectable(false);
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
    arrow
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
    let palette = status_palette(status, ui.visuals().dark_mode);
    let text = format_status(status);
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), body_font, palette.accent);

    const INNER_X: f32 = 8.0;
    const INNER_Y: f32 = 4.0;
    let pill_size = galley.size() + egui::vec2(2.0 * INNER_X, 2.0 * INNER_Y);
    let avail = ui.available_rect_before_wrap();
    let pill_rect = egui::Rect::from_center_size(avail.center(), pill_size);

    let painter = ui.painter();
    painter.rect(
        pill_rect,
        egui::CornerRadius::same(10),
        palette.fill,
        Stroke::new(1.0_f32, palette.accent),
        StrokeKind::Inside,
    );
    let text_pos = pill_rect.center() - galley.size() / 2.0;
    painter.galley(text_pos, galley, palette.accent);
}

struct StatusPalette {
    /// Opaque tint for the pill background.
    fill: Color32,
    /// Saturated hue for the outline and text.
    accent: Color32,
}

fn status_palette(status: &ReadingStatus, dark_mode: bool) -> StatusPalette {
    let (fill, accent) = match (status, dark_mode) {
        (ReadingStatus::InProgress, true) => ((20, 30, 50), (59, 130, 246)),
        (ReadingStatus::InProgress, false) => ((219, 234, 254), (29, 78, 216)),
        (ReadingStatus::Read, true) => ((20, 35, 25), (34, 197, 94)),
        (ReadingStatus::Read, false) => ((220, 252, 231), (21, 128, 61)),
        (ReadingStatus::PlanToRead, true) => ((35, 25, 50), (168, 85, 247)),
        (ReadingStatus::PlanToRead, false) => ((243, 232, 255), (126, 34, 206)),
        (ReadingStatus::Paused, true) => ((40, 30, 15), (245, 158, 11)),
        (ReadingStatus::Paused, false) => ((254, 243, 199), (180, 83, 9)),
        (ReadingStatus::Abandoned, true) => ((45, 20, 20), (239, 68, 68)),
        (ReadingStatus::Abandoned, false) => ((254, 226, 226), (185, 28, 28)),
    };
    StatusPalette {
        fill: Color32::from_rgb(fill.0, fill.1, fill.2),
        accent: Color32::from_rgb(accent.0, accent.1, accent.2),
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
    let filled = fic.user_rating.map_or(0, |r| r as usize);
    "\u{2605}".repeat(filled) + &"\u{2606}".repeat(5 - filled)
}

pub(super) fn filter_and_sort(
    fics: &[Fanfiction],
    query: &str,
    sort: SortPref,
    view: &View,
    shelf_members: &HashSet<u64>,
) -> Vec<usize> {
    let query = query.trim().to_lowercase();
    let mut keyed: Vec<(SortKey, usize)> = fics
        .iter()
        .enumerate()
        .filter(|(_, f)| view.includes(f, shelf_members) && matches_search(f, &query))
        .map(|(i, f)| (sort_key(f, sort.column), i))
        .collect();
    keyed.sort_by(|(a, _), (b, _)| match sort.direction {
        SortDirection::Ascending => a.cmp(b),
        SortDirection::Descending => b.cmp(a),
    });
    keyed.into_iter().map(|(_, i)| i).collect()
}

/// `query` must already be trimmed and lowercased.
pub(super) fn matches_search(fic: &Fanfiction, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let needle = |s: &str| s.to_lowercase().contains(query);
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

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SortKey {
    Text(Option<String>),
    Number(Option<u64>),
    Rating(Rating),
    Status(ReadingStatus),
    Date(i64),
}

fn sort_key(fic: &Fanfiction, column: ColumnKey) -> SortKey {
    match column {
        ColumnKey::Title => SortKey::Text(Some(fic.title.to_lowercase())),
        ColumnKey::Author => SortKey::Text(first_lower(&fic.authors)),
        ColumnKey::Fandom => SortKey::Text(first_lower(&fic.fandoms)),
        ColumnKey::Pairing => SortKey::Text(fic.relationships.as_deref().and_then(first_lower)),
        ColumnKey::AO3Rating => SortKey::Rating(fic.rating.clone()),
        ColumnKey::Warnings => {
            SortKey::Text(fic.warnings.first().map(|w| format_warning(w).to_string()))
        }
        ColumnKey::Status => SortKey::Status(fic.reading_status),
        ColumnKey::Complete => SortKey::Number(Some(fic.complete.into())),
        ColumnKey::LastChapter => SortKey::Number(fic.last_chapter_read.map(u64::from)),
        ColumnKey::Words => SortKey::Number(Some(fic.words.into())),
        ColumnKey::Kudos => SortKey::Number(Some(fic.kudos.into())),
        ColumnKey::Hits => SortKey::Number(Some(fic.hits.into())),
        ColumnKey::Rating => SortKey::Number(fic.user_rating.map(|r| r as u64)),
        ColumnKey::Reads => SortKey::Number(Some(fic.read_count.into())),
        ColumnKey::Language => SortKey::Text(Some(fic.language.to_lowercase())),
        ColumnKey::DatePublished => SortKey::Date(fic.date_published.timestamp()),
        ColumnKey::Updated => SortKey::Date(fic.date_updated.timestamp()),
    }
}

fn first_lower(v: &[String]) -> Option<String> {
    v.first().map(|s| s.to_lowercase())
}
