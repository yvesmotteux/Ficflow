use crate::domain::fanfiction::{Fanfiction, FanfictionOps};
use crate::domain::shelf::{Shelf, ShelfOps};
use crate::error::FicflowError;
use crate::infrastructure::persistence::repository::mapping::{row_to_fanfiction, row_to_shelf};
use chrono::Utc;
use rusqlite::{params, Connection};

pub struct SqliteRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl<'a> FanfictionOps for SqliteRepository<'a> {
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
        self.conn
            .execute("DELETE FROM fanfiction WHERE id = ?1", params![fic_id])?;
        Ok(())
    }

    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM fanfiction ORDER BY title")?;
        let rows = stmt.query_map([], row_to_fanfiction)?;
        let fics = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(fics)
    }

    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM fanfiction WHERE id = ?1")?;
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

impl<'a> SqliteRepository<'a> {
    fn ensure_fanfiction_exists(&self, fic_id: u64) -> Result<(), FicflowError> {
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM fanfiction WHERE id = ?1",
            params![fic_id],
            |r| r.get(0),
        )?;
        if count == 0 {
            return Err(FicflowError::NotFound { fic_id });
        }
        Ok(())
    }

    fn ensure_shelf_exists(&self, shelf_id: u64) -> Result<(), FicflowError> {
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM shelf WHERE id = ?1",
            params![shelf_id],
            |r| r.get(0),
        )?;
        if count == 0 {
            return Err(FicflowError::ShelfNotFound { shelf_id });
        }
        Ok(())
    }
}

impl<'a> ShelfOps for SqliteRepository<'a> {
    fn create_shelf(&self, name: &str) -> Result<Shelf, FicflowError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(FicflowError::InvalidInput(
                "shelf name must not be empty".into(),
            ));
        }

        let created_at = Utc::now();
        self.conn.execute(
            "INSERT INTO shelf (name, created_at) VALUES (?1, ?2)",
            params![trimmed, created_at.to_rfc3339()],
        )?;
        let id = self.conn.last_insert_rowid() as u64;
        Ok(Shelf {
            id,
            name: trimmed.to_string(),
            created_at,
        })
    }

    fn delete_shelf(&self, shelf_id: u64) -> Result<(), FicflowError> {
        let rows_affected = self
            .conn
            .execute("DELETE FROM shelf WHERE id = ?1", params![shelf_id])?;
        if rows_affected == 0 {
            return Err(FicflowError::ShelfNotFound { shelf_id });
        }
        Ok(())
    }

    fn list_shelves(&self) -> Result<Vec<Shelf>, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM shelf ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], row_to_shelf)?;
        let shelves = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(shelves)
    }

    fn get_shelf_by_id(&self, shelf_id: u64) -> Result<Shelf, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM shelf WHERE id = ?1")?;
        stmt.query_row(params![shelf_id], row_to_shelf)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => FicflowError::ShelfNotFound { shelf_id },
                other => FicflowError::Database(other),
            })
    }

    fn add_fic_to_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError> {
        self.ensure_fanfiction_exists(fic_id)?;
        self.ensure_shelf_exists(shelf_id)?;

        let added_at = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR IGNORE INTO fic_shelf (fic_id, shelf_id, added_at) VALUES (?1, ?2, ?3)",
            params![fic_id, shelf_id, added_at],
        )?;
        Ok(())
    }

    fn remove_fic_from_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError> {
        self.ensure_fanfiction_exists(fic_id)?;
        self.ensure_shelf_exists(shelf_id)?;

        self.conn.execute(
            "DELETE FROM fic_shelf WHERE fic_id = ?1 AND shelf_id = ?2",
            params![fic_id, shelf_id],
        )?;
        Ok(())
    }

    fn list_fics_in_shelf(&self, shelf_id: u64) -> Result<Vec<Fanfiction>, FicflowError> {
        self.ensure_shelf_exists(shelf_id)?;

        let mut stmt = self.conn.prepare(
            "SELECT f.* FROM fanfiction f \
             JOIN fic_shelf fs ON fs.fic_id = f.id \
             WHERE fs.shelf_id = ?1 \
             ORDER BY f.title",
        )?;
        let rows = stmt.query_map(params![shelf_id], row_to_fanfiction)?;
        let fics = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(fics)
    }
}
