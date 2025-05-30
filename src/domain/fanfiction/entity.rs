use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

use super::rating::{Rating, ArchiveWarnings, Categories, UserRating};
use super::status::ReadingStatus;

pub trait FanfictionFetcher {
    fn fetch_fanfiction(&self, fic_id: u64, base_url: &str) -> Result<Fanfiction, Box<dyn std::error::Error>>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fanfiction {
    pub id: u64,                           // AO3 ID
    pub title: String,
    pub authors: Vec<String>,
    pub categories: Option<Vec<Categories>>,
    pub chapters_total: Option<u32>,       // None if the fic is unfinished and the author didn't specify the total number of chapters
    pub chapters_published: u32,
    pub characters: Option<Vec<String>>,
    pub complete: bool,
    pub fandoms: Vec<String>,
    pub hits: u32,
    pub kudos: u32,
    pub language: String,
    pub rating: Rating,
    pub relationships: Option<Vec<String>>,
    pub restricted: bool,
    pub summary: String,
    pub tags: Option<Vec<String>>,
    pub warnings: Vec<ArchiveWarnings>,
    pub words: u32,
    pub date_published: DateTime::<Utc>,
    pub date_updated: DateTime::<Utc>,
    pub last_chapter_read: Option<u32>,      // Custom field
    pub reading_status: ReadingStatus,       // Custom field
    pub read_count: u32,                     // Custom field
    pub user_rating: Option<UserRating>,     // Custom field
    pub personal_note: Option<String>,       // Custom field
    pub last_checked_date: DateTime::<Utc>,  // Custom field (last update check)
}
