use rusqlite::{Connection, params, Result};
use std::error::Error;
use crate::domain::fic::Fanfiction;
use crate::infrastructure::db::mapping::row_to_fanfiction;

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
    
    // error if not found
    Err(format!("Fanfiction with ID {} not found", fic_id).into())
}

pub fn wipe_database(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM fanfiction", [])?;
    
    Ok(())
}
