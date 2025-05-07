use rusqlite::{Connection, params, Result, Row};
use serde_json;
use std::error::Error;
use std::fs;
use std::env;
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use crate::domain::fic::{Fanfiction, Rating, ReadingStatus, UserRating};
use crate::domain::db::DatabaseOps;
use crate::infrastructure::migration::run_migrations;
use dirs_next::data_local_dir;

pub fn establish_connection() -> Result<Connection, Box<dyn Error>> {
    // Check for environment variable override first
    let db_path = if let Ok(path) = env::var("FICFLOW_DB_PATH") {
        PathBuf::from(path)
    } else {
        // Default path in user's data directory
        let mut path = data_local_dir().ok_or("Failed to determine user directory")?;
        path.push("ficflow");
        fs::create_dir_all(&path)?;
        path.push("fanfictions.db");
        path
    };
    
    let mut conn = Connection::open(&db_path)?;
    run_migrations(&mut conn)?;
    Ok(conn)
}

pub struct Database<'a> {
    pub conn: &'a rusqlite::Connection,
}

impl<'a> DatabaseOps for Database<'a> {
    fn insert_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
        insert_fanfiction(self.conn, fic)
    }

    fn delete_fanfiction(&self, fic_id: u64) -> Result<(), Box<dyn Error>> {
        Ok(delete_fanfiction(self.conn, fic_id)?)
    }

    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
        get_all_fanfictions(self.conn)
    }
    
    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
        get_fanfiction_by_id(self.conn, fic_id)
    }
}

fn parse_json_array<T: serde::de::DeserializeOwned>(json_str: &str, fic_id: u64, field_name: &str) -> Result<T, rusqlite::Error> {
    serde_json::from_str(json_str)
        .map_err(|e| rusqlite::Error::InvalidParameterName(
            format!("Invalid {} JSON for fic {}: {}", field_name, fic_id, e)
        ))
}

fn parse_date_time(date_str: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(date_str)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn parse_rating(rating_str: &str) -> Rating {
    match rating_str {
        "NotRated" => Rating::NotRated,
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
    rating_opt.map(|r| match r {
        1 => UserRating::One,
        2 => UserRating::Two,
        3 => UserRating::Three,
        4 => UserRating::Four,
        5 => UserRating::Five,
        _ => UserRating::Three,
    })
}

fn row_to_fanfiction(row: &Row) -> Result<Fanfiction, rusqlite::Error> {
    // Get basic scalar values
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
    
    // Parse JSON arrays
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
    
    // Parse enums
    let rating_str: String = row.get(12)?;
    let rating = parse_rating(&rating_str);
    
    let reading_status_str: String = row.get(22)?;
    let reading_status = parse_reading_status(&reading_status_str);
    
    let user_rating_opt: Option<i32> = row.get(24)?;
    let user_rating = parse_user_rating(user_rating_opt);
    
    // Parse DateTime values
    let date_published_str: String = row.get(19)?;
    let date_published = parse_date_time(&date_published_str);
    
    let date_updated_str: String = row.get(20)?;
    let date_updated = parse_date_time(&date_updated_str);
    
    let last_checked_date_str: String = row.get(26)?;
    let last_checked_date = parse_date_time(&last_checked_date_str);
    
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

pub fn insert_fanfiction(conn: &Connection, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
    let authors = serde_json::to_string(&fic.authors)?;
    let categories = serde_json::to_string(&fic.categories)?;
    let characters = serde_json::to_string(&fic.characters)?;
    let fandoms = serde_json::to_string(&fic.fandoms)?;
    let relationships = serde_json::to_string(&fic.relationships)?;
    let tags = serde_json::to_string(&fic.tags)?;
    let warnings = serde_json::to_string(&fic.warnings)?;

    // Format dates in RFC3339 format for consistent storage and retrieval
    let date_published_str = fic.date_published.to_rfc3339();
    let date_updated_str = fic.date_updated.to_rfc3339();
    let last_checked_date_str = fic.last_checked_date.to_rfc3339();

    conn.execute(
        "INSERT INTO fanfiction (
            id, title, authors, categories, chapters_total, chapters_published, characters, 
            complete, fandoms, hits, kudos, language, rating, relationships, restricted, 
            summary, tags, warnings, words, date_published, date_updated, last_chapter_read, 
            reading_status, read_count, user_rating, personal_note, last_checked_date
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 
            ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27)",
        params![
            fic.id,
            fic.title,
            authors,
            categories,
            fic.chapters_total,
            fic.chapters_published,
            characters,
            fic.complete,
            fandoms,
            fic.hits,
            fic.kudos,
            fic.language,
            fic.rating.to_string(),
            relationships,
            fic.restricted,
            fic.summary,
            tags,
            warnings,
            fic.words,
            date_published_str,
            date_updated_str,
            fic.last_chapter_read,
            fic.reading_status.to_string(),
            fic.read_count,
            fic.user_rating.as_ref().map(|r| *r as i32),
            fic.personal_note,
            last_checked_date_str,
        ],
    )?;
    Ok(())
}

pub fn delete_fanfiction(conn: &Connection, fic_id: u64) -> Result<()> {
    conn.execute("DELETE FROM fanfiction WHERE id = ?1", params![fic_id])?;
    Ok(())
}

pub fn get_all_fanfictions(conn: &Connection) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
    let mut stmt = conn.prepare(
        "SELECT 
            id, title, authors, categories, chapters_total, chapters_published, characters, 
            complete, fandoms, hits, kudos, language, rating, relationships, restricted, 
            summary, tags, warnings, words, date_published, date_updated, last_chapter_read, 
            reading_status, read_count, user_rating, personal_note, last_checked_date
        FROM fanfiction
        ORDER BY title"
    )?;

    let fanfiction_iter = stmt.query_map([], |row| row_to_fanfiction(row))?;

    let mut fanfictions = Vec::new();
    for result in fanfiction_iter {
        match result {
            Ok(fic) => fanfictions.push(fic),
            Err(e) => eprintln!("Error loading fanfiction from database: {}", e),
        }
    }

    Ok(fanfictions)
}

pub fn get_fanfiction_by_id(conn: &Connection, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
    let mut stmt = conn.prepare(
        "SELECT 
            id, title, authors, categories, chapters_total, chapters_published, characters, 
            complete, fandoms, hits, kudos, language, rating, relationships, restricted, 
            summary, tags, warnings, words, date_published, date_updated, last_chapter_read, 
            reading_status, read_count, user_rating, personal_note, last_checked_date
        FROM fanfiction
        WHERE id = ?"
    )?;

    let mut fanfiction_iter = stmt.query_map(params![fic_id], |row| row_to_fanfiction(row))?;

    if let Some(fanfiction) = fanfiction_iter.next() {
        return Ok(fanfiction?);
    }
    
    Err(format!("Fanfiction with ID {} not found", fic_id).into())
}

pub fn wipe_database(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM fanfiction", [])?;
    Ok(())
}