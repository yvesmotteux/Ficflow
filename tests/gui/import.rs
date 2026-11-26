#[cfg(test)]
mod tests {
    use std::time::Duration;

    use httpmock::Method::GET;
    use httpmock::MockServer;

    use ficflow::infrastructure::external::ao3::session::Ao3Session;
    use ficflow::interfaces::gui::{ImportSource, TaskKind, TaskState, TaskStatus};
    use ficflow::interfaces::utils::url_parser::{Ao3List, subscriptions_list};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    fn fixture(path: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{path}.html")).unwrap()
    }

    fn given_works(server: &MockServer, ids: &[u64], delay: Duration) {
        for id in ids {
            server.mock(|when, then| {
                when.method(GET).path(format!("/works/{id}"));
                then.status(200)
                    .delay(delay)
                    .body(fixture("ao3_fic_example1"));
            });
        }
    }

    fn given_harness(server: &MockServer, existing: &[u64]) -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        for id in existing {
            let fic = fixtures::given_sample_fanfiction(*id, &format!("Fic {id}"));
            fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        }
        let mut h = GuiHarness::with_db(vec![server.base_url()], conn, db_path, td);
        h.step();
        h
    }

    fn list(path: &str, label: &str) -> ImportSource {
        ImportSource::List(Ao3List {
            path: path.to_string(),
            label: label.to_string(),
        })
    }

    fn import_task(h: &GuiHarness) -> TaskState {
        h.app
            .task_states()
            .into_iter()
            .find(|t| matches!(t.kind, TaskKind::Import { .. }))
            .expect("no import task")
    }

    fn fic_ids(h: &GuiHarness) -> Vec<u64> {
        let mut ids: Vec<u64> = h.app.fics().iter().map(|f| f.id).collect();
        ids.sort();
        ids
    }

    #[test]
    fn importing_ids_adds_the_new_ones_and_skips_the_rest() {
        let server = MockServer::start();
        given_works(&server, &[1, 2], Duration::ZERO);
        let mut h = given_harness(&server, &[3]);

        h.app.submit_import(ImportSource::Ids(vec![1, 2, 3]));
        assert!(h.wait_for_tasks(400));

        assert_eq!(fic_ids(&h), vec![1, 2, 3]);
        let import = import_task(&h);
        assert!(matches!(import.status, TaskStatus::Done));
        assert_eq!(
            import.display,
            "3 fic IDs: 2 fics added, 1 already in library"
        );
    }

    #[test]
    fn importing_a_collection_reads_every_page() {
        let server = MockServer::start();
        given_works(&server, &[101, 102, 103], Duration::ZERO);
        let page2 = fixture("lists/collection_page2");
        server.mock(|when, then| {
            when.method(GET)
                .path("/collections/test/works")
                .query_param("page", "2");
            then.status(200).body(page2);
        });
        let page1 = fixture("lists/collection_page1");
        server.mock(|when, then| {
            when.method(GET)
                .path("/collections/test/works")
                .query_param_missing("page");
            then.status(200).body(page1);
        });
        let mut h = given_harness(&server, &[102]);

        h.app
            .submit_import(list("/collections/test/works", "Collection test"));
        assert!(h.wait_for_tasks(400));

        assert_eq!(fic_ids(&h), vec![101, 102, 103]);
        assert_eq!(
            import_task(&h).display,
            "Collection test: 2 fics added, 1 already in library"
        );
    }

    #[test]
    fn big_lists_wait_for_confirmation_before_adding_anything() {
        let server = MockServer::start();
        given_works(&server, &[801, 802, 803, 804], Duration::ZERO);
        server.mock(|when, then| {
            when.method(GET)
                .path("/collections/big/works")
                .query_param("page", "2");
            then.status(200).body(
                r#"<div id="main"><ol class="work index group"><li class="work blurb"><h4 class="heading"><a href="/works/804">Last</a></h4></li></ol></div>"#,
            );
        });
        let page1 = fixture("lists/big_page1");
        server.mock(|when, then| {
            when.method(GET)
                .path("/collections/big/works")
                .query_param_missing("page");
            then.status(200).body(page1);
        });
        let mut h = given_harness(&server, &[]);

        h.app
            .submit_import(list("/collections/big/works", "Collection big"));
        assert!(h.wait_for_tasks(400));

        let waiting = import_task(&h);
        match &waiting.status {
            TaskStatus::NeedsConfirmation(message) => assert_eq!(
                message,
                "Collection big has 1,234 fics. Adding them all would take about 1 h 23 min."
            ),
            other => panic!("expected a confirmation request, got {other:?}"),
        }
        assert!(h.app.fics().is_empty());

        h.app.confirm_import(waiting.id);
        assert!(h.wait_for_tasks(400));

        assert_eq!(fic_ids(&h), vec![801, 802, 803, 804]);
        assert!(matches!(import_task(&h).status, TaskStatus::Done));
    }

    #[test]
    fn previously_deleted_fics_are_added_back() {
        let server = MockServer::start();
        given_works(&server, &[7], Duration::ZERO);
        let mut h = given_harness(&server, &[7]);
        fixtures::when_fanfiction_deleted_from_db(&h.conn, 7).unwrap();

        h.app.submit_import(ImportSource::Ids(vec![7, 8]));
        assert!(h.wait_for_tasks(400));

        assert_eq!(fic_ids(&h), vec![7]);
        assert_eq!(import_task(&h).display, "2 fic IDs: 1 fic added, 1 failed");
    }

    #[test]
    fn importing_subscriptions_uses_the_ao3_login() {
        let server = MockServer::start();
        given_works(&server, &[601], Duration::ZERO);
        let subscriptions = server.mock(|when, then| {
            when.method(GET)
                .path("/users/reader/subscriptions")
                .query_param("type", "works")
                .header_includes("cookie", "remember_user_token=abc");
            then.status(200).body(fixture("lists/subscriptions"));
        });
        let mut h = given_harness(&server, &[]);
        h.session_store.set(Some(Ao3Session {
            username: "reader".into(),
            cookie: "remember_user_token=abc".into(),
        }));

        h.app
            .submit_import(ImportSource::List(subscriptions_list("reader")));
        assert!(h.wait_for_tasks(400));

        subscriptions.assert_calls(1);
        assert_eq!(fic_ids(&h), vec![601]);
    }

    #[test]
    fn cancelling_an_import_skips_the_fics_not_added_yet() {
        let server = MockServer::start();
        given_works(&server, &[1, 2, 3, 4], Duration::from_millis(300));
        let mut h = given_harness(&server, &[]);

        h.app.submit_import(ImportSource::Ids(vec![1, 2, 3, 4]));
        while h.app.task_states().len() < 5 {
            h.step();
        }
        h.app.cancel_import(import_task(&h).id);
        assert!(h.wait_for_tasks(400));

        assert!(h.app.fics().len() <= 1);
        assert!(matches!(import_task(&h).status, TaskStatus::Cancelled));
        let cancelled = h
            .app
            .task_states()
            .iter()
            .filter(|t| matches!(t.status, TaskStatus::Cancelled))
            .count();
        assert!(cancelled >= 4);
    }
}
