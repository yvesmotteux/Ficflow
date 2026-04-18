use rusqlite::{Connection, params};
use crate::domain::fanfiction::{DatabaseOps, Fanfiction};
use crate::error::FicflowError;
use crate::infrastructure::persistence::repository::mapping::row_to_fanfiction;

pub struct SqliteRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl<'a> DatabaseOps for SqliteRepository<'a> {
    fn save_fanfiction(&self, fic: &Fanfiction) -> Result<(), FicflowError> {
        let authors = serde_json::to_string(&fic.authors)?;
        let categories = serde_json::to_string(&fic.categories)?;
        let characters = serde_json::to_string(&fic.characters)?;
        let fandoms = serde_json::to_string(&fic.fandoms)?;
        let relationships = serde_json::to_string(&fic.relationships)?;
        let tags = serde_json::to_string(&fic.tags)?;
        let warnings = serde_json::to_string(&fic.warnings)?;

        let date_published_str = fic.date_published.to_rfc3339();
        let date_updated_str = fic.date_updated.to_rfc3339();
        let last_checked_date_str = fic.last_checked_date.to_rfc3339();

        self.conn.execute(
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

    fn delete_fanfiction(&self, fic_id: u64) -> Result<(), FicflowError> {
        self.conn.execute("DELETE FROM fanfiction WHERE id = ?1", params![fic_id])?;
        Ok(())
    }

    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, FicflowError> {
        let mut stmt = self.conn.prepare("SELECT * FROM fanfiction ORDER BY title")?;
        let rows = stmt.query_map([], row_to_fanfiction)?;
        let fics = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(fics)
    }

    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, FicflowError> {
        let mut stmt = self.conn.prepare("SELECT * FROM fanfiction WHERE id = ?1")?;
        stmt.query_row(params![fic_id], row_to_fanfiction)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => FicflowError::NotFound { fic_id },
                other => FicflowError::Database(other),
            })
    }

    fn wipe_database(&self) -> Result<(), FicflowError> {
        self.conn.execute("DELETE FROM fanfiction", [])?;
        Ok(())
    }
}
