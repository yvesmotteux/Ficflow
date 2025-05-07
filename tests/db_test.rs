use rusqlite::{Connection, Result};
use std::error::Error;
use ficflow::infrastructure::migration::run_migrations;
use ficflow::domain::fic::{Fanfiction, Rating, ReadingStatus};

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};
    use ficflow::infrastructure::db::{delete_fanfiction, insert_fanfiction, get_all_fanfictions};
    use ficflow::domain::fic::assert_fanfiction_eq;

    use super::*;

    fn setup_test_db() -> Result<Connection, Box<dyn Error>> {
        let mut conn = Connection::open_in_memory()?;
        // Run migrations manually for the in-memory DB
        run_migrations(&mut conn)?;
        Ok(conn)
    }

    fn create_test_fanfiction(id: u64, title: &str) -> Fanfiction {
        Fanfiction {
            id,
            title: title.to_string(),
            authors: vec!["Author A".to_string()],
            categories: None,
            chapters_total: None,
            chapters_published: 1,
            characters: None,
            complete: true,
            fandoms: vec!["Fandom X".to_string()],
            hits: 100,
            kudos: 50,
            language: "English".to_string(),
            rating: Rating::General,
            relationships: None,
            restricted: false,
            summary: "A test fanfiction.".to_string(),
            tags: None,
            warnings: vec![],
            words: 1000,
            date_published: "2025-01-01T12:00:00Z".parse::<DateTime<Utc>>().unwrap(),
            date_updated: "2025-01-01T12:00:00Z".parse::<DateTime<Utc>>().unwrap(),
            last_chapter_read: None,
            reading_status: ReadingStatus::InProgress,
            read_count: 1,
            user_rating: None,
            personal_note: None,
            last_checked_date: "2025-01-01T12:00:00Z".parse::<DateTime<Utc>>().unwrap(),
        }
    }

    #[test]
    fn test_add_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let conn = setup_test_db().expect("Failed to establish database connection");
        
        let new_fic = create_test_fanfiction(1, "Test Fanfiction");

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
    fn test_remove_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let conn = setup_test_db().expect("Failed to establish database connection");

        let new_fic = create_test_fanfiction(2, "Test Fanfiction to Remove");

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

    #[test]
    fn test_get_all_fanfictions() -> Result<(), Box<dyn Error>> {
        // Given
        let conn = setup_test_db().expect("Failed to establish database connection");
        
        // Create three test fanfictions with different titles
        let fic1 = create_test_fanfiction(101, "Fanfiction One");
        let fic2 = create_test_fanfiction(102, "Fanfiction Two");
        let fic3 = create_test_fanfiction(103, "Fanfiction Three");
        
        // Insert the test fanfictions
        insert_fanfiction(&conn, &fic1)?;
        insert_fanfiction(&conn, &fic2)?;
        insert_fanfiction(&conn, &fic3)?;
        
        // When
        let result = get_all_fanfictions(&conn)?;
        
        // Then
        assert_eq!(result.len(), 3);
        
        // Use a HashMap to access fanfictions by ID
        let mut id_to_fanfiction = std::collections::HashMap::new();
        for fic in result {
            id_to_fanfiction.insert(fic.id, fic);
        }
        
        // Use assert_fanfiction_eq to compare each fanfiction with its expected version
        assert!(id_to_fanfiction.contains_key(&101));
        let fic1_result = id_to_fanfiction.get(&101).unwrap();
        assert_fanfiction_eq(&fic1, fic1_result);
        
        assert!(id_to_fanfiction.contains_key(&102));
        let fic2_result = id_to_fanfiction.get(&102).unwrap();
        assert_fanfiction_eq(&fic2, fic2_result);
        
        assert!(id_to_fanfiction.contains_key(&103));
        let fic3_result = id_to_fanfiction.get(&103).unwrap();
        assert_fanfiction_eq(&fic3, fic3_result);
        
        Ok(())
    }
}
