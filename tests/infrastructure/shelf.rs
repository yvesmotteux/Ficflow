use rusqlite::{Connection, OpenFlags};
use std::error::Error;
use tempfile::TempDir;

use crate::common::fixtures;

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::FanfictionOps;
    use ficflow::domain::shelf::ShelfOps;
    use ficflow::error::FicflowError;
    use ficflow::infrastructure::persistence::database::migration::run_migrations;
    use ficflow::infrastructure::persistence::repository::SqliteRepository;

    use super::*;

    fn setup_test_db() -> (Connection, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        let mut conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .expect("Failed to open database with write permissions");
        run_migrations(&mut conn).expect("Failed to run migrations");
        // FK enforcement matches production; without this PRAGMA, cascades
        // and FK rejections we rely on (e.g. fic_shelf row deletion when a
        // fic is hard-removed) wouldn't fire.
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .expect("Failed to enable foreign keys");
        (conn, temp_dir)
    }

    #[test]
    fn test_create_shelf_persists_with_trimmed_name() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("  Favorites  ")?;

        assert_eq!(
            shelf.name, "Favorites",
            "leading/trailing whitespace trimmed"
        );
        let listed = repo.list_shelves()?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, shelf.id);
        Ok(())
    }

    #[test]
    fn test_create_shelf_rejects_empty_name() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.create_shelf("").unwrap_err();
        assert!(
            matches!(err, FicflowError::InvalidInput(_)),
            "expected InvalidInput, got {:?}",
            err
        );
    }

    #[test]
    fn test_create_shelf_rejects_whitespace_only_name() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.create_shelf("   \t  ").unwrap_err();
        assert!(matches!(err, FicflowError::InvalidInput(_)));
    }

    #[test]
    fn test_get_shelf_by_id_returns_shelf() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let created = repo.create_shelf("Long Reads")?;
        let fetched = repo.get_shelf_by_id(created.id)?;

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.name, "Long Reads");
        Ok(())
    }

    #[test]
    fn test_get_shelf_by_id_returns_not_found_for_missing() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.get_shelf_by_id(9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
    }

    #[test]
    fn test_list_shelves_sorts_by_name_case_insensitive() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        repo.create_shelf("zebra")?;
        repo.create_shelf("Apple")?;
        repo.create_shelf("Mango")?;

        let listed: Vec<String> = repo.list_shelves()?.into_iter().map(|s| s.name).collect();
        assert_eq!(listed, vec!["Apple", "Mango", "zebra"]);
        Ok(())
    }

    #[test]
    fn test_delete_shelf_excludes_from_list_but_keeps_fics() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(1, "Forever Belongs Here");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Doomed")?;
        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        repo.delete_shelf(shelf.id)?;

        // Shelf no longer appears.
        assert!(repo.list_shelves()?.is_empty());
        // get_shelf_by_id treats soft-deleted shelves as not found.
        assert!(matches!(
            repo.get_shelf_by_id(shelf.id).unwrap_err(),
            FicflowError::ShelfNotFound { .. }
        ));
        // The fic itself is still there.
        assert!(repo.list_fanfictions()?.iter().any(|f| f.id == fic.id));
        Ok(())
    }

    #[test]
    fn test_delete_shelf_returns_not_found_when_already_gone() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.delete_shelf(404).unwrap_err();
        assert!(matches!(err, FicflowError::ShelfNotFound { shelf_id: 404 }));
    }

    #[test]
    fn test_add_fic_to_shelf_creates_link() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(11, "Test Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Reading")?;

        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        let in_shelf = repo.list_fics_in_shelf(shelf.id)?;
        assert_eq!(in_shelf.len(), 1);
        assert_eq!(in_shelf[0].id, fic.id);
        Ok(())
    }

    #[test]
    fn test_add_fic_to_shelf_is_idempotent() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(12, "Twice-Added Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Reading")?;

        repo.add_fic_to_shelf(fic.id, shelf.id)?;
        // Second add must not error and must not duplicate.
        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 1);
        Ok(())
    }

    #[test]
    fn test_add_fic_to_shelf_rejects_missing_fic() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Reading")?;
        let err = repo.add_fic_to_shelf(9999, shelf.id).unwrap_err();
        assert!(matches!(err, FicflowError::NotFound { fic_id: 9999 }));
        Ok(())
    }

    #[test]
    fn test_add_fic_to_shelf_rejects_missing_shelf() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(13, "Lonely Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;

        let err = repo.add_fic_to_shelf(fic.id, 9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
        Ok(())
    }

    #[test]
    fn test_remove_fic_from_shelf() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(14, "Removed Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let shelf = repo.create_shelf("Reading")?;
        repo.add_fic_to_shelf(fic.id, shelf.id)?;

        repo.remove_fic_from_shelf(fic.id, shelf.id)?;

        assert!(repo.list_fics_in_shelf(shelf.id)?.is_empty());
        Ok(())
    }

    #[test]
    fn test_list_fics_in_shelf_sorted_by_title_excludes_deleted_fics() -> Result<(), Box<dyn Error>>
    {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic1 = fixtures::given_sample_fanfiction(21, "Banana Saga");
        let fic2 = fixtures::given_sample_fanfiction(22, "Apple Tale");
        let fic3 = fixtures::given_sample_fanfiction(23, "Cherry Story");
        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic3)?;
        let shelf = repo.create_shelf("Reading")?;
        repo.add_fic_to_shelf(fic1.id, shelf.id)?;
        repo.add_fic_to_shelf(fic2.id, shelf.id)?;
        repo.add_fic_to_shelf(fic3.id, shelf.id)?;

        // Soft-delete the middle one — it must drop out of the listing.
        repo.delete_fanfiction(fic3.id)?;

        let titles: Vec<String> = repo
            .list_fics_in_shelf(shelf.id)?
            .into_iter()
            .map(|f| f.title)
            .collect();
        assert_eq!(titles, vec!["Apple Tale", "Banana Saga"]);
        Ok(())
    }

    #[test]
    fn test_list_fics_in_shelf_rejects_missing_shelf() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.list_fics_in_shelf(9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
    }

    #[test]
    fn test_list_shelves_for_fic_sorted_excludes_deleted_shelves() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let fic = fixtures::given_sample_fanfiction(31, "Shared Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let s_zebra = repo.create_shelf("Zebra")?;
        let s_apple = repo.create_shelf("Apple")?;
        let s_mango = repo.create_shelf("Mango")?;
        repo.add_fic_to_shelf(fic.id, s_zebra.id)?;
        repo.add_fic_to_shelf(fic.id, s_apple.id)?;
        repo.add_fic_to_shelf(fic.id, s_mango.id)?;

        // Soft-delete one shelf — it must drop out.
        repo.delete_shelf(s_mango.id)?;

        let names: Vec<String> = repo
            .list_shelves_for_fic(fic.id)?
            .into_iter()
            .map(|s| s.name)
            .collect();
        assert_eq!(names, vec!["Apple", "Zebra"]);
        Ok(())
    }

    #[test]
    fn test_list_shelves_for_fic_rejects_missing_fic() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.list_shelves_for_fic(9999).unwrap_err();
        assert!(matches!(err, FicflowError::NotFound { fic_id: 9999 }));
    }

    #[test]
    fn test_count_fics_in_shelf_returns_active_link_count() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Reading")?;
        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 0, "empty shelf is 0");

        let fic1 = fixtures::given_sample_fanfiction(41, "First");
        let fic2 = fixtures::given_sample_fanfiction(42, "Second");
        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        repo.add_fic_to_shelf(fic1.id, shelf.id)?;
        repo.add_fic_to_shelf(fic2.id, shelf.id)?;

        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 2);
        Ok(())
    }

    #[test]
    fn test_count_fics_in_shelf_excludes_soft_deleted_fics() -> Result<(), Box<dyn Error>> {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let shelf = repo.create_shelf("Reading")?;
        let fic1 = fixtures::given_sample_fanfiction(51, "Stays");
        let fic2 = fixtures::given_sample_fanfiction(52, "Goes");
        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        repo.add_fic_to_shelf(fic1.id, shelf.id)?;
        repo.add_fic_to_shelf(fic2.id, shelf.id)?;
        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 2);

        // Soft-delete one of the fics; the COUNT query must skip it
        // because it joins on `fanfiction.deleted_at IS NULL`.
        repo.delete_fanfiction(fic2.id)?;

        assert_eq!(repo.count_fics_in_shelf(shelf.id)?, 1);
        Ok(())
    }

    #[test]
    fn test_count_fics_in_shelf_rejects_missing_shelf() {
        let (conn, _td) = setup_test_db();
        let repo = SqliteRepository::new(&conn);

        let err = repo.count_fics_in_shelf(9999).unwrap_err();
        assert!(matches!(
            err,
            FicflowError::ShelfNotFound { shelf_id: 9999 }
        ));
    }
}
