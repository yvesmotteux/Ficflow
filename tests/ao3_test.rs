#[path = "common/mod.rs"]
mod common;
use common::{fixtures, assertions};

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use ficflow::{
        domain::{
            url_config,
            fanfiction::{ArchiveWarnings, Categories, Fanfiction, Rating, ReadingStatus, UserRating}
        }, 
        infrastructure::external::ao3::Ao3Fetcher
    };
    use super::*;

    #[test]
    fn test_fetch_fanfiction_from_mock() {
        // Given
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let fetcher = Ao3Fetcher::new().unwrap();
        
        let expected_fanfic = Fanfiction {
            id: 53960491,
            title: "Featherlight".to_string(),
            authors: vec!["Gummy_bean".to_string()],
            categories: Some(vec![Categories::FM]),
            chapters_total: Some(1),
            chapters_published: 1,
            characters: Some(vec!["Charlie Magne | Morningstar".to_string(), "Lucifer Magne | Morningstar".to_string()]),
            complete: true,
            fandoms: vec!["Hazbin Hotel (Cartoon)".to_string()],
            hits: 2295,
            kudos: 159,
            language: "English".to_string(),
            rating: Rating::TeenAndUp,
            relationships: Some(vec!["Charlie Magne | Morningstar/Lucifer Magne | Morningstar".to_string()]),
            restricted: false,
            summary: "Charlie can't resist touching her dad's beautiful wings.".to_string(),
            tags: Some(vec![
                "Wings".to_string(),
                "Wingfic".to_string(),
                "Wings as Erogenous Zone".to_string(),
                "Accidental Stimulation".to_string(),
                "Fluff".to_string(),
                "Praise Kink".to_string(),
                "Tickling".to_string(),
                "Feathers & Featherplay".to_string(),
                "Angel Wings".to_string(),
                "Feel-good".to_string(),
                "Unresolved Tension".to_string(),
                "no established relationships".to_string(),
                "Fluff without Plot".to_string(),
            ]),
            warnings: vec![ArchiveWarnings::NoArchiveWarningsApply],
            words: 1021,
            date_published: "2024-02-21T00:00:00Z".parse().unwrap(),
            date_updated: "2024-02-21T00:00:00Z".parse().unwrap(),
            last_chapter_read: None,
            reading_status: ReadingStatus::PlanToRead,
            read_count: 0,
            user_rating: None,
            personal_note: None,
            last_checked_date: Utc::now(),
        };

        // When
        let fetched_fanfic = fixtures::when_fetching_fanfiction(&fetcher, fic_id, &mock_server.base_url())
            .expect("Failed to fetch fanfiction");

        // Then
        assertions::then_fanfiction_was_fetched(&expected_fanfic, &fetched_fanfic, None);
    }

    #[test]
    fn test_config_base_url() {
        // Given
        let original_url = url_config::get_ao3_base_url();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let fetcher = Ao3Fetcher::new().unwrap();
        
        // When
        url_config::set_ao3_base_url(&mock_server.base_url());
        let result = fixtures::when_fetching_fanfiction(&fetcher, fic_id, &url_config::get_ao3_base_url());
        
        // Then
        assert!(result.is_ok());
        
        // Cleanup
        url_config::set_ao3_base_url(&original_url);
    }
    
    #[test]
    fn test_check_fic_updates() {
        use ficflow::{
            application::check_updates::check_fic_updates,
            domain::fanfiction::DatabaseOps,
            infrastructure::persistence::repository::FanfictionRepository,
            infrastructure::persistence::database::sqlite_connection::Database
        };
        
        // Given
        let (conn, _path, _temp_dir) = fixtures::given_test_database();
        let database = Database::new(&conn);
        let db_ops = FanfictionRepository::new(database);
        
        let fetcher = Ao3Fetcher::new().unwrap();
        let (outdated_server, fic_id) = fixtures::given_mock_outdated_ao3_server();
        let (updated_server, _) = fixtures::given_mock_up_to_date_ao3_server();
        
        let mut outdated_fic = fixtures::when_fetching_fanfiction(
            &fetcher, fic_id, &outdated_server.base_url()
        ).expect("Failed to fetch outdated fanfiction");
        assert_eq!(outdated_fic.chapters_published, 18, "Outdated fic should have 18 chapters");
        
        outdated_fic.personal_note = Some("This is my favorite Alastor fic!".to_string());
        outdated_fic.user_rating = Some(UserRating::Five);
        outdated_fic.last_chapter_read = Some(15);
        outdated_fic.reading_status = ReadingStatus::InProgress;
        outdated_fic.read_count = 3;
        
        db_ops.insert_fanfiction(&outdated_fic).expect("Failed to insert outdated fic");
        
        // When
        let (has_new_chapters, updated_fic) = check_fic_updates(
            &fetcher, 
            &db_ops, 
            fic_id, 
            &updated_server.base_url()
        ).expect("Failed to check for updates");
        
        // Then
        assert!(has_new_chapters, "Should detect new chapters");
        assert_eq!(updated_fic.chapters_published, 32, "Updated fic should have 32 chapters");
        
        let stored_fic = db_ops.get_fanfiction_by_id(fic_id).expect("Failed to retrieve from DB");
        assert_eq!(stored_fic.chapters_published, 32, "DB should have updated chapter count");
        
        assert_eq!(stored_fic.kudos, 4305, "Kudos should be updated");
        assert_eq!(stored_fic.hits, 135291, "Hits should be updated");
        assert_eq!(stored_fic.rating, Rating::Explicit, "Rating should be updated to Explicit");
        
        // Verify that custom user data is preserved
        assert_eq!(stored_fic.personal_note, Some("This is my favorite Alastor fic!".to_string()), 
                  "Personal note should be preserved");
        assert_eq!(stored_fic.user_rating, Some(UserRating::Five), 
                  "User rating should be preserved");
        assert_eq!(stored_fic.last_chapter_read, Some(15), 
                  "Last chapter read should be preserved");
        assert_eq!(stored_fic.reading_status, ReadingStatus::InProgress, 
                  "Reading status should be preserved");
        assert_eq!(stored_fic.read_count, 3, 
                  "Read count should be preserved");
        
        // Verify no changes reported when checking again
        let (has_newer_chapters, _) = check_fic_updates(
            &fetcher, 
            &db_ops, 
            fic_id, 
            &updated_server.base_url()
        ).expect("Failed to check for updates second time");
        
        assert!(!has_newer_chapters, "Second update should report no new chapters");
    }
}
