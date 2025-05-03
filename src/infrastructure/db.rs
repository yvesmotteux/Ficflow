use rusqlite::{Connection, params, Result};
use serde_json;
use std::error::Error;
use std::fs;
use crate::domain::fic::Fanfiction;
use crate::domain::db::DatabaseOps;
use crate::infrastructure::migration::run_migrations;
use dirs_next::data_local_dir;

pub fn establish_connection() -> Result<Connection, Box<dyn Error>> {
    let mut db_path = data_local_dir().ok_or("Failed to determine user directory")?;
    db_path.push("ficflow");
    fs::create_dir_all(&db_path)?;
    db_path.push("fanfictions.db");
    
    let mut conn = Connection::open("fanfictions.db")?;
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
        //Ok(get_all_fanfictions(self.conn).map_err(|e| e.to_string())?)
        Ok(Vec::new()) // Placeholder for the actual implementation
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

// pub fn get_all_fanfictions(conn: &Connection) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
//     let mut stmt = conn.prepare(
//         "SELECT 
//             id, title, authors, categories, chapters_total, chapters_published, characters, 
//             complete, fandoms, hits, kudos, language, rating, relationships, restricted, 
//             summary, tags, warnings, words, date_published, date_updated, last_chapter_read, 
//             reading_status, read_count, user_rating, personal_note, last_checked_date
//         FROM fanfiction",
//     )?;

//     let fanfiction_iter = stmt.query_map([], |row| {
//         let authors: String = row.get(2)?;
//         let categories: String = row.get(3)?;
//         let characters: String = row.get(6)?;
//         let fandoms: String = row.get(8)?;
//         let relationships: String = row.get(13)?;
//         let tags: String = row.get(16)?;
//         let warnings: String = row.get(17)?;

//         Ok(Fanfiction {
//             id: row.get(0)?,
//             title: row.get(1)?,
//             authors: serde_json::from_str(&authors).map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             categories: serde_json::from_str(&categories).map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             chapters_total: row.get(4)?,
//             chapters_published: row.get(5)?,
//             characters: serde_json::from_str(&characters).map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             complete: row.get(7)?,
//             fandoms: serde_json::from_str(&fandoms).map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             hits: row.get(9)?,
//             kudos: row.get(10)?,
//             language: row.get(11)?,
//             rating: row.get::<_, String>(12)?.parse().map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             relationships: serde_json::from_str(&relationships).map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             restricted: row.get(14)?,
//             summary: row.get(15)?,
//             tags: serde_json::from_str(&tags).map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             warnings: serde_json::from_str(&warnings).map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             words: row.get(18)?,
//             date_published: row.get::<_, String>(19)?.parse().map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             date_updated: row.get::<_, String>(20)?.parse().map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             last_chapter_read: row.get(21)?,
//             reading_status: row.get::<_, String>(22)?.parse().map_err(|e| Box::new(e) as Box<dyn Error>)?,
//             read_count: row.get(23)?,
//             user_rating: row.get(24)?,
//             personal_note: row.get(25)?,
//             last_checked_date: row.get::<_, String>(26)?.parse().map_err(|e| Box::new(e) as Box<dyn Error>)?,
//         })
//     })?;

//     let mut fanfictions = Vec::new();
//     for fanfiction in fanfiction_iter {
//         fanfictions.push(fanfiction?);
//     }

//     Ok(fanfictions)
// }