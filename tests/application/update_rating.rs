use rusqlite::Connection;
use std::error::Error;
use tempfile::TempDir;

use crate::common::fixtures;

use ficflow::{
    application::update_rating::update_user_rating,
    domain::fanfiction::{Fanfiction, FanfictionOps, UserRating},
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

    fn create_test_fanfiction(id: u64, user_rating: Option<UserRating>) -> Fanfiction {
        let mut fic =
            fixtures::given_sample_fanfiction(id, format!("Test Fanfiction {}", id).as_str());
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
        let fanfiction_ops = SqliteRepository::new(&conn);

        // When
        update_user_rating(&fanfiction_ops, fic_id, "4")?;

        // Then
        let updated_fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;

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
        let fanfiction_ops = SqliteRepository::new(&conn);

        // When
        update_user_rating(&fanfiction_ops, fic_id, "five")?;

        // Then
        let updated_fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;

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
        let fanfiction_ops = SqliteRepository::new(&conn);

        // When
        update_user_rating(&fanfiction_ops, fic_id, "three")?;

        // Then
        let updated_fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;

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
        let fanfiction_ops = SqliteRepository::new(&conn);

        // When
        update_user_rating(&fanfiction_ops, fic_id, "none")?;

        // Then
        let updated_fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;

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
        let fanfiction_ops = SqliteRepository::new(&conn);

        // When
        let result = update_user_rating(&fanfiction_ops, fic_id, "ten");

        // Then
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid rating"));

        // Verify rating was not changed
        let unchanged_fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
        assert_eq!(unchanged_fic.user_rating, None);

        Ok(())
    }
}
