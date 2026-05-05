//! Group F — front-end-only behaviour.
//!
//! These don't drive any new application use-cases; they pin down the
//! GUI's view-coordination logic — search filtering, sort direction,
//! and the right-panel visibility rule. Phase 12's chrome work is
//! likely to touch the same code, so locking the contract now means a
//! regression there shows up as a test failure rather than a manual
//! bug report.

#[cfg(test)]
mod tests {
    use ficflow::interfaces::gui::{ColumnKey, Selection, SortDirection, View};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    fn given_three_fics_named(a: &str, b: &str, c: &str) -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        for (id, title) in [(1u64, a), (2, b), (3, c)] {
            let fic = fixtures::given_sample_fanfiction(id, title);
            fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        }
        GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td)
    }

    /// F26 — search filter: typing into the bar narrows `visible_ids`
    /// to fics whose title/author/fandom/etc. matches; clearing
    /// restores all.
    #[test]
    fn search_filter_narrows_visible_ids() {
        let mut h = given_three_fics_named("Apple Pie", "Banana Bread", "Cherry Tart");
        h.step_n(1);
        assert_eq!(h.app.visible_ids().len(), 3);

        h.app.set_search("banana");
        assert_eq!(h.app.visible_ids(), vec![2]);

        // Case-insensitive matching across the searched fields.
        h.app.set_search("PIE");
        assert_eq!(h.app.visible_ids(), vec![1]);

        h.app.set_search("");
        assert_eq!(h.app.visible_ids().len(), 3);
    }

    /// F27 — sort: switching column / direction reorders `visible_ids`
    /// without touching the underlying `fics` slice.
    #[test]
    fn sort_by_column_reorders_visible_ids() {
        let mut h = given_three_fics_named("Banana", "Apple", "Cherry");
        h.step_n(1);

        h.app.set_sort(ColumnKey::Title, SortDirection::Ascending);
        assert_eq!(h.app.visible_ids(), vec![2, 1, 3], "alphabetical asc");

        h.app.set_sort(ColumnKey::Title, SortDirection::Descending);
        assert_eq!(h.app.visible_ids(), vec![3, 1, 2], "alphabetical desc");
    }

    /// F29 — the details panel is mounted only when EXACTLY one fic
    /// is selected AND the active view is a library view (not Tasks /
    /// Settings). Empty selection, multi-selection, and non-library
    /// views all hide it.
    #[test]
    fn details_panel_visibility_rule() {
        let mut h = given_three_fics_named("A", "B", "C");
        h.step_n(1);

        // No selection → hidden.
        assert!(matches!(h.app.selection(), Selection::None));
        assert!(!h.app.details_panel_visible());

        // Single + library view → visible.
        h.app.select_fic(1);
        assert!(h.app.details_panel_visible());

        // Multi → hidden (the user can't be looking at a single fic's
        // details when several are selected).
        h.app.select_fics(&[1, 2]);
        assert!(!h.app.details_panel_visible());

        // Single but Tasks view → hidden.
        h.app.select_fic(1);
        h.app.open_view(View::Tasks);
        h.step();
        assert!(!h.app.details_panel_visible());

        // Single but Settings view → also hidden.
        h.app.open_view(View::Settings);
        h.step();
        assert!(!h.app.details_panel_visible());

        // Back to a library view → visible again.
        // (Selection survives view switches as long as the new view
        // is a library view; it's pruned on Tasks/Settings switches,
        // so re-select to restore it.)
        h.app.open_view(View::AllFics);
        h.app.select_fic(1);
        h.step();
        assert!(h.app.details_panel_visible());
    }
}
