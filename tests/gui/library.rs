//! Group A — library essentials.
//!
//! Covers `add_fic`, `delete_fic`, `get_fic`, `list_fics`, `check_updates`.
//! Each test boots a `GuiHarness` against a per-test temp DB and (when
//! the scenario needs network) an `httpmock` AO3 server.

#[cfg(test)]
mod tests {
    use ficflow::interfaces::gui::{Selection, TaskStatus, View};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    /// A1 — empty library: a fresh DB renders an empty table, no
    /// shelves, no selection, default `AllFics` view.
    #[test]
    fn empty_library_renders_clean_state() {
        let mut h = GuiHarness::new(vec!["http://127.0.0.1:1".into()]);
        h.step_n(2);

        assert!(h.app.fics().is_empty());
        assert!(h.app.shelves().is_empty());
        assert!(matches!(h.app.selection(), Selection::None));
        assert!(matches!(h.app.current_view(), View::AllFics));
        assert!(!h.app.has_running_tasks());
    }

    /// A2 — happy path add: submitting an AO3 fic ID fires a worker
    /// fetch, persists the fic, and surfaces it in the table on the
    /// next frame.
    #[test]
    fn add_fic_via_id_appears_in_library() {
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let mut h = GuiHarness::new(vec![mock_server.base_url()]);
        h.step_n(1);

        // Pass the numeric ID — `extract_ao3_id` handles it directly.
        // (Full URLs work in production because they hit the literal
        // `archiveofourown.org` host, which the regex requires; tests
        // shouldn't fake that hostname.)
        h.app.submit_add_fic(fic_id.to_string());

        assert!(
            h.wait_for_tasks(80),
            "worker did not finish add task in time"
        );

        let fics = h.app.fics();
        assert_eq!(fics.len(), 1, "exactly one fic was added");
        assert_eq!(fics[0].id, fic_id);

        // The corresponding task should now be Done.
        let states = h.app.task_states();
        assert_eq!(states.len(), 1);
        assert!(
            matches!(states[0].status, TaskStatus::Done),
            "expected Done, got {:?}",
            states[0].status
        );
    }

    /// A3 — failure path: an unmocked ID bounces off `httpmock` as a
    /// 404 (since no matcher is registered), the worker records a
    /// Failed task, and the library stays empty.
    #[test]
    fn add_fic_with_unreachable_target_records_failed_task() {
        // Server is started but no mocks → any /works/N returns 404.
        let mock_server = httpmock::MockServer::start();
        let mut h = GuiHarness::new(vec![mock_server.base_url()]);
        h.step_n(1);

        h.app.submit_add_fic("9999999");

        assert!(
            h.wait_for_tasks(80),
            "worker did not finish failed task in time"
        );

        // Library is still empty because the task failed.
        assert!(h.app.fics().is_empty(), "no fic should be persisted");

        // The task should be in `Failed` state.
        let states = h.app.task_states();
        assert_eq!(states.len(), 1);
        assert!(
            matches!(states[0].status, TaskStatus::Failed(_)),
            "expected Failed, got {:?}",
            states[0].status
        );
    }

    /// A4 — selecting a row populates the details panel by switching
    /// `selection` to `Single(id)` and refreshing the per-fic shelf-id
    /// cache (which is what feeds the shelves dropdown's checked
    /// state).
    #[test]
    fn clicking_a_row_selects_and_loads_details() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(101, "My Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();

        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);
        assert!(matches!(h.app.selection(), Selection::None));

        h.app.select_fic(101);
        h.step();

        assert!(matches!(h.app.selection(), Selection::Single(101)));
    }

    /// A5 — clicking "Delete Fic" in the details panel soft-deletes
    /// the fic, removes it from the in-memory table, and clears the
    /// orphaned selection on the next render pass.
    #[test]
    fn delete_fic_from_details_removes_row_and_clears_selection() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(202, "Doomed Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();

        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);
        h.app.select_fic(202);
        h.step();

        h.app.delete_selected();
        h.step();

        assert!(
            h.app.fics().is_empty(),
            "in-memory cache reflects soft-delete"
        );
        assert!(
            matches!(h.app.selection(), Selection::None),
            "selection clears once the fic disappears from `fics`"
        );

        // And the DB row is genuinely soft-deleted (deleted_at IS NOT NULL),
        // matching what `list_fanfictions` filters out.
        use ficflow::domain::fanfiction::FanfictionOps;
        use ficflow::infrastructure::SqliteRepository;
        let repo = SqliteRepository::new(&h.conn);
        assert!(repo.list_fanfictions().unwrap().is_empty());
    }

    /// A6 — the ↻ refresh button kicks off a `check_fic_updates`
    /// background fetch. When the worker finishes, the in-memory fic
    /// has the freshly-fetched metadata and a bumped `last_checked_date`.
    #[test]
    fn refresh_selected_fic_updates_metadata_and_last_checked() {
        // Seed the DB with a fic carrying an obviously-stale title /
        // last_checked_date so we can detect the refresh overwriting them.
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut stale = fixtures::given_sample_fanfiction(53960491, "Stale Title");
        // Push the last_checked_date a year into the past — the refresh
        // must overwrite it with `Utc::now()` (modulo test-runtime drift).
        stale.last_checked_date = chrono::DateTime::from_naive_utc_and_offset(
            chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
            chrono::Utc,
        );
        fixtures::when_fanfiction_added_to_db(&conn, &stale).unwrap();

        // Mock returns the example1.html, which the parser turns into a
        // fic whose real title is *not* "Stale Title".
        let (mock_server, _fic_id) = fixtures::given_mock_ao3_server();
        let mut h = GuiHarness::with_db(vec![mock_server.base_url()], conn, db_path, td);
        h.step_n(1);
        h.app.select_fic(53960491);
        h.step();

        let stale_checked = h.app.fics()[0].last_checked_date;
        h.app.refresh_selected();
        assert!(h.wait_for_tasks(80));

        let refreshed = &h.app.fics()[0];
        assert_ne!(
            refreshed.title, "Stale Title",
            "title should be overwritten with whatever the mock fixture parses to"
        );
        assert!(
            refreshed.last_checked_date > stale_checked,
            "last_checked_date should advance past the seeded 2020 timestamp"
        );
    }
}
