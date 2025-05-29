use rusqlite::Row;
use chrono::{DateTime, Utc};
use crate::domain::fanfiction::{Fanfiction, Rating, ReadingStatus, UserRating};

pub fn row_to_fanfiction(row: &Row) -> Result<Fanfiction, rusqlite::Error> {
    let id: u64 = row.get(0)?;
    let title: String = row.get(1)?;
    let chapters_total: Option<u32> = row.get(4)?;
    let chapters_published: u32 = row.get(5)?;
    let complete: bool = row.get(7)?;
    let hits: u32 = row.get(9)?;
    let kudos: u32 = row.get(10)?;
    let language: String = row.get(11)?;
    let restricted: bool = row.get(14)?;
    let summary: String = row.get(15)?;
    let words: u32 = row.get(18)?;
    let last_chapter_read: Option<u32> = row.get(21)?;
    let read_count: u32 = row.get(23)?;
    let personal_note: Option<String> = row.get(25)?;
    
    let authors_json: String = row.get(2)?;
    let authors = parse_json_array(&authors_json, id, "authors")?;
    
    let categories_json: String = row.get(3)?;
    let categories = parse_json_array(&categories_json, id, "categories")?;
    
    let characters_json: String = row.get(6)?;
    let characters = parse_json_array(&characters_json, id, "characters")?;
    
    let fandoms_json: String = row.get(8)?;
    let fandoms = parse_json_array(&fandoms_json, id, "fandoms")?;
    
    let relationships_json: String = row.get(13)?;
    let relationships = parse_json_array(&relationships_json, id, "relationships")?;
    
    let tags_json: String = row.get(16)?;
    let tags = parse_json_array(&tags_json, id, "tags")?;
    
    let warnings_json: String = row.get(17)?;
    let warnings = parse_json_array(&warnings_json, id, "warnings")?;
    
    let rating_str: String = row.get(12)?;
    let rating = parse_rating(&rating_str);
    
    let reading_status_str: String = row.get(22)?;
    let reading_status = parse_reading_status(&reading_status_str);
    
    let user_rating_opt: Option<i32> = row.get(24)?;
    let user_rating = parse_user_rating(user_rating_opt);
    
    let date_published_str: String = row.get(19)?;
    let date_updated_str: String = row.get(20)?;
    let last_checked_date_str: String = row.get(26)?;
    
    let date_published = DateTime::parse_from_rfc3339(&date_published_str)
        .map_err(|_| rusqlite::Error::InvalidColumnType(19, "date_published".into(), 
                                                      rusqlite::types::Type::Text))?
        .with_timezone(&Utc);
        
    let date_updated = DateTime::parse_from_rfc3339(&date_updated_str)
        .map_err(|_| rusqlite::Error::InvalidColumnType(20, "date_updated".into(), 
                                                      rusqlite::types::Type::Text))?
        .with_timezone(&Utc);
        
    let last_checked_date = DateTime::parse_from_rfc3339(&last_checked_date_str)
        .map_err(|_| rusqlite::Error::InvalidColumnType(26, "last_checked_date".into(), 
                                                      rusqlite::types::Type::Text))?
        .with_timezone(&Utc);

    Ok(Fanfiction {
        id,
        title,
        authors,
        categories,
        chapters_total,
        chapters_published,
        characters,
        complete,
        fandoms,
        hits,
        kudos,
        language,
        rating,
        relationships,
        restricted,
        summary,
        tags,
        warnings,
        words,
        date_published,
        date_updated,
        last_chapter_read,
        reading_status,
        read_count,
        user_rating,
        personal_note,
        last_checked_date,
    })
}

fn parse_json_array<T: serde::de::DeserializeOwned>(json: &str, id: u64, field_name: &str) -> Result<T, rusqlite::Error> {
    serde_json::from_str(json)
        .map_err(|_| {
            rusqlite::Error::InvalidParameterName(format!(
                "Error parsing JSON for fic_id={}, field={}: {}",
                id, field_name, json
            ))
        })
}

fn parse_rating(rating_str: &str) -> Rating {
    match rating_str {
        "General" => Rating::General,
        "TeenAndUp" => Rating::TeenAndUp,
        "Mature" => Rating::Mature,
        "Explicit" => Rating::Explicit,
        _ => Rating::NotRated,
    }
}

fn parse_reading_status(status_str: &str) -> ReadingStatus {
    match status_str {
        "InProgress" => ReadingStatus::InProgress,
        "Read" => ReadingStatus::Read,
        "PlanToRead" => ReadingStatus::PlanToRead,
        "Paused" => ReadingStatus::Paused,
        "Abandoned" => ReadingStatus::Abandoned,
        _ => ReadingStatus::PlanToRead,
    }
}

fn parse_user_rating(rating_opt: Option<i32>) -> Option<UserRating> {
    rating_opt.and_then(|r| match r {
        1 => Some(UserRating::One),
        2 => Some(UserRating::Two),
        3 => Some(UserRating::Three),
        4 => Some(UserRating::Four),
        5 => Some(UserRating::Five),
        _ => None,
    })
}
