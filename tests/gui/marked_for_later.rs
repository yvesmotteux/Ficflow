#[cfg(test)]
mod tests {
    use std::time::Duration;

    use httpmock::Method::GET;
    use httpmock::{Mock, MockServer};

    use ficflow::domain::fanfiction::ReadingStatus;
    use ficflow::infrastructure::config::config_file_path;
    use ficflow::infrastructure::external::ao3::session::Ao3Session;
    use ficflow::interfaces::gui::TaskKind;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    fn fixture(path: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{path}.html")).unwrap()
    }

    fn given_any_work(server: &MockServer) {
        server.mock(|when, then| {
            when.method(GET).path_matches(r"^/works/\d+$");
            then.status(200).body(fixture("ao3_fic_example1"));
        });
    }

    fn given_marked_for_later(server: &MockServer) -> Mock<'_> {
        server.mock(|when, then| {
            when.method(GET)
                .path("/users/reader/readings")
                .query_param("show", "to-read")
                .query_param("page", "2")
                .header_includes("cookie", "remember_user_token=abc");
            then.status(200).body(fixture("lists/marked_for_later_p2"));
        });
        server.mock(|when, then| {
            when.method(GET)
                .path("/users/reader/readings")
                .query_param("show", "to-read")
                .query_param_missing("page")
                .header_includes("cookie", "remember_user_token=abc");
            then.status(200).body(fixture("lists/marked_for_later_p1"));
        })
    }

    fn given_logged_in_harness(server: &MockServer, existing: &[u64]) -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        for id in existing {
            let fic = fixtures::given_sample_fanfiction(*id, &format!("Fic {id}"));
            fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        }
        let mut h = GuiHarness::with_db(vec![server.base_url()], conn, db_path, td);
        log_in_and_restart(&mut h, server);
        h
    }

    fn log_in_and_restart(h: &mut GuiHarness, server: &MockServer) {
        h.session_store.set(Some(Ao3Session {
            username: "reader".into(),
            cookie: "remember_user_token=abc".into(),
        }));
        h.restart(vec![server.base_url()]);
        h.step();
    }

    fn wait_for_sync(h: &mut GuiHarness) {
        h.step_n(3);
        assert!(h.wait_for_tasks(400), "sync did not finish");
    }

    fn fic_ids(h: &GuiHarness) -> Vec<u64> {
        let mut ids: Vec<u64> = h.app.fics().iter().map(|f| f.id).collect();
        ids.sort();
        ids
    }

    fn status_of(h: &GuiHarness, id: u64) -> ReadingStatus {
        h.app
            .fics()
            .iter()
            .find(|f| f.id == id)
            .expect("fic missing")
            .reading_status
    }

    fn import_rows(h: &GuiHarness) -> usize {
        h.app
            .task_states()
            .iter()
            .filter(|t| matches!(t.kind, TaskKind::Import { .. }))
            .count()
    }

    fn set_last_sync(value: &str) {
        let path = config_file_path().unwrap();
        let mut table: toml::Table =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).expect("config should parse");
        table.insert(
            "marked_for_later_synced_at".into(),
            toml::Value::String(value.into()),
        );
        std::fs::write(&path, toml::to_string(&table).unwrap()).unwrap();
    }

    #[test]
    fn it_stays_off_until_turned_on() {
        let server = MockServer::start();
        given_any_work(&server);
        let readings = given_marked_for_later(&server);
        let mut h = given_logged_in_harness(&server, &[]);

        h.step_n(10);
        std::thread::sleep(Duration::from_millis(100));
        h.step_n(5);

        readings.assert_calls(0);
        assert!(fic_ids(&h).is_empty());
    }

    #[test]
    fn turning_it_on_adds_every_marked_fic_as_plan_to_read() {
        let server = MockServer::start();
        given_any_work(&server);
        let readings = given_marked_for_later(&server);
        let mut h = given_logged_in_harness(&server, &[]);

        h.app.set_marked_for_later_sync(true);
        wait_for_sync(&mut h);

        readings.assert_calls(1);
        assert_eq!(fic_ids(&h), vec![801, 802, 803]);
        for id in [801, 802, 803] {
            assert_eq!(status_of(&h, id), ReadingStatus::PlanToRead);
        }
    }

    #[test]
    fn a_changed_status_survives_the_next_sync_and_nothing_new_leaves_no_task() {
        let server = MockServer::start();
        given_any_work(&server);
        given_marked_for_later(&server);
        let mut h = given_logged_in_harness(&server, &[]);
        h.app.set_marked_for_later_sync(true);
        wait_for_sync(&mut h);
        h.app.set_status(802, ReadingStatus::Abandoned).unwrap();
        h.app.clear_finished_tasks();

        h.app.set_marked_for_later_sync(true);
        wait_for_sync(&mut h);

        assert_eq!(status_of(&h, 802), ReadingStatus::Abandoned);
        assert_eq!(import_rows(&h), 0);
    }

    #[test]
    fn it_syncs_once_a_day_across_restarts() {
        let server = MockServer::start();
        given_any_work(&server);
        let mut readings = given_marked_for_later(&server);
        let mut h = given_logged_in_harness(&server, &[801, 802, 803]);
        h.app.set_marked_for_later_sync(true);
        wait_for_sync(&mut h);

        h.restart(vec![server.base_url()]);
        wait_for_sync(&mut h);
        readings.assert_calls(1);

        readings.delete();
        let readings = given_marked_for_later(&server);
        set_last_sync("2026-01-01T00:00:00Z");
        h.restart(vec![server.base_url()]);
        wait_for_sync(&mut h);
        readings.assert_calls(1);
    }

    #[test]
    fn a_fic_deleted_in_ficflow_comes_back() {
        let server = MockServer::start();
        given_any_work(&server);
        given_marked_for_later(&server);
        let mut h = given_logged_in_harness(&server, &[801, 802, 803]);
        h.app.select_fic(801);
        h.app.delete_selected();
        h.step();
        assert_eq!(fic_ids(&h), vec![802, 803]);

        h.app.set_marked_for_later_sync(true);
        wait_for_sync(&mut h);

        assert_eq!(fic_ids(&h), vec![801, 802, 803]);
    }
}
