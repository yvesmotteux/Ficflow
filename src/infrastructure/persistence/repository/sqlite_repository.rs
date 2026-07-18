use crate::domain::fanfiction::{Fanfiction, FanfictionOps};
use crate::domain::shelf::{AutoShelfCriteria, MAX_SHELF_DEPTH, Shelf, ShelfKind, ShelfOps};
use crate::error::FicflowError;
use crate::infrastructure::persistence::repository::mapping::{row_to_fanfiction, row_to_shelf};
use chrono::Utc;
use rusqlite::{Connection, params};

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

        let reviving = match self.conn.query_row(
            "SELECT deleted_at IS NOT NULL FROM fanfiction WHERE id = ?1",
            params![fic.id],
            |r| r.get::<_, bool>(0),
        ) {
            Ok(b) => b,
            Err(rusqlite::Error::QueryReturnedNoRows) => false,
            Err(e) => return Err(FicflowError::Database(e)),
        };

        if reviving {
            self.conn
                .execute("DELETE FROM fic_shelf WHERE fic_id = ?1", params![fic.id])?;
        }

        self.conn.execute(
            "INSERT OR REPLACE INTO fanfiction (
                id, title, authors, categories, chapters_total, chapters_published, characters,
                complete, fandoms, hits, kudos, language, rating, relationships, restricted,
                summary, tags, warnings, words, date_published, date_updated, last_chapter_read,
                reading_status, read_count, user_rating, personal_note, last_checked_date, deleted_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, NULL)",
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
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE fanfiction SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![fic_id, now],
        )?;
        Ok(())
    }

    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM fanfiction WHERE deleted_at IS NULL ORDER BY title")?;
        let rows = stmt.query_map([], row_to_fanfiction)?;
        let fics = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(fics)
    }

    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM fanfiction WHERE id = ?1 AND deleted_at IS NULL")?;
        stmt.query_row(params![fic_id], row_to_fanfiction)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => FicflowError::NotFound { fic_id },
                other => FicflowError::Database(other),
            })
    }

    fn wipe_database(&self) -> Result<(), FicflowError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE fanfiction SET deleted_at = ?1 WHERE deleted_at IS NULL",
            params![now],
        )?;
        Ok(())
    }
}

impl<'a> SqliteRepository<'a> {
    fn ensure_fanfiction_exists(&self, fic_id: u64) -> Result<(), FicflowError> {
        let count: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM fanfiction WHERE id = ?1 AND deleted_at IS NULL",
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
            "SELECT COUNT(*) FROM shelf WHERE id = ?1 AND deleted_at IS NULL",
            params![shelf_id],
            |r| r.get(0),
        )?;
        if count == 0 {
            return Err(FicflowError::ShelfNotFound { shelf_id });
        }
        Ok(())
    }

    /// 1 for a root shelf, parent's depth + 1 below it.
    fn shelf_depth(&self, shelf_id: u64) -> Result<u32, FicflowError> {
        let depth: i64 = self.conn.query_row(
            "WITH RECURSIVE anc(id) AS ( \
                 SELECT ?1 \
                 UNION \
                 SELECT s.parent_shelf_id FROM shelf s \
                 JOIN anc ON s.id = anc.id \
                 WHERE s.parent_shelf_id IS NOT NULL \
             ) \
             SELECT COUNT(*) FROM anc",
            params![shelf_id],
            |r| r.get(0),
        )?;
        Ok(depth as u32)
    }

    /// `(id, depth-below-shelf_id)` for the shelf and every non-deleted
    /// descendant; the shelf itself is at depth 0.
    fn shelf_subtree(&self, shelf_id: u64) -> Result<Vec<(u64, u32)>, FicflowError> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE subtree(id, depth) AS ( \
                 SELECT id, 0 FROM shelf WHERE id = ?1 AND deleted_at IS NULL \
                 UNION \
                 SELECT s.id, subtree.depth + 1 FROM shelf s \
                 JOIN subtree ON s.parent_shelf_id = subtree.id \
                 WHERE s.deleted_at IS NULL \
             ) \
             SELECT id, depth FROM subtree",
        )?;
        let rows = stmt.query_map(params![shelf_id], |r| {
            Ok((r.get::<_, u64>(0)?, r.get::<_, u32>(1)?))
        })?;
        rows.collect::<Result<_, _>>()
            .map_err(FicflowError::Database)
    }
}

impl<'a> SqliteRepository<'a> {
    fn insert_shelf_row(
        &self,
        name: &str,
        parent_shelf_id: Option<u64>,
        kind: ShelfKind,
    ) -> Result<Shelf, FicflowError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(FicflowError::InvalidInput(
                "shelf name must not be empty".into(),
            ));
        }
        if let Some(parent) = parent_shelf_id {
            self.ensure_shelf_exists(parent)?;
            if matches!(self.get_shelf_by_id(parent)?.kind, ShelfKind::Auto(_)) {
                return Err(FicflowError::InvalidInput(
                    "cannot nest a shelf under an auto-shelf".into(),
                ));
            }
            if self.shelf_depth(parent)? >= MAX_SHELF_DEPTH as u32 {
                return Err(FicflowError::ShelfDepthExceeded {
                    max: MAX_SHELF_DEPTH,
                });
            }
        }

        let (kind_col, criteria_col) = match &kind {
            ShelfKind::Normal => ("normal", None),
            ShelfKind::Auto(criteria) => ("auto", Some(serde_json::to_string(criteria)?)),
        };

        let created_at = Utc::now();
        self.conn.execute(
            "INSERT INTO shelf (name, created_at, parent_shelf_id, kind, auto_criteria) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                trimmed,
                created_at.to_rfc3339(),
                parent_shelf_id,
                kind_col,
                criteria_col
            ],
        )?;
        let id = self.conn.last_insert_rowid() as u64;
        Ok(Shelf {
            id,
            name: trimmed.to_string(),
            parent_shelf_id,
            pinned: false,
            created_at,
            kind,
        })
    }
}

impl<'a> ShelfOps for SqliteRepository<'a> {
    fn create_shelf(
        &self,
        name: &str,
        parent_shelf_id: Option<u64>,
    ) -> Result<Shelf, FicflowError> {
        self.insert_shelf_row(name, parent_shelf_id, ShelfKind::Normal)
    }

    fn upsert_auto_shelf(
        &self,
        shelf_id: Option<u64>,
        name: &str,
        parent_shelf_id: Option<u64>,
        criteria: AutoShelfCriteria,
    ) -> Result<Shelf, FicflowError> {
        let name = if name.trim().is_empty() {
            "Unnamed"
        } else {
            name
        };
        match shelf_id {
            None => self.insert_shelf_row(name, parent_shelf_id, ShelfKind::Auto(criteria)),
            Some(id) => {
                if !matches!(self.get_shelf_by_id(id)?.kind, ShelfKind::Auto(_)) {
                    return Err(FicflowError::InvalidInput(
                        "shelf is not an auto-shelf".into(),
                    ));
                }
                let criteria_json = serde_json::to_string(&criteria)?;
                self.conn.execute(
                    "UPDATE shelf SET name = ?2, auto_criteria = ?3 \
                     WHERE id = ?1 AND deleted_at IS NULL",
                    params![id, name.trim(), criteria_json],
                )?;
                self.get_shelf_by_id(id)
            }
        }
    }

    fn delete_shelf(&self, shelf_id: u64) -> Result<(), FicflowError> {
        let now = Utc::now().to_rfc3339();
        let rows_affected = self.conn.execute(
            "UPDATE shelf SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![shelf_id, now],
        )?;
        if rows_affected == 0 {
            return Err(FicflowError::ShelfNotFound { shelf_id });
        }
        // Children are promoted to the deleted shelf's parent rather
        // than deleted with it.
        self.conn.execute(
            "UPDATE shelf SET parent_shelf_id = \
                 (SELECT parent_shelf_id FROM shelf WHERE id = ?1) \
             WHERE parent_shelf_id = ?1 AND deleted_at IS NULL",
            params![shelf_id],
        )?;
        Ok(())
    }

    fn move_shelf(&self, shelf_id: u64, new_parent: Option<u64>) -> Result<Shelf, FicflowError> {
        self.ensure_shelf_exists(shelf_id)?;
        if let Some(parent) = new_parent {
            if parent != shelf_id {
                self.ensure_shelf_exists(parent)?;
            }
            if matches!(self.get_shelf_by_id(parent)?.kind, ShelfKind::Auto(_)) {
                return Err(FicflowError::InvalidInput(
                    "cannot nest a shelf under an auto-shelf".into(),
                ));
            }
            let subtree = self.shelf_subtree(shelf_id)?;
            if subtree.iter().any(|(id, _)| *id == parent) {
                return Err(FicflowError::ShelfCycle);
            }
            let height = subtree.iter().map(|(_, d)| *d).max().unwrap_or(0);
            if self.shelf_depth(parent)? + 1 + height > MAX_SHELF_DEPTH as u32 {
                return Err(FicflowError::ShelfDepthExceeded {
                    max: MAX_SHELF_DEPTH,
                });
            }
        }
        self.conn.execute(
            "UPDATE shelf SET parent_shelf_id = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![shelf_id, new_parent],
        )?;
        self.get_shelf_by_id(shelf_id)
    }

    fn update_shelf_name(&self, shelf_id: u64, new_name: &str) -> Result<Shelf, FicflowError> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(FicflowError::InvalidInput(
                "shelf name must not be empty".into(),
            ));
        }
        let rows_affected = self.conn.execute(
            "UPDATE shelf SET name = ?1 WHERE id = ?2 AND deleted_at IS NULL",
            params![trimmed, shelf_id],
        )?;
        if rows_affected == 0 {
            return Err(FicflowError::ShelfNotFound { shelf_id });
        }
        self.get_shelf_by_id(shelf_id)
    }

    fn set_shelf_pinned(&self, shelf_id: u64, pinned: bool) -> Result<Shelf, FicflowError> {
        self.ensure_shelf_exists(shelf_id)?;
        self.conn.execute(
            "UPDATE shelf SET pinned = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![shelf_id, pinned],
        )?;
        self.get_shelf_by_id(shelf_id)
    }

    fn list_shelves(&self) -> Result<Vec<Shelf>, FicflowError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_shelf_id, pinned, created_at, kind, auto_criteria FROM shelf \
             WHERE deleted_at IS NULL \
             ORDER BY pinned DESC, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], row_to_shelf)?;
        let shelves = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(shelves)
    }

    fn get_shelf_by_id(&self, shelf_id: u64) -> Result<Shelf, FicflowError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_shelf_id, pinned, created_at, kind, auto_criteria FROM shelf \
             WHERE id = ?1 AND deleted_at IS NULL",
        )?;
        stmt.query_row(params![shelf_id], row_to_shelf)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => FicflowError::ShelfNotFound { shelf_id },
                other => FicflowError::Database(other),
            })
    }

    fn add_fic_to_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError> {
        self.ensure_fanfiction_exists(fic_id)?;
        self.ensure_shelf_exists(shelf_id)?;
        if matches!(self.get_shelf_by_id(shelf_id)?.kind, ShelfKind::Auto(_)) {
            return Err(FicflowError::InvalidInput(
                "cannot add fics to an auto-shelf".into(),
            ));
        }

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
        if matches!(self.get_shelf_by_id(shelf_id)?.kind, ShelfKind::Auto(_)) {
            return Err(FicflowError::InvalidInput(
                "cannot remove fics from an auto-shelf".into(),
            ));
        }

        self.conn.execute(
            "DELETE FROM fic_shelf WHERE fic_id = ?1 AND shelf_id = ?2",
            params![fic_id, shelf_id],
        )?;
        Ok(())
    }

    fn list_fics_in_shelf(&self, shelf_id: u64) -> Result<Vec<Fanfiction>, FicflowError> {
        self.ensure_shelf_exists(shelf_id)?;

        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE subtree(id) AS ( \
                 SELECT id FROM shelf WHERE id = ?1 AND deleted_at IS NULL \
                 UNION \
                 SELECT s.id FROM shelf s \
                 JOIN subtree ON s.parent_shelf_id = subtree.id \
                 WHERE s.deleted_at IS NULL \
             ) \
             SELECT DISTINCT f.* FROM fanfiction f \
             JOIN fic_shelf fs ON fs.fic_id = f.id \
             JOIN subtree ON subtree.id = fs.shelf_id \
             WHERE f.deleted_at IS NULL \
             ORDER BY f.title",
        )?;
        let rows = stmt.query_map(params![shelf_id], row_to_fanfiction)?;
        let fics = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(fics)
    }

    fn list_shelves_for_fic(&self, fic_id: u64) -> Result<Vec<Shelf>, FicflowError> {
        self.ensure_fanfiction_exists(fic_id)?;

        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.parent_shelf_id, s.pinned, s.created_at, s.kind, s.auto_criteria FROM shelf s \
             JOIN fic_shelf fs ON fs.shelf_id = s.id \
             WHERE fs.fic_id = ?1 AND s.deleted_at IS NULL \
             ORDER BY s.name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![fic_id], row_to_shelf)?;
        let shelves = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(shelves)
    }

    fn count_fics_in_shelf(&self, shelf_id: u64) -> Result<usize, FicflowError> {
        self.ensure_shelf_exists(shelf_id)?;

        let count: i64 = self.conn.query_row(
            "WITH RECURSIVE subtree(id) AS ( \
                 SELECT id FROM shelf WHERE id = ?1 AND deleted_at IS NULL \
                 UNION \
                 SELECT s.id FROM shelf s \
                 JOIN subtree ON s.parent_shelf_id = subtree.id \
                 WHERE s.deleted_at IS NULL \
             ) \
             SELECT COUNT(DISTINCT fs.fic_id) FROM fic_shelf fs \
             JOIN fanfiction f ON f.id = fs.fic_id \
             JOIN subtree ON subtree.id = fs.shelf_id \
             WHERE f.deleted_at IS NULL",
            params![shelf_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    fn count_fics_per_shelf(&self) -> Result<std::collections::HashMap<u64, usize>, FicflowError> {
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE anc(ancestor, node) AS ( \
                 SELECT id, id FROM shelf WHERE deleted_at IS NULL \
                 UNION \
                 SELECT anc.ancestor, s.id FROM shelf s \
                 JOIN anc ON s.parent_shelf_id = anc.node \
                 WHERE s.deleted_at IS NULL \
             ) \
             SELECT anc.ancestor, COUNT(DISTINCT fs.fic_id) FROM anc \
             JOIN fic_shelf fs ON fs.shelf_id = anc.node \
             JOIN fanfiction f ON f.id = fs.fic_id \
             WHERE f.deleted_at IS NULL \
             GROUP BY anc.ancestor",
        )?;
        let rows = stmt.query_map([], |row| {
            let shelf_id: i64 = row.get(0)?;
            let count: i64 = row.get(1)?;
            Ok((shelf_id as u64, count as usize))
        })?;
        rows.collect::<Result<_, _>>()
            .map_err(FicflowError::Database)
    }
}
