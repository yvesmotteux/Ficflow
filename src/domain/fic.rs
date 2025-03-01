use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use strum_macros::Display;

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
    pub date_published: NaiveDateTime,
    pub date_updated: NaiveDateTime,
    pub last_chapter_read: Option<u32>,    // Custom field
    pub reading_status: ReadingStatus,     // Custom field
    pub read_count: u32,                   // Custom field
    pub user_rating: Option<UserRating>,   // Custom field
    pub personal_note: Option<String>,     // Custom field
    pub last_checked_date: NaiveDateTime,  // Custom field (last update check)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Display)]
pub enum UserRating {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
}

#[derive(Debug, Serialize, Deserialize, Display)]
pub enum ReadingStatus {
    InProgress,
    Read,
    PlanToRead,
    Paused,
    Abandoned,
}

#[derive(Debug, Serialize, Deserialize, Display)]
pub enum Rating {
    NotRated,
    General,
    TeenAndUp,
    Mature,
    Explicit,
}

#[derive(Debug, Serialize, Deserialize, Display)]
pub enum ArchiveWarnings {
    ChooseNotToUse,
    GraphicDepictionsOfViolence,
    MajorCharacterDeath,
    NoArchiveWarningsApply,
    RapeNonCon,
    Underage,
}

#[derive(Debug, Serialize, Deserialize, Display)]
pub enum Categories {
    FF,
    FM,
    MM,
    Gen,
    Other,
    Multi,
}