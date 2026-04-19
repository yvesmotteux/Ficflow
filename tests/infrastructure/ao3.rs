use crate::common::{assertions, fixtures};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ficflow::{
        domain::fanfiction::{
            ArchiveWarnings, Categories, Fanfiction, FanfictionFetcher, Rating, ReadingStatus,
            UserRating,
        },
        infrastructure::external::ao3::Ao3Fetcher,
    };
    use std::time::Duration;

    fn test_fetcher(base_url: String) -> Ao3Fetcher {
        Ao3Fetcher::with_min_gap(vec![base_url], 1, Duration::ZERO, Duration::from_millis(1))
            .unwrap()
    }

    #[test]
    fn test_fetch_fanfiction_from_mock() {
        // Given
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let fetcher = test_fetcher(mock_server.base_url());

        let expected_fanfic = Fanfiction {
            id: 53960491,
            title: "Featherlight".to_string(),
            authors: vec!["Gummy_bean".to_string()],
            categories: Some(vec![Categories::FM]),
            chapters_total: Some(1),
            chapters_published: 1,
            characters: Some(vec![
                "Charlie Magne | Morningstar".to_string(),
                "Lucifer Magne | Morningstar".to_string(),
            ]),
            complete: true,
            fandoms: vec!["Hazbin Hotel (Cartoon)".to_string()],
            hits: 2295,
            kudos: 159,
            language: "English".to_string(),
            rating: Rating::TeenAndUp,
            relationships: Some(vec![
                "Charlie Magne | Morningstar/Lucifer Magne | Morningstar".to_string(),
            ]),
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
        let fetched_fanfic = fixtures::when_fetching_fanfiction(&fetcher, fic_id)
            .expect("Failed to fetch fanfiction");

        // Then
        assertions::then_fanfiction_was_fetched(&expected_fanfic, &fetched_fanfic, None);
    }

    #[test]
    fn falls_back_from_failing_url_to_working_url() {
        use httpmock::{Method::GET, MockServer};

        // Broken server — returns 500 on any GET request.
        let broken = MockServer::start();
        broken.mock(|when, then| {
            when.method(GET);
            then.status(500);
        });

        // Working server — serves the real fanfiction fixture.
        let (working, fic_id) = fixtures::given_mock_ao3_server();

        let fetcher = Ao3Fetcher::with_min_gap(
            vec![broken.base_url(), working.base_url()],
            1,
            Duration::ZERO,
            Duration::from_millis(1),
        )
        .unwrap();

        let fic = fetcher
            .fetch_fanfiction(fic_id)
            .expect("fallback should reach the working server");
        assert_eq!(fic.id, fic_id);
        assert_eq!(fic.title, "Featherlight");
    }

    #[test]
    fn test_check_fic_updates() {
        use ficflow::{
            application::check_updates::check_fic_updates, domain::fanfiction::FanfictionOps,
            infrastructure::persistence::repository::SqliteRepository,
        };

        // Given
        let (conn, _path, _temp_dir) = fixtures::given_test_database();
        let fanfiction_ops = SqliteRepository::new(&conn);

        let (outdated_server, fic_id) = fixtures::given_mock_outdated_ao3_server();
        let (updated_server, _) = fixtures::given_mock_up_to_date_ao3_server();

        let outdated_fetcher = test_fetcher(outdated_server.base_url());
        let updated_fetcher = test_fetcher(updated_server.base_url());

        let mut outdated_fic = fixtures::when_fetching_fanfiction(&outdated_fetcher, fic_id)
            .expect("Failed to fetch outdated fanfiction");
        assert_eq!(
            outdated_fic.chapters_published, 18,
            "Outdated fic should have 18 chapters"
        );

        outdated_fic.personal_note = Some("This is my favorite Alastor fic!".to_string());
        outdated_fic.user_rating = Some(UserRating::Five);
        outdated_fic.last_chapter_read = Some(15);
        outdated_fic.reading_status = ReadingStatus::InProgress;
        outdated_fic.read_count = 3;

        fanfiction_ops
            .save_fanfiction(&outdated_fic)
            .expect("Failed to save outdated fic");

        // When
        let (has_new_chapters, updated_fic) =
            check_fic_updates(&updated_fetcher, &fanfiction_ops, fic_id)
                .expect("Failed to check for updates");

        // Then
        assert!(has_new_chapters, "Should detect new chapters");
        assert_eq!(
            updated_fic.chapters_published, 32,
            "Updated fic should have 32 chapters"
        );

        let stored_fic = fanfiction_ops
            .get_fanfiction_by_id(fic_id)
            .expect("Failed to retrieve from DB");
        assert_eq!(
            stored_fic.chapters_published, 32,
            "DB should have updated chapter count"
        );

        assert_eq!(stored_fic.kudos, 4305, "Kudos should be updated");
        assert_eq!(stored_fic.hits, 135291, "Hits should be updated");
        assert_eq!(
            stored_fic.rating,
            Rating::Explicit,
            "Rating should be updated to Explicit"
        );

        // Verify that custom user data is preserved
        assert_eq!(
            stored_fic.personal_note,
            Some("This is my favorite Alastor fic!".to_string()),
            "Personal note should be preserved"
        );
        assert_eq!(
            stored_fic.user_rating,
            Some(UserRating::Five),
            "User rating should be preserved"
        );
        assert_eq!(
            stored_fic.last_chapter_read,
            Some(15),
            "Last chapter read should be preserved"
        );
        assert_eq!(
            stored_fic.reading_status,
            ReadingStatus::InProgress,
            "Reading status should be preserved"
        );
        assert_eq!(stored_fic.read_count, 3, "Read count should be preserved");

        // Verify no changes reported when checking again
        let (has_newer_chapters, _) = check_fic_updates(&updated_fetcher, &fanfiction_ops, fic_id)
            .expect("Failed to check for updates second time");

        assert!(
            !has_newer_chapters,
            "Second update should report no new chapters"
        );
    }
}
