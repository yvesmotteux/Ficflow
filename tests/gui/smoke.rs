//! Smoke tests for the GUI harness itself. They prove the scaffolding
//! works end-to-end: the harness boots, ticks frames without panicking,
//! and surfaces the expected initial state (empty library / no
//! selection / All Fanfictions view). The 11.4 scenario tests build on
//! this foundation.

#[cfg(test)]
mod tests {
    use ficflow::interfaces::gui::{Selection, View};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    #[test]
    fn harness_boots_and_ticks_without_panic() {
        let mut harness = GuiHarness::new(vec!["http://127.0.0.1:1".into()]);

        // 5 frames is arbitrary — egui needs at least one frame to lay
        // out widgets; a few more catch any second-frame initialisation
        // bugs (e.g. the `initial_window_state_applied` gate).
        harness.step_n(5);

        // Default state on a fresh DB.
        assert_eq!(harness.app.fics().len(), 0);
        assert_eq!(harness.app.shelves().len(), 0);
        assert!(matches!(harness.app.selection(), Selection::None));
        assert!(matches!(harness.app.current_view(), View::AllFics));
        assert_eq!(harness.app.search_query(), "");
        assert!(!harness.app.has_running_tasks());
    }

    #[test]
    fn harness_sees_pre_seeded_fixtures() {
        // Seed two fics through the same Connection the harness will
        // open later — verifies that the path-based injection actually
        // points the GUI at the test DB.
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic1 = fixtures::given_sample_fanfiction(1, "Alpha Tale");
        let fic2 = fixtures::given_sample_fanfiction(2, "Beta Tale");
        fixtures::when_fanfiction_added_to_db(&conn, &fic1).unwrap();
        fixtures::when_fanfiction_added_to_db(&conn, &fic2).unwrap();

        let mut harness = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        harness.step_n(2);

        let titles: Vec<&str> = harness
            .app
            .fics()
            .iter()
            .map(|f| f.title.as_str())
            .collect();
        assert_eq!(titles.len(), 2);
        // `list_fanfictions` sorts by title.
        assert!(titles.contains(&"Alpha Tale"));
        assert!(titles.contains(&"Beta Tale"));
    }
}
