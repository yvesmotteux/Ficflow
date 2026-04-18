use rusqlite::Connection;
use std::cell::Cell;
use std::error::Error;
use tempfile::TempDir;

use crate::common::fixtures;

use ficflow::{
    application::add_fic::add_fanfiction,
    domain::fanfiction::{Fanfiction, FanfictionFetcher, FanfictionOps, ReadingStatus, UserRating},
    error::FicflowError,
    infrastructure::persistence::database::migration::run_migrations,
    infrastructure::persistence::repository::SqliteRepository,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> (Connection, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");

        let mut conn = Connection::open(&db_path).expect("Failed to open database");
        run_migrations(&mut conn).expect("Failed to run migrations");

        (conn, temp_dir)
    }

    struct StubFetcher {
        fic_to_return: Fanfiction,
        call_count: Cell<u32>,
    }

    impl FanfictionFetcher for StubFetcher {
        fn fetch_fanfiction(
            &self,
            _fic_id: u64,
            _base_url: &str,
        ) -> Result<Fanfiction, FicflowError> {
            self.call_count.set(self.call_count.get() + 1);
            Ok(clone_fanfiction(&self.fic_to_return))
        }
    }

    fn clone_fanfiction(fic: &Fanfiction) -> Fanfiction {
        let mut cloned = fixtures::given_sample_fanfiction(fic.id, &fic.title);
        cloned.authors = fic.authors.clone();
        cloned.chapters_published = fic.chapters_published;
        cloned.chapters_total = fic.chapters_total;
        cloned.hits = fic.hits;
        cloned.kudos = fic.kudos;
        cloned.summary = fic.summary.clone();
        cloned.words = fic.words;
        cloned
    }

    #[test]
    fn test_add_preserves_user_fields_when_fic_already_exists() -> Result<(), Box<dyn Error>> {
        // Given: a fic in the DB with customized user fields.
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 7001;

        let mut existing = fixtures::given_sample_fanfiction(fic_id, "Original Title");
        existing.reading_status = ReadingStatus::InProgress;
        existing.user_rating = Some(UserRating::Five);
        existing.read_count = 3;
        existing.personal_note = Some("favorite scene in chapter 4".to_string());
        existing.last_chapter_read = Some(2);

        fixtures::when_fanfiction_added_to_db(&conn, &existing)?;

        // And: a fetcher that would return a fresh fic with default user fields if called.
        let fresh_from_ao3 = fixtures::given_sample_fanfiction(fic_id, "Original Title");
        let fetcher = StubFetcher {
            fic_to_return: fresh_from_ao3,
            call_count: Cell::new(0),
        };

        let fanfiction_ops = SqliteRepository::new(&conn);

        // When: add is invoked again.
        let result = add_fanfiction(&fetcher, &fanfiction_ops, fic_id, "http://unused");

        // Then: we get AlreadyExists and the fetcher was never called.
        match result {
            Err(FicflowError::AlreadyExists {
                fic_id: returned_id,
            }) => {
                assert_eq!(returned_id, fic_id);
            }
            other => panic!("expected AlreadyExists, got {:?}", other),
        }
        assert_eq!(
            fetcher.call_count.get(),
            0,
            "fetcher should not be called when the fic already exists"
        );

        // And: user-customized fields are untouched.
        let stored = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
        assert_eq!(stored.reading_status, ReadingStatus::InProgress);
        assert_eq!(stored.user_rating, Some(UserRating::Five));
        assert_eq!(stored.read_count, 3);
        assert_eq!(
            stored.personal_note,
            Some("favorite scene in chapter 4".to_string())
        );
        assert_eq!(stored.last_chapter_read, Some(2));

        Ok(())
    }

    #[test]
    fn test_add_persists_new_fic_on_happy_path() -> Result<(), Box<dyn Error>> {
        // Given: an empty DB and a fetcher that returns a fresh fic.
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 7002;

        let fresh = fixtures::given_sample_fanfiction(fic_id, "A New Fic");
        let fetcher = StubFetcher {
            fic_to_return: fresh,
            call_count: Cell::new(0),
        };

        let fanfiction_ops = SqliteRepository::new(&conn);

        // When: add is invoked.
        let title = add_fanfiction(&fetcher, &fanfiction_ops, fic_id, "http://unused")?;

        // Then: the fetcher was called and the fic is persisted.
        assert_eq!(title, "A New Fic");
        assert_eq!(fetcher.call_count.get(), 1);

        let stored = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
        assert_eq!(stored.id, fic_id);
        assert_eq!(stored.title, "A New Fic");

        Ok(())
    }
}
