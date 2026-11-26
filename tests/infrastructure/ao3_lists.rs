#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use ficflow::domain::fanfiction::{FanfictionFetcher, WorkList};
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

    fn serve(server: &MockServer, path: &str, name: &str) {
        let body = fixture(name);
        server.mock(|when, then| {
            when.method(GET).path(path);
            then.status(200).body(body);
        });
    }

    #[test]
    fn follows_next_page_links_until_the_last_page() {
        let server = MockServer::start();
        let page2 = fixture("collection_page2");
        server.mock(|when, then| {
            when.method(GET)
                .path("/collections/test/works")
                .query_param("page", "2");
            then.status(200).body(page2);
        });
        let page1 = fixture("collection_page1");
        server.mock(|when, then| {
            when.method(GET)
                .path("/collections/test/works")
                .query_param_missing("page");
            then.status(200).body(page1);
        });

        let list = fetcher(&server)
            .fetch_work_ids("/collections/test/works", 100)
            .unwrap();

        assert_eq!(
            list,
            WorkList {
                ids: vec![101, 102, 103],
                total: Some(3),
            }
        );
    }

    #[test]
    fn bookmarks_keep_only_works() {
        let server = MockServer::start();
        serve(&server, "/users/reader/bookmarks", "bookmarks");

        let list = fetcher(&server)
            .fetch_work_ids("/users/reader/bookmarks", 100)
            .unwrap();

        assert_eq!(list.ids, vec![201, 202]);
        assert_eq!(list.total, Some(4));
    }

    #[test]
    fn series_total_comes_from_the_series_stats() {
        let server = MockServer::start();
        serve(&server, "/series/999", "series");

        let list = fetcher(&server).fetch_work_ids("/series/999", 100).unwrap();

        assert_eq!(list.ids, vec![501, 502]);
        assert_eq!(list.total, Some(2));
    }

    #[test]
    fn subscriptions_keep_only_works() {
        let server = MockServer::start();
        serve(&server, "/users/reader/subscriptions", "subscriptions");

        let list = fetcher(&server)
            .fetch_work_ids("/users/reader/subscriptions?type=works", 100)
            .unwrap();

        assert_eq!(list.ids, vec![601]);
        assert_eq!(list.total, None);
    }

    #[test]
    fn stops_after_the_first_page_when_the_total_is_over_the_limit() {
        let server = MockServer::start();
        serve(&server, "/collections/big/works", "big_page1");

        let list = fetcher(&server)
            .fetch_work_ids("/collections/big/works", 100)
            .unwrap();

        assert_eq!(list.ids, vec![801, 802, 803]);
        assert_eq!(list.total, Some(1234));
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
}
