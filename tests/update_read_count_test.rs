use std::error::Error;
use rusqlite::Connection;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::fixtures;

use ficflow::{
    application::update_read_count::update_read_count,
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
    
    fn create_test_fanfiction(id: u64, read_count: u32) -> Fanfiction {
        let mut fic = fixtures::given_sample_fanfiction(id, format!("Test Fanfiction {}", id).as_str());
        fic.read_count = read_count;
        fic
    }
    
    #[test]
    fn test_update_read_count_from_zero() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 3001;
        let fic = create_test_fanfiction(fic_id, 0);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_read_count(&db_ops, fic_id, 5)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.read_count, 5);
        // Reading status should remain unchanged
        assert_eq!(updated_fic.reading_status, fic.reading_status);
        
        Ok(())
    }
    
    #[test]
    fn test_update_read_count_decrease() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 3002;
        let fic = create_test_fanfiction(fic_id, 10);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_read_count(&db_ops, fic_id, 3)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.read_count, 3);
        // Other fields should be unchanged
        assert_eq!(updated_fic.reading_status, fic.reading_status);
        assert_eq!(updated_fic.last_chapter_read, fic.last_chapter_read);
        
        Ok(())
    }
    
    #[test]
    fn test_update_read_count_to_zero() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 3003;
        let fic = create_test_fanfiction(fic_id, 7);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_read_count(&db_ops, fic_id, 0)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.read_count, 0);
        
        Ok(())
    }
    
    #[test]
    fn test_update_read_count_to_zero_changes_status() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 3004;
        let mut fic = create_test_fanfiction(fic_id, 5);
        fic.reading_status = ReadingStatus::Read; // Explicitly set to Read status
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_read_count(&db_ops, fic_id, 0)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.read_count, 0);
        assert_eq!(updated_fic.reading_status, ReadingStatus::PlanToRead); // Status should be changed
        
        Ok(())
    }
}
