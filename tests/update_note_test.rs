use std::error::Error;
use rusqlite::Connection;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::fixtures;

use ficflow::{
    application::update_note::update_personal_note,
    domain::fanfiction::Fanfiction,
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
    
    fn create_test_fanfiction(id: u64) -> Fanfiction {
        fixtures::given_sample_fanfiction(id, format!("Test Fanfiction {}", id).as_str())
    }
    
    #[test]
    fn test_update_personal_note() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 4001;
        let fic = create_test_fanfiction(fic_id);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // Initial note should be None
        let initial_fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert!(initial_fic.personal_note.is_none());
        
        // When adding a note
        let note = "This is my favorite story!";
        update_personal_note(&db_ops, fic_id, Some(note))?;
        
        // Then note should be added
        let fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert!(fic.personal_note.is_some());
        assert_eq!(fic.personal_note.unwrap(), note);
        
        // When updating the note
        let updated_note = "Actually I changed my mind. It's good but not my favorite.";
        update_personal_note(&db_ops, fic_id, Some(updated_note))?;
        
        // Then note should be updated
        let fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert!(fic.personal_note.is_some());
        assert_eq!(fic.personal_note.unwrap(), updated_note);
        
        // When removing the note
        update_personal_note(&db_ops, fic_id, None)?;
        
        // Then note should be removed
        let fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert!(fic.personal_note.is_none());
        
        Ok(())
    }
    
    #[test]
    fn test_update_note_nonexistent_fic() {
        let (conn, _temp_dir) = setup_test_db();
        let db_ops = MockDatabase::new(&conn);
        
        let invalid_fic_id = 999999; // A non-existent fanfiction ID
        let result = update_personal_note(&db_ops, invalid_fic_id, Some("Note for non-existent fic"));
        
        assert!(result.is_err(), "Expected error when updating note for non-existent fanfiction");
    }
}
