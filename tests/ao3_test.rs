#[path = "common/mod.rs"]
mod common;
use common::{fixtures, assertions};

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use ficflow::{
        domain::{
            url_config,
            fanfiction::{ArchiveWarnings, Categories, Fanfiction, Rating, ReadingStatus}
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
}
