use rusqlite::{Connection, params, Result};
use std::error::Error;
use crate::domain::fanfiction::Fanfiction;
use crate::infrastructure::persistence::repository::mapping::row_to_fanfiction;

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
        "INSERT OR REPLACE INTO fanfiction (
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
            fic.user_rating.map(|r| r as u32),
            fic.personal_note,
            last_checked_date_str
        ],
    )?;

    Ok(())
}

pub fn delete_fanfiction(conn: &Connection, fic_id: u64) -> Result<(), Box<dyn Error>> {
    conn.execute("DELETE FROM fanfiction WHERE id = ?1", params![fic_id])?;
    Ok(())
}

pub fn get_all_fanfictions(conn: &Connection) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
    let mut stmt = conn.prepare("SELECT * FROM fanfiction ORDER BY title")?;
    let mut fics = Vec::new();
    let rows = stmt.query_map([], |row| row_to_fanfiction(row))?; // query_map already returns Result<T, E>
    
    for result in rows {
        match result {
            Ok(fic) => fics.push(fic),
            Err(e) => return Err(Box::new(e) as Box<dyn Error>), // Propagate the error
        }
    }
    
    Ok(fics)
}

pub fn get_fanfiction_by_id(conn: &Connection, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
    let mut stmt = conn.prepare("SELECT * FROM fanfiction WHERE id = ?1")?;
    
    let fic = stmt.query_row(params![fic_id], |row| row_to_fanfiction(row))
        .map_err(|e| Box::new(e) as Box<dyn Error>)?; // map_err on the Result from query_row
    
    Ok(fic)
}

pub fn update_fanfiction(conn: &Connection, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
    // Convert complex types to JSON strings for storage
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

    // Execute the update query
    conn.execute(
        "UPDATE fanfiction SET 
            title = ?2,
            authors = ?3, 
            categories = ?4, 
            chapters_total = ?5, 
            chapters_published = ?6, 
            characters = ?7, 
            complete = ?8, 
            fandoms = ?9, 
            hits = ?10, 
            kudos = ?11, 
            language = ?12, 
            rating = ?13, 
            relationships = ?14, 
            restricted = ?15, 
            summary = ?16, 
            tags = ?17, 
            warnings = ?18, 
            words = ?19, 
            date_published = ?20, 
            date_updated = ?21, 
            last_chapter_read = ?22,
            reading_status = ?23, 
            read_count = ?24, 
            user_rating = ?25, 
            personal_note = ?26, 
            last_checked_date = ?27
        WHERE id = ?1",
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

pub fn wipe_database(conn: &Connection) -> Result<(), Box<dyn Error>> {
    conn.execute("DELETE FROM fanfiction", [])?;
    Ok(())
}
