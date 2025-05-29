use std::error::Error;
use rusqlite::Connection;
use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::fixtures;

use ficflow::{
    application::update_rating::update_user_rating,
    domain::fanfiction::{Fanfiction, UserRating},
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
    
    fn create_test_fanfiction(id: u64, user_rating: Option<UserRating>) -> Fanfiction {
        let mut fic = fixtures::given_sample_fanfiction(id, format!("Test Fanfiction {}", id).as_str());
        fic.user_rating = user_rating;
        fic
    }
    
    #[test]
    fn test_update_rating_from_none_to_rating() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 4001;
        let fic = create_test_fanfiction(fic_id, None);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_user_rating(&db_ops, fic_id, "4")?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.user_rating, Some(UserRating::Four));
        
        Ok(())
    }
    
    #[test]
    fn test_update_rating_with_words() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 4002;
        let fic = create_test_fanfiction(fic_id, None);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_user_rating(&db_ops, fic_id, "five")?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.user_rating, Some(UserRating::Five));
        
        Ok(())
    }
    
    #[test]
    fn test_update_rating_change_existing() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 4003;
        let fic = create_test_fanfiction(fic_id, Some(UserRating::Two));
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_user_rating(&db_ops, fic_id, "three")?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.user_rating, Some(UserRating::Three));
        
        Ok(())
    }
    
    #[test]
    fn test_update_rating_clear_rating() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 4004;
        let fic = create_test_fanfiction(fic_id, Some(UserRating::Five));
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        update_user_rating(&db_ops, fic_id, "none")?;
        
        // Then
        let updated_fic = get_fanfiction_by_id(&conn, fic_id)?;
        
        assert_eq!(updated_fic.user_rating, None);
        
        Ok(())
    }
    
    #[test]
    fn test_update_rating_invalid_rating() -> Result<(), Box<dyn Error>> {
        // Given
        let (conn, _temp_dir) = setup_test_db();
        let fic_id = 4005;
        let fic = create_test_fanfiction(fic_id, None);
        
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        let db_ops = MockDatabase::new(&conn);
        
        // When
        let result = update_user_rating(&db_ops, fic_id, "ten");
        
        // Then
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid rating"));
        
        // Verify rating was not changed
        let unchanged_fic = get_fanfiction_by_id(&conn, fic_id)?;
        assert_eq!(unchanged_fic.user_rating, None);
        
        Ok(())
    }
}
