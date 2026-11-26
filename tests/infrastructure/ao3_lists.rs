#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use ficflow::domain::fanfiction::FanfictionFetcher;
    use ficflow::error::FicflowError;
    use ficflow::infrastructure::external::ao3::Ao3Fetcher;
    use httpmock::prelude::*;

    fn fixture(name: &str) -> String {
        fs::read_to_string(format!("tests/fixtures/lists/{name}.html")).unwrap()
    }

    fn fetcher(server: &MockServer) -> Ao3Fetcher {
        Ao3Fetcher::with_min_gap(
            vec![server.base_url()],
            1,
            Duration::ZERO,
            Duration::from_millis(1),
        )
        .unwrap()
    }

    fn serve_pages(server: &MockServer, path: &str, pages: &[&str]) {
        for (i, name) in pages.iter().enumerate().skip(1) {
            let body = fixture(name);
            let page = (i + 1).to_string();
            server.mock(|when, then| {
                when.method(GET).path(path).query_param("page", page);
                then.status(200).body(body);
            });
        }
        let first = fixture(pages[0]);
        server.mock(|when, then| {
            when.method(GET).path(path).query_param_missing("page");
            then.status(200).body(first);
        });
    }

    #[test]
    fn follows_next_page_links_until_the_last_page() {
        let server = MockServer::start();
        serve_pages(
            &server,
            "/users/KayleySayer/works",
            &["user_works_p1", "user_works_p2", "user_works_p3"],
        );

        let list = fetcher(&server)
            .fetch_work_ids("/users/KayleySayer/works", 100)
            .unwrap();

        assert_eq!(list.total, Some(42));
        assert_eq!(list.ids.len(), 42);
        assert_eq!(list.ids.first(), Some(&96703796));
        assert_eq!(list.ids.last(), Some(&48226681));
    }

    #[test]
    fn series_are_read_page_by_page_with_their_total() {
        let server = MockServer::start();
        serve_pages(&server, "/series/6653231", &["series_p1", "series_p2"]);

        let list = fetcher(&server)
            .fetch_work_ids("/series/6653231", 100)
            .unwrap();

        assert_eq!(list.total, Some(31));
        assert_eq!(list.ids.len(), 31);
        assert_eq!(list.ids.first(), Some(&93626551));
        assert_eq!(list.ids.last(), Some(&96172631));
    }

    #[test]
    fn bookmarks_keep_only_works() {
        let server = MockServer::start();
        serve_pages(&server, "/users/KayleySayer/bookmarks", &["user_bookmarks"]);

        let list = fetcher(&server)
            .fetch_work_ids("/users/KayleySayer/bookmarks", 18)
            .unwrap();

        assert_eq!(list.total, Some(27));
        assert_eq!(list.ids.len(), 18);
        assert_eq!(list.ids.first(), Some(&69081951));
        assert!(!list.ids.contains(&1914706));
    }

    #[test]
    fn collection_works_link_through_the_collection() {
        let server = MockServer::start();
        serve_pages(
            &server,
            "/collections/HP_Yule_Bliss_Parent_Collection/works",
            &["collection_big"],
        );

        let list = fetcher(&server)
            .fetch_work_ids("/collections/HP_Yule_Bliss_Parent_Collection/works", 100)
            .unwrap();

        assert_eq!(list.total, Some(112));
        assert_eq!(list.ids.len(), 20);
        assert_eq!(list.ids.first(), Some(&75589916));
    }

    #[test]
    fn subscriptions_keep_only_works() {
        let server = MockServer::start();
        serve_pages(&server, "/users/reader/subscriptions", &["subscriptions"]);

        let list = fetcher(&server)
            .fetch_work_ids("/users/reader/subscriptions?type=works", 100)
            .unwrap();

        assert_eq!(list.ids, vec![601]);
        assert_eq!(list.total, None);
    }

    #[test]
    fn login_redirect_on_a_list_means_login_required() {
        let server = MockServer::start();
        let login = format!("{}/users/login", server.base_url());
        server.mock(|when, then| {
            when.method(GET).path("/users/reader/subscriptions");
            then.status(302).header("Location", login);
        });
        server.mock(|when, then| {
            when.method(GET).path("/users/login");
            then.status(200).body("<html></html>");
        });

        let err = fetcher(&server)
            .fetch_work_ids("/users/reader/subscriptions?type=works", 100)
            .unwrap_err();

        assert!(matches!(err, FicflowError::LoginRequired));
    }

    #[test]
    fn reads_a_series_and_all_its_parts() {
        let server = MockServer::start();
        serve_pages(&server, "/series/6653231", &["series_p1", "series_p2"]);

        let series = fetcher(&server).fetch_series(6653231).unwrap();

        assert_eq!(series.title, "Arslan-tober 2026: Tiny Tales");
        assert_eq!(series.authors, vec!["KayleySayer"]);
        assert_eq!(
            series.date_begun.unwrap().to_string(),
            "2026-10-01 00:00:00 UTC"
        );
        assert_eq!(
            series.date_updated.unwrap().to_string(),
            "2026-10-31 00:00:00 UTC"
        );
        assert!(
            series
                .description
                .unwrap()
                .starts_with("A collection of Arslan Senki")
        );
        assert!(
            series
                .notes
                .unwrap()
                .contains("\n\nThe original Tumblr prompt list")
        );
        assert_eq!(
            (series.words, series.works_count, series.bookmarks),
            (33428, 31, 2)
        );
        assert!(!series.complete);
        let parts: Vec<u32> = series.works.iter().map(|w| w.part).collect();
        assert_eq!(parts, (1..=31).collect::<Vec<_>>());
        assert_eq!(series.works[0].work_id, 93626551);
        assert_eq!(series.works[0].title, "A Matter of Perspective");
        assert_eq!(series.works[0].authors, vec!["KayleySayer"]);
        assert_eq!(series.works[30].work_id, 96172631);
        assert_eq!(series.works[30].title, "The Farther Shore");
    }
}
