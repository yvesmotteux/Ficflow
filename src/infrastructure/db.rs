use rusqlite::{Connection, params, Result};
use serde_json;
use std::error::Error;
use crate::domain::fic::Fanfiction;
use crate::infrastructure::migration::run_migrations;

pub fn establish_connection() -> Result<Connection, Box<dyn Error>> {
    let mut conn = Connection::open("fanfictions.db")?;
    run_migrations(&mut conn)?;
    Ok(conn)
}

pub fn insert_fanfiction(conn: &Connection, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
    let authors = serde_json::to_string(&fic.authors)?;
    let categories = serde_json::to_string(&fic.categories)?;
    let characters = serde_json::to_string(&fic.characters)?;
    let fandoms = serde_json::to_string(&fic.fandoms)?;
    let relationships = serde_json::to_string(&fic.relationships)?;
    let tags = serde_json::to_string(&fic.tags)?;
    let warnings = serde_json::to_string(&fic.warnings)?;

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
            fic.date_published.to_string(),
            fic.date_updated.to_string(),
            fic.last_chapter_read,
            fic.reading_status.to_string(),
            fic.read_count,
            fic.user_rating.as_ref().map(|r| *r as i32),
            fic.personal_note,
            fic.last_checked_date.to_string(),
        ],
    )?;
    Ok(())
}

pub fn delete_fanfiction(conn: &Connection, fic_id: u64) -> Result<()> {
    conn.execute("DELETE FROM fanfiction WHERE id = ?1", params![fic_id])?;
    Ok(())
}