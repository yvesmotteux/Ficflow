#![allow(dead_code)]  // Suppress warnings about unused functions

use std::error::Error;
use std::path::PathBuf;
use httpmock::MockServer;
use rusqlite::{Connection, OpenFlags};
use tempfile::TempDir;
use chrono::Utc;

use ficflow::{
    domain::fanfiction::{Fanfiction, FanfictionFetcher},
    infrastructure::db::operations::{get_all_fanfictions, delete_fanfiction, insert_fanfiction},
};

pub mod assertions {
    use super::*;

    /// Asserts that a fanfiction was successfully fetched and matches the expected fanfiction.
    /// 
    /// # Arguments
    /// 
    /// * `expected` - The expected fanfiction
    /// * `actual` - The actual fetched fanfiction
    /// * `mock` - Optional mock server to verify that it was called
    pub fn then_fanfiction_was_fetched(expected: &Fanfiction, actual: &Fanfiction, mock: Option<&httpmock::Mock>) {
        {
            let expected = expected;
            let actual = actual;
            let mut errors = Vec::new();

            macro_rules! compare_field {
                ($field:ident) => {
                    if expected.$field != actual.$field {
                        errors.push(format!(
                            "Field `{}` differs:\n  Expected: {:?}\n  Actual:   {:?}",
                            stringify!($field), expected.$field, actual.$field
                        ));
                    }
                };
            }

            compare_field!(id);
            compare_field!(title);
            compare_field!(authors);
            compare_field!(categories);
            compare_field!(chapters_total);
            compare_field!(chapters_published);
            compare_field!(characters);
            compare_field!(complete);
            compare_field!(fandoms);
            compare_field!(hits);
            compare_field!(kudos);
            compare_field!(language);
            compare_field!(rating);
            compare_field!(relationships);
            compare_field!(restricted);
            compare_field!(summary);
            compare_field!(tags);
            compare_field!(warnings);
            compare_field!(words);
            compare_field!(date_published);
            compare_field!(date_updated);

            if !errors.is_empty() {
                panic!("Fanfiction structs are not equal:\n{}", errors.join("\n"));
            }

        };
        
        if let Some(mock_val) = mock {
            mock_val.assert();
        }
    }

    /// Asserts that a fanfiction was successfully added to the database.
    /// 
    /// # Arguments
    /// 
    /// * `conn` - Database connection
    /// * `expected_fic` - The fanfiction that was expected to be added
    pub fn then_fanfiction_was_added(conn: &Connection, expected_fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
        let fanfictions = get_all_fanfictions(conn)?;
        assert!(!fanfictions.is_empty(), "Expected fanfictions in database but none were found");
        
        let found = fanfictions.iter().any(|fic| {
            fic.id == expected_fic.id && fic.title == expected_fic.title
        });
        
        assert!(found, "Expected fanfiction with id={} and title=\"{}\" not found in database", 
               expected_fic.id, expected_fic.title);
        
        Ok(())
    }

    /// Asserts that a fanfiction was successfully deleted from the database.
    /// 
    /// # Arguments
    /// 
    /// * `conn` - Database connection
    /// * `fic_id` - The ID of the fanfiction that was expected to be deleted
    pub fn then_fanfiction_was_deleted(conn: &Connection, fic_id: u64) -> Result<(), Box<dyn Error>> {
        let fanfictions = get_all_fanfictions(conn)?;
        
        let found = fanfictions.iter().any(|fic| fic.id == fic_id);
        assert!(!found, "Fanfiction with id={} still exists in database after deletion", fic_id);
        
        Ok(())
    }

    /// Asserts that the database was successfully wiped.
    /// 
    /// # Arguments
    /// 
    /// * `conn` - Database connection
    pub fn then_database_was_wiped(conn: &Connection) -> Result<(), Box<dyn Error>> {
        let fanfictions = get_all_fanfictions(conn)?;
        assert_eq!(fanfictions.len(), 0, "Expected no fanfictions in database after wipe");
        
        Ok(())
    }

    /// Asserts that a CLI command execution was successful.
    /// 
    /// # Arguments
    /// 
    /// * `status` - The exit status of the command
    /// * `stderr` - The stderr output of the command
    /// * `expected_strings` - Optional strings that should be in the stdout
    /// * `stdout` - The stdout output of the command to check against expected_strings
    pub fn then_command_succeeded(status: i32, stderr: &str, expected_strings: Option<&[&str]>, stdout: Option<&str>) {
        assert_eq!(status, 0, "Command failed with stderr: {}", stderr);
        
        if let (Some(strings), Some(output)) = (expected_strings, stdout) {
            for expected in strings {
                assert!(output.contains(expected), 
                    "Expected to find '{}' in command output, got: {}", expected, output);
            }
        }
    }
}

pub mod fixtures {
    use super::*;
    use std::fs;
    use httpmock::Method::GET;
    use ficflow::{
        domain::fanfiction::{Rating, ReadingStatus, Categories, ArchiveWarnings},
        infrastructure::db::migration::run_migrations
    };
    
    /// Sets up a test database with migrations.
    pub fn given_test_database() -> (Connection, PathBuf, TempDir) {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        
        // Explicitly set the OpenFlags to CREATE | READ_WRITE to ensure write permissions
        let mut conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)
            .expect("Failed to open database with write permissions");
        
        run_migrations(&mut conn).expect("Failed to run migrations");
        
        (conn, db_path, temp_dir)
    }
    
    /// Sets up a mock AO3 server with a sample fanfiction.
    pub fn given_mock_ao3_server() -> (MockServer, u64) {
        let mock_server = MockServer::start();
        let fic_id = 53960491;
        
        let html_content = fs::read_to_string("tests/fixtures/ao3_fic_example1.html")
            .expect("Failed to read mock HTML file");
            
        mock_server.mock(|when, then| {
            when.method(GET).path(format!("/works/{}", fic_id));
            then.status(200).body(html_content);
        });
        
        (mock_server, fic_id)
    }
    
    /// Creates a sample fanfiction for testing.
    pub fn given_sample_fanfiction(id: u64, title: &str) -> Fanfiction {
        Fanfiction {
            id,
            title: title.to_string(),
            authors: vec!["Test Author".to_string()],
            categories: Some(vec![Categories::FM]),
            chapters_total: Some(2),
            chapters_published: 1,
            characters: Some(vec!["Character A".to_string(), "Character B".to_string()]),
            complete: false,
            fandoms: vec!["Test Fandom".to_string()],
            hits: 100,
            kudos: 50,
            language: "English".to_string(),
            rating: Rating::General,
            relationships: Some(vec!["A/B".to_string()]),
            restricted: false,
            summary: "A test fanfiction.".to_string(),
            tags: Some(vec!["Tag 1".to_string(), "Tag 2".to_string()]),
            warnings: vec![ArchiveWarnings::NoArchiveWarningsApply],
            words: 1000,
            date_published: "2025-01-01T12:00:00Z".parse().unwrap(),
            date_updated: "2025-01-01T12:00:00Z".parse().unwrap(),
            last_chapter_read: None,
            reading_status: ReadingStatus::PlanToRead,
            read_count: 0,
            user_rating: None,
            personal_note: None,
            last_checked_date: Utc::now(),
        }
    }
    
    /// Adds a fanfiction to the test database.
    pub fn when_fanfiction_added_to_db(conn: &Connection, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
        insert_fanfiction(conn, fic)?;
        Ok(())
    }
    
    /// Deletes a fanfiction from the test database.
    pub fn when_fanfiction_deleted_from_db(conn: &Connection, fic_id: u64) -> Result<(), Box<dyn Error>> {
        delete_fanfiction(conn, fic_id)?;
        Ok(())
    }
    
    /// Fetches a fanfiction using the provided fetcher.
    pub fn when_fetching_fanfiction(
        fetcher: &dyn FanfictionFetcher, 
        fic_id: u64, 
        base_url: &str
    ) -> Result<Fanfiction, Box<dyn Error>> {
        fetcher.fetch_fanfiction(fic_id, base_url)
    }
}