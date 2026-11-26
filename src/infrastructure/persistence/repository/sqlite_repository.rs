use crate::domain::fanfiction::{Fanfiction, FanfictionOps};
use crate::domain::inbox::{InboxMessage, InboxOps, MessageContent};
use crate::domain::series::{Series, SeriesOps, SeriesRef, SeriesWork};
use crate::domain::shelf::{AutoShelfCriteria, MAX_SHELF_DEPTH, Shelf, ShelfKind, ShelfOps};
use crate::error::FicflowError;
use crate::infrastructure::persistence::repository::mapping::{
    row_to_fanfiction, row_to_inbox_message, row_to_series, row_to_series_work, row_to_shelf,
};
use std::collections::HashMap;

use chrono::Utc;
use rusqlite::{Connection, params};

const SHELF_COLUMNS: &str =
    "s.id, s.name, s.parent_shelf_id, s.pinned, s.created_at, s.kind, s.auto_criteria";

const SUBTREE_CTE: &str = "WITH RECURSIVE subtree(id, depth) AS ( \
         SELECT id, 0 FROM shelf WHERE id = ?1 AND deleted_at IS NULL \
         UNION \
         SELECT s.id, subtree.depth + 1 FROM shelf s \
         JOIN subtree ON s.parent_shelf_id = subtree.id \
         WHERE s.deleted_at IS NULL \
     ) ";

pub struct SqliteRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl FanfictionOps for SqliteRepository<'_> {
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
            "INSERT INTO fanfiction (
                id, title, authors, categories, chapters_total, chapters_published, characters,
                complete, fandoms, hits, kudos, language, rating, relationships, restricted,
                summary, tags, warnings, words, date_published, date_updated, last_chapter_read,
                reading_status, read_count, user_rating, personal_note, last_checked_date, deleted_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, NULL)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                authors = excluded.authors,
                categories = excluded.categories,
                chapters_total = excluded.chapters_total,
                chapters_published = excluded.chapters_published,
                characters = excluded.characters,
                complete = excluded.complete,
                fandoms = excluded.fandoms,
                hits = excluded.hits,
                kudos = excluded.kudos,
                language = excluded.language,
                rating = excluded.rating,
                relationships = excluded.relationships,
                restricted = excluded.restricted,
                summary = excluded.summary,
                tags = excluded.tags,
                warnings = excluded.warnings,
                words = excluded.words,
                date_published = excluded.date_published,
                date_updated = excluded.date_updated,
                last_chapter_read = excluded.last_chapter_read,
                reading_status = excluded.reading_status,
                read_count = excluded.read_count,
                user_rating = excluded.user_rating,
                personal_note = excluded.personal_note,
                last_checked_date = excluded.last_checked_date,
                deleted_at = excluded.deleted_at",
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
        self.soft_delete_orphan_series()
    }

    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM fanfiction WHERE deleted_at IS NULL ORDER BY title")?;
        let rows = stmt.query_map([], row_to_fanfiction)?;
        let mut fics = rows.collect::<Result<Vec<_>, _>>()?;
        let mut refs = self.series_refs(None)?;
        for fic in &mut fics {
            fic.series = refs.remove(&fic.id).unwrap_or_default();
        }
        Ok(fics)
    }

    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM fanfiction WHERE id = ?1 AND deleted_at IS NULL")?;
        let mut fic = stmt
            .query_row(params![fic_id], row_to_fanfiction)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => FicflowError::NotFound { fic_id },
                other => FicflowError::Database(other),
            })?;
        fic.series = self
            .series_refs(Some(fic_id))?
            .remove(&fic_id)
            .unwrap_or_default();
        Ok(fic)
    }

    fn wipe_database(&self) -> Result<(), FicflowError> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE fanfiction SET deleted_at = ?1 WHERE deleted_at IS NULL",
            params![now],
        )?;
        self.soft_delete_orphan_series()
    }
}

impl SqliteRepository<'_> {
    fn soft_delete_orphan_series(&self) -> Result<(), FicflowError> {
        self.conn.execute(
            "UPDATE series SET deleted_at = ?1 WHERE deleted_at IS NULL AND NOT EXISTS ( \
                 SELECT 1 FROM series_work sw JOIN fanfiction f ON f.id = sw.work_id \
                 WHERE sw.series_id = series.id AND f.deleted_at IS NULL)",
            params![Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn series_refs(
        &self,
        fic_id: Option<u64>,
    ) -> Result<HashMap<u64, Vec<SeriesRef>>, FicflowError> {
        let mut stmt = self.conn.prepare(
            "SELECT sw.work_id, s.id, s.title, sw.part FROM series_work sw \
             JOIN series s ON s.id = sw.series_id \
             WHERE s.deleted_at IS NULL AND (?1 IS NULL OR sw.work_id = ?1) \
             ORDER BY s.title, s.id",
        )?;
        let rows = stmt.query_map(params![fic_id], |row| {
            Ok((
                row.get::<_, u64>(0)?,
                SeriesRef {
                    id: row.get(1)?,
                    title: row.get(2)?,
                    part: row.get(3)?,
                },
            ))
        })?;
        let mut refs: HashMap<u64, Vec<SeriesRef>> = HashMap::new();
        for row in rows {
            let (work_id, series) = row?;
            refs.entry(work_id).or_default().push(series);
        }
        Ok(refs)
    }

    fn series_works(&self, series_id: Option<u64>) -> Result<Vec<(u64, SeriesWork)>, FicflowError> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM series_work WHERE ?1 IS NULL OR series_id = ?1 ORDER BY series_id, part",
        )?;
        let rows = stmt.query_map(params![series_id], row_to_series_work)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

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

    fn reject_auto(&self, shelf_id: u64, message: &str) -> Result<(), FicflowError> {
        match self.get_shelf_by_id(shelf_id)?.kind {
            ShelfKind::Auto(_) => Err(FicflowError::InvalidInput(message.into())),
            ShelfKind::Normal => Ok(()),
        }
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
        let mut stmt = self
            .conn
            .prepare(&format!("{SUBTREE_CTE} SELECT id, depth FROM subtree"))?;
        let rows = stmt.query_map(params![shelf_id], |r| {
            Ok((r.get::<_, u64>(0)?, r.get::<_, u32>(1)?))
        })?;
        rows.collect::<Result<_, _>>()
            .map_err(FicflowError::Database)
    }

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
            self.reject_auto(parent, "cannot nest a shelf under an auto-shelf")?;
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

impl ShelfOps for SqliteRepository<'_> {
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
                if matches!(self.get_shelf_by_id(id)?.kind, ShelfKind::Normal) {
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
            self.reject_auto(parent, "cannot nest a shelf under an auto-shelf")?;
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
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SHELF_COLUMNS} FROM shelf s \
             WHERE s.deleted_at IS NULL \
             ORDER BY s.pinned DESC, s.name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map([], row_to_shelf)?;
        let shelves = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(shelves)
    }

    fn get_shelf_by_id(&self, shelf_id: u64) -> Result<Shelf, FicflowError> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SHELF_COLUMNS} FROM shelf s WHERE s.id = ?1 AND s.deleted_at IS NULL"
        ))?;
        stmt.query_row(params![shelf_id], row_to_shelf)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => FicflowError::ShelfNotFound { shelf_id },
                other => FicflowError::Database(other),
            })
    }

    fn add_fic_to_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError> {
        self.ensure_fanfiction_exists(fic_id)?;
        self.reject_auto(shelf_id, "cannot add fics to an auto-shelf")?;

        let added_at = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR IGNORE INTO fic_shelf (fic_id, shelf_id, added_at) VALUES (?1, ?2, ?3)",
            params![fic_id, shelf_id, added_at],
        )?;
        Ok(())
    }

    fn remove_fic_from_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError> {
        self.ensure_fanfiction_exists(fic_id)?;
        self.reject_auto(shelf_id, "cannot remove fics from an auto-shelf")?;

        self.conn.execute(
            "DELETE FROM fic_shelf WHERE fic_id = ?1 AND shelf_id = ?2",
            params![fic_id, shelf_id],
        )?;
        Ok(())
    }

    fn list_fics_in_shelf(&self, shelf_id: u64) -> Result<Vec<Fanfiction>, FicflowError> {
        self.ensure_shelf_exists(shelf_id)?;

        let mut stmt = self.conn.prepare(&format!(
            "{SUBTREE_CTE} \
             SELECT DISTINCT f.* FROM fanfiction f \
             JOIN fic_shelf fs ON fs.fic_id = f.id \
             JOIN subtree ON subtree.id = fs.shelf_id \
             WHERE f.deleted_at IS NULL \
             ORDER BY f.title"
        ))?;
        let rows = stmt.query_map(params![shelf_id], row_to_fanfiction)?;
        let fics = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(fics)
    }

    fn list_shelves_for_fic(&self, fic_id: u64) -> Result<Vec<Shelf>, FicflowError> {
        self.ensure_fanfiction_exists(fic_id)?;

        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SHELF_COLUMNS} FROM shelf s \
             JOIN fic_shelf fs ON fs.shelf_id = s.id \
             WHERE fs.fic_id = ?1 AND s.deleted_at IS NULL \
             ORDER BY s.name COLLATE NOCASE"
        ))?;
        let rows = stmt.query_map(params![fic_id], row_to_shelf)?;
        let shelves = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(shelves)
    }

    fn count_fics_in_shelf(&self, shelf_id: u64) -> Result<usize, FicflowError> {
        self.ensure_shelf_exists(shelf_id)?;

        let count: i64 = self.conn.query_row(
            &format!(
                "{SUBTREE_CTE} \
                 SELECT COUNT(DISTINCT fs.fic_id) FROM fic_shelf fs \
                 JOIN fanfiction f ON f.id = fs.fic_id \
                 JOIN subtree ON subtree.id = fs.shelf_id \
                 WHERE f.deleted_at IS NULL"
            ),
            params![shelf_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    fn count_fics_per_shelf(&self) -> Result<HashMap<u64, usize>, FicflowError> {
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

impl InboxOps for SqliteRepository<'_> {
    fn post_message(&self, content: &MessageContent) -> Result<InboxMessage, FicflowError> {
        let json = serde_json::to_string(content)?;
        let created_at = Utc::now();
        self.conn.execute(
            "INSERT INTO inbox_message (content, created_at) VALUES (?1, ?2)",
            params![json, created_at.to_rfc3339()],
        )?;
        Ok(InboxMessage {
            id: self.conn.last_insert_rowid() as u64,
            created_at,
            read_at: None,
            content: content.clone(),
        })
    }

    fn list_messages(&self) -> Result<Vec<InboxMessage>, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM inbox_message ORDER BY created_at DESC, id DESC")?;
        let rows = stmt.query_map([], row_to_inbox_message)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    fn delete_message(&self, message_id: u64) -> Result<(), FicflowError> {
        self.conn.execute(
            "DELETE FROM inbox_message WHERE id = ?1",
            params![message_id],
        )?;
        Ok(())
    }

    fn clear_inbox(&self) -> Result<(), FicflowError> {
        self.conn.execute("DELETE FROM inbox_message", [])?;
        Ok(())
    }

    fn mark_message_read(&self, message_id: u64) -> Result<(), FicflowError> {
        self.conn.execute(
            "UPDATE inbox_message SET read_at = ?2 WHERE id = ?1 AND read_at IS NULL",
            params![message_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn mark_all_read(&self) -> Result<(), FicflowError> {
        self.conn.execute(
            "UPDATE inbox_message SET read_at = ?1 WHERE read_at IS NULL",
            params![Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }
}

impl SeriesOps for SqliteRepository<'_> {
    fn save_series(&self, series: &Series) -> Result<(), FicflowError> {
        self.conn.execute(
            "INSERT INTO series (id, title, authors, description, notes, date_begun, date_updated, \
                 words, works_count, complete, bookmarks, last_checked_date, deleted_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL) \
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, authors = excluded.authors, \
                 description = excluded.description, notes = excluded.notes, \
                 date_begun = excluded.date_begun, date_updated = excluded.date_updated, \
                 words = excluded.words, works_count = excluded.works_count, \
                 complete = excluded.complete, bookmarks = excluded.bookmarks, \
                 last_checked_date = excluded.last_checked_date, deleted_at = NULL",
            params![
                series.id,
                series.title,
                serde_json::to_string(&series.authors)?,
                series.description,
                series.notes,
                series.date_begun.map(|d| d.to_rfc3339()),
                series.date_updated.map(|d| d.to_rfc3339()),
                series.words,
                series.works_count,
                series.complete,
                series.bookmarks,
                series.last_checked_date.to_rfc3339(),
            ],
        )?;
        self.conn.execute(
            "DELETE FROM series_work WHERE series_id = ?1 \
             AND work_id NOT IN (SELECT id FROM fanfiction WHERE deleted_at IS NULL)",
            params![series.id],
        )?;
        for work in &series.works {
            self.conn.execute(
                "INSERT INTO series_work (series_id, work_id, part, title, authors) \
                 VALUES (?1, ?2, ?3, ?4, ?5) \
                 ON CONFLICT(series_id, work_id) DO UPDATE SET part = excluded.part, \
                     title = excluded.title, authors = excluded.authors",
                params![
                    series.id,
                    work.work_id,
                    work.part,
                    work.title,
                    serde_json::to_string(&work.authors)?,
                ],
            )?;
        }
        Ok(())
    }

    fn link_fic_to_series(&self, fic: &Fanfiction) -> Result<(), FicflowError> {
        let ids: Vec<String> = fic.series.iter().map(|s| s.id.to_string()).collect();
        self.conn.execute(
            &format!(
                "DELETE FROM series_work WHERE work_id = ?1 AND series_id NOT IN ({})",
                ids.join(",")
            ),
            params![fic.id],
        )?;
        let authors = serde_json::to_string(&fic.authors)?;
        for series in &fic.series {
            self.conn.execute(
                "INSERT INTO series_work (series_id, work_id, part, title, authors) \
                 SELECT ?1, ?2, ?3, ?4, ?5 WHERE EXISTS (SELECT 1 FROM series WHERE id = ?1) \
                 ON CONFLICT(series_id, work_id) DO NOTHING",
                params![series.id, fic.id, series.part, fic.title, authors],
            )?;
        }
        self.soft_delete_orphan_series()
    }

    fn get_series(&self, series_id: u64) -> Result<Series, FicflowError> {
        let mut series = self
            .conn
            .query_row(
                "SELECT * FROM series WHERE id = ?1 AND deleted_at IS NULL",
                params![series_id],
                row_to_series,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => FicflowError::SeriesNotFound { series_id },
                other => FicflowError::Database(other),
            })?;
        series.works = self
            .series_works(Some(series_id))?
            .into_iter()
            .map(|(_, work)| work)
            .collect();
        Ok(series)
    }

    fn list_series(&self) -> Result<Vec<Series>, FicflowError> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM series WHERE deleted_at IS NULL ORDER BY title")?;
        let mut series = stmt
            .query_map([], row_to_series)?
            .collect::<Result<Vec<_>, _>>()?;
        let mut works: HashMap<u64, Vec<SeriesWork>> = HashMap::new();
        for (series_id, work) in self.series_works(None)? {
            works.entry(series_id).or_default().push(work);
        }
        for s in &mut series {
            s.works = works.remove(&s.id).unwrap_or_default();
        }
        Ok(series)
    }
}
