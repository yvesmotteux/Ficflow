use std::fs;
use httpmock::MockServer;
use httpmock::prelude::*;
use ficflow::infrastructure::ao3::fetch_fic::fetch_fanfiction;

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use ficflow::domain::fic::{assert_fanfiction_eq, ArchiveWarnings, Categories, Fanfiction, Rating, ReadingStatus};

    use super::*;

    #[test]
    fn test_fetch_fanfiction_from_mock() {
        let mock_server = MockServer::start();
        
        // Given
        let html_content = fs::read_to_string("tests/fixtures/ao3_fic_example1.html")
            .expect("Failed to read mock HTML file");

        let mock = mock_server.mock(|when, then| {
            when.method(GET).path("/works/53960491");
            then.status(200).body(html_content);
        });
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
        let fetched_fanfic = fetch_fanfiction(53960491, &mock_server.base_url()).expect("Failed to fetch fanfiction");

        // Then
        assert_fanfiction_eq(&expected_fanfic, &fetched_fanfic);
        mock.assert();
    }

    #[test]
    fn test_it_can_fetch_a_fanfiction_from_the_real_website() {        
        // Given
        let expected_title = "Brasier Année Zéro".to_string();

        // When
        let fetched_fanfic = fetch_fanfiction(63776797, "https://archiveofourown.org").expect("Failed to fetch fanfiction");

        // Then
        assert_eq!(expected_title, fetched_fanfic.title);
    }
}
