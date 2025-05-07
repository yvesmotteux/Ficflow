use rusqlite::{Connection, params, Result};
use serde_json;
use std::error::Error;
use std::fs;
use chrono::{DateTime, Utc};
use crate::domain::fic::{Fanfiction, Rating, ReadingStatus, UserRating, ArchiveWarnings, Categories};
use crate::domain::db::DatabaseOps;
use crate::infrastructure::migration::run_migrations;
use dirs_next::data_local_dir;

pub fn establish_connection() -> Result<Connection, Box<dyn Error>> {
    let mut db_path = data_local_dir().ok_or("Failed to determine user directory")?;
    db_path.push("ficflow");
    fs::create_dir_all(&db_path)?;
    db_path.push("fanfictions.db");
    
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
        // Call the actual implementation instead of returning an empty vector
        get_all_fanfictions(self.conn)
    }
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

    let fanfiction_iter = stmt.query_map([], |row| {
        let id: u64 = row.get(0)?;
        let title: String = row.get(1)?;
        
        // Parse JSON arrays
        let authors_json: String = row.get(2)?;
        let authors: Vec<String> = serde_json::from_str(&authors_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Invalid authors JSON for fic {}: {}", id, e)))?;
        
        let categories_json: String = row.get(3)?;
        let categories: Option<Vec<Categories>> = serde_json::from_str(&categories_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Invalid categories JSON for fic {}: {}", id, e)))?;
        
        let chapters_total: Option<u32> = row.get(4)?;
        let chapters_published: u32 = row.get(5)?;
        
        let characters_json: String = row.get(6)?;
        let characters: Option<Vec<String>> = serde_json::from_str(&characters_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Invalid characters JSON for fic {}: {}", id, e)))?;
        
        let complete: bool = row.get(7)?;
        
        let fandoms_json: String = row.get(8)?;
        let fandoms: Vec<String> = serde_json::from_str(&fandoms_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Invalid fandoms JSON for fic {}: {}", id, e)))?;
        
        let hits: u32 = row.get(9)?;
        let kudos: u32 = row.get(10)?;
        let language: String = row.get(11)?;
        
        // Parse Rating enum
        let rating_str: String = row.get(12)?;
        let rating = match rating_str.as_str() {
            "NotRated" => Rating::NotRated,
            "General" => Rating::General,
            "TeenAndUp" => Rating::TeenAndUp,
            "Mature" => Rating::Mature,
            "Explicit" => Rating::Explicit,
            _ => Rating::NotRated, // Default value
        };
        
        let relationships_json: String = row.get(13)?;
        let relationships: Option<Vec<String>> = serde_json::from_str(&relationships_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Invalid relationships JSON for fic {}: {}", id, e)))?;
        
        let restricted: bool = row.get(14)?;
        let summary: String = row.get(15)?;
        
        let tags_json: String = row.get(16)?;
        let tags: Option<Vec<String>> = serde_json::from_str(&tags_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Invalid tags JSON for fic {}: {}", id, e)))?;
        
        let warnings_json: String = row.get(17)?;
        let warnings: Vec<ArchiveWarnings> = serde_json::from_str(&warnings_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(format!("Invalid warnings JSON for fic {}: {}", id, e)))?;
        
        let words: u32 = row.get(18)?;
        
        // Parse DateTime values
        let date_published_str: String = row.get(19)?;
        let date_published = DateTime::parse_from_rfc3339(&date_published_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        let date_updated_str: String = row.get(20)?;
        let date_updated = DateTime::parse_from_rfc3339(&date_updated_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
        let last_chapter_read: Option<u32> = row.get(21)?;
        
        // Parse ReadingStatus enum
        let reading_status_str: String = row.get(22)?;
        let reading_status = match reading_status_str.as_str() {
            "InProgress" => ReadingStatus::InProgress,
            "Read" => ReadingStatus::Read,
            "PlanToRead" => ReadingStatus::PlanToRead,
            "Paused" => ReadingStatus::Paused,
            "Abandoned" => ReadingStatus::Abandoned,
            _ => ReadingStatus::PlanToRead, // Default value
        };
        
        let read_count: u32 = row.get(23)?;
        
        // Parse UserRating enum
        let user_rating_opt: Option<i32> = row.get(24)?;
        let user_rating = user_rating_opt.map(|r| match r {
            1 => UserRating::One,
            2 => UserRating::Two,
            3 => UserRating::Three,
            4 => UserRating::Four,
            5 => UserRating::Five,
            _ => UserRating::Three, // Default value
        });
        
        let personal_note: Option<String> = row.get(25)?;
        
        let last_checked_date_str: String = row.get(26)?;
        let last_checked_date = DateTime::parse_from_rfc3339(&last_checked_date_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        
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
    })?;

    let mut fanfictions = Vec::new();
    for result in fanfiction_iter {
        match result {
            Ok(fic) => fanfictions.push(fic),
            Err(e) => eprintln!("Error loading fanfiction from database: {}", e),
        }
    }

    Ok(fanfictions)
}