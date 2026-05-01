use std::cmp::Ordering;

use egui::{Align, Layout, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};

use crate::domain::fanfiction::{Fanfiction, ReadingStatus};

const HEADER_HEIGHT: f32 = 22.0;
const ROW_HEIGHT: f32 = 22.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortColumn {
    Title,
    Author,
    Status,
    LastChapter,
    Rating,
    Reads,
    Updated,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug)]
pub struct SortState {
    pub column: SortColumn,
    pub direction: SortDirection,
}

impl Default for SortState {
    fn default() -> Self {
        Self {
            column: SortColumn::Updated,
            direction: SortDirection::Descending,
        }
    }
}

impl SortState {
    /// Click a header: toggle direction if same column, otherwise switch column
    /// (and default to descending — most recent / highest first feels natural).
    fn click(&mut self, column: SortColumn) {
        if self.column == column {
            self.direction = match self.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
        } else {
            self.column = column;
            self.direction = SortDirection::Descending;
        }
    }

    fn glyph_for(&self, column: SortColumn) -> &'static str {
        // ASCII fallback — default egui font lacks the unicode arrow glyphs;
        // Phase 12 swaps in Comfortaa which supports ▲/▼ properly.
        if self.column != column {
            return "";
        }
        match self.direction {
            SortDirection::Ascending => " ^",
            SortDirection::Descending => " v",
        }
    }
}

pub struct LibraryViewState<'a> {
    pub fics: &'a [Fanfiction],
    pub sort: &'a mut SortState,
    pub search_query: &'a mut String,
}

pub fn draw(ui: &mut Ui, state: LibraryViewState<'_>) {
    let LibraryViewState {
        fics,
        sort,
        search_query,
    } = state;

    draw_search_bar(ui, search_query);
    ui.add_space(6.0);

    let visible: Vec<&Fanfiction> = filter_and_sort(fics, search_query, *sort);
    draw_table(ui, &visible, sort);
}

fn draw_search_bar(ui: &mut Ui, query: &mut String) {
    ui.add(
        egui::TextEdit::singleline(query)
            .hint_text("Search title, author, fandom, characters, relationships, tags…")
            .desired_width(f32::INFINITY),
    );
}

fn draw_table(ui: &mut Ui, fics: &[&Fanfiction], sort: &mut SortState) {
    if fics.is_empty() {
        ui.label(
            RichText::new("No fanfictions match. Add one with the CLI: `ficflow add <fic_id>`.")
                .italics()
                .weak(),
        );
        return;
    }

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::initial(180.0).at_least(80.0).clip(true))
        .column(Column::initial(110.0).at_least(60.0).clip(true))
        .column(Column::initial(90.0).at_least(60.0).clip(true))
        .column(Column::initial(60.0).at_least(40.0).clip(true))
        .column(Column::initial(70.0).at_least(40.0).clip(true))
        .column(Column::initial(50.0).at_least(30.0).clip(true))
        .column(Column::remainder().at_least(80.0).clip(true))
        .header(HEADER_HEIGHT, |mut header| {
            header_cell(&mut header, "Title", SortColumn::Title, sort);
            header_cell(&mut header, "Author", SortColumn::Author, sort);
            header_cell(&mut header, "Status", SortColumn::Status, sort);
            header_cell(&mut header, "Last Ch.", SortColumn::LastChapter, sort);
            header_cell(&mut header, "Rating", SortColumn::Rating, sort);
            header_cell(&mut header, "Reads", SortColumn::Reads, sort);
            header_cell(&mut header, "Updated", SortColumn::Updated, sort);
        })
        .body(|body| {
            body.rows(ROW_HEIGHT, fics.len(), |mut row| {
                let fic = fics[row.index()];
                row.col(|ui| cell(ui, &fic.title));
                row.col(|ui| cell(ui, &fic.authors.join(", ")));
                row.col(|ui| cell(ui, format_status(&fic.reading_status)));
                row.col(|ui| cell(ui, &format_last_chapter(fic)));
                row.col(|ui| cell(ui, &format_rating(fic)));
                row.col(|ui| cell(ui, &fic.read_count.to_string()));
                row.col(|ui| cell(ui, &fic.date_updated.format("%Y-%m-%d").to_string()));
            });
        });
}

fn header_cell(
    header: &mut egui_extras::TableRow<'_, '_>,
    label: &str,
    column: SortColumn,
    sort: &mut SortState,
) {
    header.col(|ui| {
        let text = format!("{}{}", label, sort.glyph_for(column));
        let resp = ui.add(egui::Label::new(RichText::new(text).strong()).sense(Sense::click()));
        if resp.clicked() {
            sort.click(column);
        }
    });
}

fn cell(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(text).truncate());
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
    // Use ASCII '-' instead of the em-dash here too — same default-font reason.
    match fic.last_chapter_read {
        Some(c) => format!("{}/{}", c, fic.chapters_published),
        None => format!("-/{}", fic.chapters_published),
    }
}

fn format_rating(fic: &Fanfiction) -> String {
    // ASCII '*' for stars and '-' for none — Phase 12 swaps in star unicode.
    match fic.user_rating {
        Some(r) => "*".repeat(r as usize),
        None => "-".to_string(),
    }
}

fn filter_and_sort<'a>(
    fics: &'a [Fanfiction],
    query: &str,
    sort: SortState,
) -> Vec<&'a Fanfiction> {
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

fn compare(a: &Fanfiction, b: &Fanfiction, column: SortColumn) -> Ordering {
    match column {
        SortColumn::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        SortColumn::Author => a
            .authors
            .first()
            .map(|s| s.to_lowercase())
            .cmp(&b.authors.first().map(|s| s.to_lowercase())),
        SortColumn::Status => status_order(&a.reading_status).cmp(&status_order(&b.reading_status)),
        SortColumn::LastChapter => a.last_chapter_read.cmp(&b.last_chapter_read),
        SortColumn::Rating => a
            .user_rating
            .map(|r| r as u8)
            .cmp(&b.user_rating.map(|r| r as u8)),
        SortColumn::Reads => a.read_count.cmp(&b.read_count),
        SortColumn::Updated => a.date_updated.cmp(&b.date_updated),
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
