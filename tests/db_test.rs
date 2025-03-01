use rusqlite::{Connection, Result};
use ficflow::infrastructure::migration::run_migrations;
use chrono::NaiveDateTime;
use ficflow::domain::fic::{Fanfiction, Rating, ReadingStatus, UserRating};

#[cfg(test)]
mod tests {
    use ficflow::infrastructure::db::{delete_fanfiction, insert_fanfiction};

    use super::*;

    fn setup_test_db() -> Result<Connection, Box<dyn std::error::Error>> {
        let mut conn = Connection::open_in_memory()?;
        // Run migrations manually for the in-memory DB
        run_migrations(&mut conn)?;
        Ok(conn)
    }

    #[test]
    fn test_add_fanfiction() -> Result<(), Box<dyn std::error::Error>> {
        // Given
        let conn = setup_test_db().expect("Failed to establish database connection");
        
        let new_fic = Fanfiction {
            id: 1,
            title: "Test Fanfiction".to_string(),
            authors: vec!["Author 1".to_string()],
            categories: None,
            chapters_total: None,
            chapters_published: 1,
            characters: None,
            complete: true,
            fandoms: vec!["Fandom 1".to_string()],
            hits: 100,
            kudos: 50,
            language: "English".to_string(),
            rating: Rating::General,
            relationships: None,
            restricted: false,
            summary: "A test fanfiction.".to_string(),
            tags: None,
            warnings: vec![],
            words: 5000,
            date_published: NaiveDateTime::parse_from_str("2025-01-01T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
            date_updated: NaiveDateTime::parse_from_str("2025-02-01T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
            last_chapter_read: None,
            reading_status: ReadingStatus::InProgress,
            read_count: 1,
            user_rating: Some(UserRating::Five),
            personal_note: Some("Great story!".to_string()),
            last_checked_date: NaiveDateTime::parse_from_str("2025-02-01T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
        };

        // When
        let result = insert_fanfiction(&conn, &new_fic);
        assert!(result.is_ok());

        // Then
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM fanfiction WHERE id = ?")?;
        let mut rows = stmt.query([new_fic.id])?;
        let count: i64 = rows.next()?.unwrap().get(0)?;

        assert_eq!(count, 1);
        Ok(())
    }

    #[test]
    fn test_remove_fanfiction() -> Result<(), Box<dyn std::error::Error>> {
        // Given
        let conn = setup_test_db().expect("Failed to establish database connection");

        let new_fic = Fanfiction {
            id: 2,
            title: "Test Fanfiction to Remove".to_string(),
            authors: vec!["Author 1".to_string()],
            categories: None,
            chapters_total: None,
            chapters_published: 1,
            characters: None,
            complete: true,
            fandoms: vec!["Fandom 1".to_string()],
            hits: 100,
            kudos: 50,
            language: "English".to_string(),
            rating: Rating::General,
            relationships: None,
            restricted: false,
            summary: "A test fanfiction.".to_string(),
            tags: None,
            warnings: vec![],
            words: 5000,
            date_published: NaiveDateTime::parse_from_str("2025-01-01T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
            date_updated: NaiveDateTime::parse_from_str("2025-02-01T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
            last_chapter_read: None,
            reading_status: ReadingStatus::InProgress,
            read_count: 1,
            user_rating: Some(UserRating::Five),
            personal_note: Some("Great story!".to_string()),
            last_checked_date: NaiveDateTime::parse_from_str("2025-02-01T12:00:00", "%Y-%m-%dT%H:%M:%S").unwrap(),
        };

        insert_fanfiction(&conn, &new_fic).expect("Failed to insert fanfiction");

        // When
        let result = delete_fanfiction(&conn, new_fic.id);
        assert!(result.is_ok());

        // Then
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM fanfiction WHERE id = ?")?;
        let mut rows = stmt.query([new_fic.id])?;
        let count: i64 = rows.next()?.unwrap().get(0)?;

        assert_eq!(count, 0);
        Ok(())
    }
}
