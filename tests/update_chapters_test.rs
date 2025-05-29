use std::error::Error;
use rusqlite::Connection;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::fixtures;

use ficflow::{
    application::update_chapters::update_last_chapter_read,
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
    
    fn create_test_fanfiction(id: u64, status: ReadingStatus, chapters_total: Option<u32>, read_count: u32) -> Fanfiction {
        let mut fic = fixtures::given_sample_fanfiction(id, format!("Test Fanfiction {}", id).as_str());
        fic.reading_status = status;
        fic.chapters_total = chapters_total;
        fic.read_count = read_count;
        fic
    }
    
    #[test]
    fn test_update_to_in_progress_from_plan_to_read() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1001;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::PlanToRead, Some(10), 0);
        fic.last_chapter_read = None;
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_last_chapter_read(&db_ops, fic_id, 5)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(5));
        assert_eq!(updated_fic.reading_status, ReadingStatus::InProgress);
        assert_eq!(updated_fic.read_count, 0); // Should not change
        
        Ok(())
    }
    
    #[test]
    fn test_update_to_in_progress_from_paused() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1002;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::Paused, Some(10), 0);
        fic.last_chapter_read = Some(3);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_last_chapter_read(&db_ops, fic_id, 5)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(5));
        assert_eq!(updated_fic.reading_status, ReadingStatus::InProgress);
        assert_eq!(updated_fic.read_count, 0); // Should not change
        
        Ok(())
    }
    
    #[test]
    fn test_update_from_in_progress_stays_in_progress() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1003;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::InProgress, Some(10), 0);
        fic.last_chapter_read = Some(3);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_last_chapter_read(&db_ops, fic_id, 5)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(5));
        assert_eq!(updated_fic.reading_status, ReadingStatus::InProgress);
        assert_eq!(updated_fic.read_count, 0);
        
        Ok(())
    }
    
    #[test]
    fn test_update_to_final_chapter_from_in_progress() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1004;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::InProgress, Some(10), 0);
        fic.last_chapter_read = Some(8);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_last_chapter_read(&db_ops, fic_id, 10)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(10));
        assert_eq!(updated_fic.reading_status, ReadingStatus::Read);
        assert_eq!(updated_fic.read_count, 1); // Should increment
        
        Ok(())
    }
    
    #[test]
    fn test_update_with_already_read_status() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1005;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::Read, Some(10), 1);
        fic.last_chapter_read = Some(10);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        // Re-reading the final chapter should increment read count
        update_last_chapter_read(&db_ops, fic_id, 10)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(10));
        assert_eq!(updated_fic.reading_status, ReadingStatus::Read);
        assert_eq!(updated_fic.read_count, 2); // Should increment again
        
        Ok(())
    }
    
    #[test]
    fn test_prevent_exceeding_chapter_count() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1006;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::InProgress, Some(10), 0);
        fic.last_chapter_read = Some(5);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_last_chapter_read(&db_ops, fic_id, 15)?; // Try to exceed total
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(10)); // Should be adjusted to maximum
        assert_eq!(updated_fic.reading_status, ReadingStatus::Read);
        assert_eq!(updated_fic.read_count, 1); // Should increment
        
        Ok(())
    }
    
    #[test]
    fn test_unknown_total_chapters() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1007;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::PlanToRead, None, 0);
        fic.last_chapter_read = None;
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_last_chapter_read(&db_ops, fic_id, 5)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(5));
        assert_eq!(updated_fic.reading_status, ReadingStatus::InProgress);
        assert_eq!(updated_fic.read_count, 0); // Should not change
        
        // Can go to any chapter number since total is unknown
        update_last_chapter_read(&db_ops, fic_id, 100)?;
        
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(updated_fic.last_chapter_read, Some(100));
        assert_eq!(updated_fic.reading_status, ReadingStatus::InProgress); // Still in progress without known total
        
        Ok(())
    }
    
    #[test]
    fn test_abandoned_status_preserved() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 1008;
        let mut fic = create_test_fanfiction(fic_id, ReadingStatus::Abandoned, Some(10), 0);
        fic.last_chapter_read = Some(3);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_last_chapter_read(&db_ops, fic_id, 5)?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.last_chapter_read, Some(5));
        assert_eq!(updated_fic.reading_status, ReadingStatus::Abandoned); // Should stay Abandoned
        assert_eq!(updated_fic.read_count, 0);
        
        // Even if reaching final chapter
        update_last_chapter_read(&db_ops, fic_id, 10)?;
        
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(updated_fic.last_chapter_read, Some(10));
        assert_eq!(updated_fic.reading_status, ReadingStatus::Read); // Final chapter changes to Read
        assert_eq!(updated_fic.read_count, 1); // Should increment
        
        Ok(())
    }
}
