use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub id: u64,
    pub title: String,
    pub authors: Vec<String>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub date_begun: Option<DateTime<Utc>>,
    pub date_updated: Option<DateTime<Utc>>,
    pub words: u32,
    pub works_count: u32,
    pub complete: bool,
    pub bookmarks: u32,
    pub last_checked_date: DateTime<Utc>,
    pub works: Vec<SeriesWork>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesWork {
    pub work_id: u64,
    pub part: u32,
    pub title: String,
    pub authors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeriesRef {
    pub id: u64,
    pub title: String,
    pub part: u32,
}
