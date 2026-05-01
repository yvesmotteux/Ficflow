use std::cmp::Ordering;

use egui::{Align, Layout, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use crate::domain::fanfiction::{Fanfiction, ReadingStatus};
use crate::infrastructure::config::{ColumnKey, SortDirection, SortPref};

const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 22.0;

pub struct LibraryViewState<'a> {
    pub fics: &'a [Fanfiction],
    pub sort: &'a mut SortPref,
    pub search_query: &'a mut String,
    pub visible_columns: &'a [ColumnKey],
}

/// Returns true if `sort` was mutated by a header click — caller persists.
pub fn draw(ui: &mut Ui, state: LibraryViewState<'_>) -> bool {
    let LibraryViewState {
        fics,
        sort,
        search_query,
        visible_columns,
    } = state;

    draw_search_bar(ui, search_query);
    ui.add_space(6.0);

    let visible: Vec<&Fanfiction> = filter_and_sort(fics, search_query, *sort);
    draw_table(ui, &visible, sort, visible_columns)
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
                for col in visible_columns {
                    row.col(|ui| render_cell(ui, fic, *col));
                }
            });
        });
    sort_changed
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
    ui.add(egui::Label::new(text).truncate());
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
        Some(r) => "*".repeat(r as usize),
        None => "-".to_string(),
    }
}

fn filter_and_sort<'a>(fics: &'a [Fanfiction], query: &str, sort: SortPref) -> Vec<&'a Fanfiction> {
    let mut visible: Vec<&Fanfiction> = fics.iter().filter(|f| matches_search(f, query)).collect();
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
