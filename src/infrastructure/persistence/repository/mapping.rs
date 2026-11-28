use chrono::{DateTime, Utc};
use rusqlite::Row;
use rusqlite::types::Type;
use serde::de::DeserializeOwned;

use crate::domain::fanfiction::{Fanfiction, Rating, ReadingStatus, UserRating};
use crate::domain::inbox::InboxMessage;
use crate::domain::series::{Series, SeriesWork};
use crate::domain::shelf::{Shelf, ShelfKind};

pub fn row_to_shelf(row: &Row) -> Result<Shelf, rusqlite::Error> {
    let kind = match row.get::<_, String>("kind")?.as_str() {
        "auto" => {
            let json: Option<String> = row.get("auto_criteria")?;
            let criteria = json
                .and_then(|json| serde_json::from_str(&json).ok())
                .ok_or_else(|| invalid_text_column(row, "auto_criteria"))?;
            ShelfKind::Auto(criteria)
        }
        _ => ShelfKind::Normal,
    };
    Ok(Shelf {
        id: row.get("id")?,
        name: row.get("name")?,
        parent_shelf_id: row.get("parent_shelf_id")?,
        pinned: row.get("pinned")?,
        created_at: timestamp(row, "created_at")?,
        kind,
    })
}

pub fn row_to_fanfiction(row: &Row) -> Result<Fanfiction, rusqlite::Error> {
    let id: u64 = row.get("id")?;
    Ok(Fanfiction {
        id,
        title: row.get("title")?,
        authors: json_column(row, id, "authors")?,
        categories: json_column(row, id, "categories")?,
        chapters_total: row.get("chapters_total")?,
        chapters_published: row.get("chapters_published")?,
        characters: json_column(row, id, "characters")?,
        complete: row.get("complete")?,
        fandoms: json_column(row, id, "fandoms")?,
        hits: row.get("hits")?,
        kudos: row.get("kudos")?,
        language: row.get("language")?,
        rating: row
            .get::<_, String>("rating")?
            .parse()
            .unwrap_or(Rating::NotRated),
        relationships: json_column(row, id, "relationships")?,
        restricted: row.get("restricted")?,
        summary: row.get("summary")?,
        tags: json_column(row, id, "tags")?,
        warnings: json_column(row, id, "warnings")?,
        words: row.get("words")?,
        date_published: timestamp(row, "date_published")?,
        date_updated: timestamp(row, "date_updated")?,
        last_chapter_read: row.get("last_chapter_read")?,
        reading_status: row
            .get::<_, String>("reading_status")?
            .parse()
            .unwrap_or(ReadingStatus::PlanToRead),
        read_count: row.get("read_count")?,
        user_rating: row
            .get::<_, Option<i64>>("user_rating")?
            .and_then(|n| u8::try_from(n).ok())
            .and_then(|n| UserRating::try_from(n).ok()),
        personal_note: row.get("personal_note")?,
        last_checked_date: timestamp(row, "last_checked_date")?,
        chapter_ids: Vec::new(),
        series: Vec::new(),
        epub_path: None,
    })
}

pub fn row_to_series(row: &Row) -> Result<Series, rusqlite::Error> {
    let id: u64 = row.get("id")?;
    Ok(Series {
        id,
        title: row.get("title")?,
        authors: json_column(row, id, "authors")?,
        description: row.get("description")?,
        notes: row.get("notes")?,
        date_begun: optional_timestamp(row, "date_begun")?,
        date_updated: optional_timestamp(row, "date_updated")?,
        words: row.get("words")?,
        works_count: row.get("works_count")?,
        complete: row.get("complete")?,
        bookmarks: row.get("bookmarks")?,
        last_checked_date: timestamp(row, "last_checked_date")?,
        works: Vec::new(),
    })
}

pub fn row_to_series_work(row: &Row) -> Result<(u64, SeriesWork), rusqlite::Error> {
    let work_id: u64 = row.get("work_id")?;
    Ok((
        row.get("series_id")?,
        SeriesWork {
            work_id,
            part: row.get("part")?,
            title: row.get("title")?,
            authors: json_column(row, work_id, "authors")?,
        },
    ))
}

pub fn row_to_inbox_message(row: &Row) -> Result<InboxMessage, rusqlite::Error> {
    let content: String = row.get("content")?;
    let read_at = optional_timestamp(row, "read_at")?;
    Ok(InboxMessage {
        id: row.get("id")?,
        created_at: timestamp(row, "created_at")?,
        read_at,
        content: serde_json::from_str(&content).map_err(|_| invalid_text_column(row, "content"))?,
    })
}

fn timestamp(row: &Row, column: &str) -> Result<DateTime<Utc>, rusqlite::Error> {
    let text: String = row.get(column)?;
    DateTime::parse_from_rfc3339(&text)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| invalid_text_column(row, column))
}

fn optional_timestamp(row: &Row, column: &str) -> Result<Option<DateTime<Utc>>, rusqlite::Error> {
    match row.get::<_, Option<String>>(column)? {
        Some(_) => timestamp(row, column).map(Some),
        None => Ok(None),
    }
}

fn json_column<T: DeserializeOwned>(
    row: &Row,
    id: u64,
    column: &str,
) -> Result<T, rusqlite::Error> {
    let json: String = row.get(column)?;
    serde_json::from_str(&json).map_err(|_| {
        rusqlite::Error::InvalidParameterName(format!(
            "Error parsing JSON for fic_id={}, field={}: {}",
            id, column, json
        ))
    })
}

fn invalid_text_column(row: &Row, column: &str) -> rusqlite::Error {
    let index = row.as_ref().column_index(column).unwrap_or(0);
    rusqlite::Error::InvalidColumnType(index, column.into(), Type::Text)
}
