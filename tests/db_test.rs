use std::error::Error;
use rusqlite::{Connection, OpenFlags};
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::{fixtures, assertions};

#[cfg(test)]
mod tests {
    use ficflow::infrastructure::{
        db::{delete_fanfiction, get_all_fanfictions, get_fanfiction_by_id},
        migration::run_migrations
    };

    use super::*;

    fn setup_test_db() -> (Connection, TempDir) {
        // Given
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        
        // Explicitly set the OpenFlags to CREATE | READ_WRITE to ensure write permissions
        let mut conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)
            .expect("Failed to open database with write permissions");
        
        run_migrations(&mut conn).expect("Failed to run migrations");
        
        (conn, temp_dir)
    }

    #[test]
    fn test_add_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let new_fic = fixtures::given_sample_fanfiction(1, "Test Fanfiction");

        // When
        fixtures::when_fanfiction_added_to_db(&conn, &new_fic)?;

        // Then
        assertions::then_fanfiction_was_added(&conn, &new_fic)?;

        Ok(())
    }

    #[test]
    fn test_remove_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let new_fic = fixtures::given_sample_fanfiction(2, "Test Fanfiction to Remove");
        fixtures::when_fanfiction_added_to_db(&conn, &new_fic)?;

        // When
        delete_fanfiction(&conn, new_fic.id)?;

        // Then
        assertions::then_fanfiction_was_deleted(&conn, new_fic.id)?;
        
        Ok(())
    }

    #[test]
    fn test_get_all_fanfictions() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();

        let fic1 = fixtures::given_sample_fanfiction(101, "Fanfiction One");
        let fic2 = fixtures::given_sample_fanfiction(102, "Fanfiction Two");
        let fic3 = fixtures::given_sample_fanfiction(103, "Fanfiction Three");
        
        // When
        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic3)?;
        
        // Then
        let result = get_all_fanfictions(&conn)?;
        assert_eq!(result.len(), 3);
        
        // Verify each fanfiction was properly added
        assertions::then_fanfiction_was_added(&conn, &fic1)?;
        assertions::then_fanfiction_was_added(&conn, &fic2)?;
        assertions::then_fanfiction_was_added(&conn, &fic3)?;
        
        Ok(())
    }

    #[test]
    fn test_wipe_database() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        
        let fic1 = fixtures::given_sample_fanfiction(201, "Wipe Test Fanfiction One");
        let fic2 = fixtures::given_sample_fanfiction(202, "Wipe Test Fanfiction Two");
        let fic3 = fixtures::given_sample_fanfiction(203, "Wipe Test Fanfiction Three");

        fixtures::when_fanfiction_added_to_db(&conn, &fic1)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic2)?;
        fixtures::when_fanfiction_added_to_db(&conn, &fic3)?;

        let before_wipe = get_all_fanfictions(&conn)?;
        assert_eq!(before_wipe.len(), 3);
        
        // When
        let wipe_result = ficflow::infrastructure::db::wipe_database(&conn);
        assert!(wipe_result.is_ok());
        
        // Then
        assertions::then_database_was_wiped(&conn)?;
        
        Ok(())
    }

    #[test]
    fn test_get_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let test_fic = fixtures::given_sample_fanfiction(301, "Get Test Fanfiction");
        
        // When
        fixtures::when_fanfiction_added_to_db(&conn, &test_fic)?;
        
        // Then
        let result = get_fanfiction_by_id(&conn, 301)
            .expect("Failed to retrieve fanfiction by ID");
        
        assertions::then_fanfiction_was_fetched(&test_fic, &result, None);
        Ok(())
    }
}
