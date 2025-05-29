use std::error::Error;
use rusqlite::Connection;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::fixtures;

use ficflow::{
    application::update_status::update_reading_status,
    domain::fanfiction::{Fanfiction, ReadingStatus},
    infrastructure::persistence::repository::operations::{
        insert_fanfiction, get_fanfiction_by_id
    },
    infrastructure::persistence::database::migration::run_migrations
};

#[cfg(test)]
mod tests {
    use super::*;
    
    // Mock DatabaseOps implementation for testing
    struct MockDatabase<'a> {
        conn: &'a Connection,
    }
    
    impl<'a> MockDatabase<'a> {
        fn new(conn: &'a Connection) -> Self {
            Self { conn }
        }
    }
    
    impl<'a> ficflow::domain::fanfiction::DatabaseOps for MockDatabase<'a> {
        fn insert_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
            insert_fanfiction(self.conn, fic)
        }
        
        fn update_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
            ficflow::infrastructure::persistence::repository::operations::update_fanfiction(self.conn, fic)
        }
        
        fn delete_fanfiction(&self, fic_id: u64) -> Result<(), Box<dyn Error>> {
            ficflow::infrastructure::persistence::repository::operations::delete_fanfiction(self.conn, fic_id)
        }
        
        fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
            ficflow::infrastructure::persistence::repository::operations::get_all_fanfictions(self.conn)
        }
        
        fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
            get_fanfiction_by_id(self.conn, fic_id)
        }
        
        fn wipe_database(&self) -> Result<(), Box<dyn Error>> {
            ficflow::infrastructure::persistence::repository::operations::wipe_database(self.conn)
        }
    }
    
    fn setup_test_db() -> (Connection, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        
        let mut conn = Connection::open(&db_path).expect("Failed to open database");
        run_migrations(&mut conn).expect("Failed to run migrations");
        
        (conn, temp_dir)
    }
    
    fn create_test_fanfiction(id: u64, status: ReadingStatus) -> Fanfiction {
        let mut fic = fixtures::given_sample_fanfiction(id, format!("Test Fanfiction {}", id).as_str());
        fic.reading_status = status;
        fic
    }
    
    #[test]
    fn test_update_status_from_plan_to_read_to_in_progress() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 2001;
        let fic = create_test_fanfiction(fic_id, ReadingStatus::PlanToRead);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_reading_status(&db_ops, fic_id, "inprogress")?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.reading_status, ReadingStatus::InProgress);
        
        Ok(())
    }
    
    #[test]
    fn test_update_status_to_read() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 2002;
        let fic = create_test_fanfiction(fic_id, ReadingStatus::InProgress);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_reading_status(&db_ops, fic_id, "read")?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(updated_fic.reading_status, ReadingStatus::Read);
        
        Ok(())
    }
    
    #[test]
    fn test_update_status_with_different_formats() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 2003;
        let fic = create_test_fanfiction(fic_id, ReadingStatus::InProgress);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When - Test with different formats of the same status
        update_reading_status(&db_ops, fic_id, "plan-to-read")?;
        let fic1 = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(fic1.reading_status, ReadingStatus::PlanToRead);
        
        update_reading_status(&db_ops, fic_id, "plantoread")?;
        let fic2 = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(fic2.reading_status, ReadingStatus::PlanToRead);
        
        update_reading_status(&db_ops, fic_id, "plan")?;
        let fic3 = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(fic3.reading_status, ReadingStatus::PlanToRead);
        
        update_reading_status(&db_ops, fic_id, "in-progress")?;
        let fic4 = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(fic4.reading_status, ReadingStatus::InProgress);
        
        Ok(())
    }
    
    #[test]
    fn test_update_status_invalid_status() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 2004;
        let fic = create_test_fanfiction(fic_id, ReadingStatus::InProgress);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When - Test with invalid status
        let result = update_reading_status(&db_ops, fic_id, "invalid_status");
        
        // Then
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid reading status"));
        
        // Verify status was not changed
        let unchanged_fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(unchanged_fic.reading_status, ReadingStatus::InProgress);
        
        Ok(())
    }
}
