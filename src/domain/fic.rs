use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
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
    pub date_published: DateTime::<Utc>,
    pub date_updated: DateTime::<Utc>,
    pub last_chapter_read: Option<u32>,      // Custom field
    pub reading_status: ReadingStatus,       // Custom field
    pub read_count: u32,                     // Custom field
    pub user_rating: Option<UserRating>,     // Custom field
    pub personal_note: Option<String>,       // Custom field
    pub last_checked_date: DateTime::<Utc>,  // Custom field (last update check)
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum UserRating {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
}

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum ReadingStatus {
    InProgress,
    Read,
    PlanToRead,
    Paused,
    Abandoned,
}

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum Rating {
    NotRated,
    General,
    TeenAndUp,
    Mature,
    Explicit,
}

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum ArchiveWarnings {
    ChooseNotToUse,
    GraphicDepictionsOfViolence,
    MajorCharacterDeath,
    NoArchiveWarningsApply,
    RapeNonCon,
    Underage,
}

#[derive(Debug, Serialize, Deserialize, Display, PartialEq)]
pub enum Categories {
    FF,
    FM,
    MM,
    Gen,
    Other,
    Multi,
}

pub fn assert_fanfiction_eq(expected: &Fanfiction, actual: &Fanfiction) {
    let mut errors = Vec::new();

    macro_rules! compare_field {
        ($field:ident) => {
            if expected.$field != actual.$field {
                errors.push(format!(
                    "Field `{}` differs:\n  Expected: {:?}\n  Actual:   {:?}",
                    stringify!($field), expected.$field, actual.$field
                ));
            }
        };
    }

    compare_field!(id);
    compare_field!(title);
    compare_field!(authors);
    compare_field!(categories);
    compare_field!(chapters_total);
    compare_field!(chapters_published);
    compare_field!(characters);
    compare_field!(complete);
    compare_field!(fandoms);
    compare_field!(hits);
    compare_field!(kudos);
    compare_field!(language);
    compare_field!(rating);
    compare_field!(relationships);
    compare_field!(restricted);
    compare_field!(summary);
    compare_field!(tags);
    compare_field!(warnings);
    compare_field!(words);
    compare_field!(date_published);
    compare_field!(date_updated);

    if !errors.is_empty() {
        panic!("Fanfiction structs are not equal:\n{}", errors.join("\n"));
    }

}