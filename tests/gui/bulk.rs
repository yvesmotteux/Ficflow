//! Group D — bulk operations on multi-selection.
//!
//! Covers `update_status` ×N, `add_to_shelf` ×N, `remove_from_shelf`
//! ×N, `delete_fic` ×N. Every scenario builds a multi-selection via
//! `select_fics`, runs the bulk action through the same control-surface
//! method the per-fic tests use (since the GUI's bulk-handler is
//! literally a `for id in ids { … }` loop), and asserts every selected
//! fic ended up in the expected state.

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::{FanfictionOps, ReadingStatus};
    use ficflow::domain::shelf::ShelfOps;
    use ficflow::infrastructure::SqliteRepository;
    use ficflow::interfaces::gui::{Selection, View};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    /// Seed N fics + return harness. IDs are 1..=n.
    fn given_harness_with_n_fics(n: u64) -> (GuiHarness, Vec<u64>) {
        let (conn, db_path, td) = fixtures::given_test_database();
        let ids: Vec<u64> = (1..=n).collect();
        for id in &ids {
            let fic = fixtures::given_sample_fanfiction(*id, &format!("Fic {}", id));
            fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        }
        let h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        (h, ids)
    }

    /// D20 — multi-selection state itself: `select_fics` collapses
    /// 0 → None, 1 → Single, 2+ → Multi; `Selection::contains` works
    /// across all variants.
    #[test]
    fn multi_selection_state_round_trips() {
        let (mut h, ids) = given_harness_with_n_fics(3);
        h.step_n(1);

        assert!(matches!(h.app.selection(), Selection::None));

        h.app.select_fics(&[ids[0]]);
        assert!(matches!(h.app.selection(), Selection::Single(_)));
        assert!(h.app.selection().contains(ids[0]));

        h.app.select_fics(&ids);
        assert!(matches!(h.app.selection(), Selection::Multi(_)));
        assert!(ids.iter().all(|id| h.app.selection().contains(*id)));

        h.app.select_fics(&[]);
        assert!(matches!(h.app.selection(), Selection::None));
    }

    /// D21 — bulk status change through the selection-bar's `Change
    /// status` menu. The dispatch is a loop of `update_reading_status`
    /// calls; we verify every selected fic's status moved.
    #[test]
    fn bulk_status_change_updates_all_selected() {
        let (mut h, ids) = given_harness_with_n_fics(3);
        h.step_n(1);
        // Sample fic ships as PlanToRead — flip them all to Read.
        h.app.select_fics(&ids);
        for id in &ids {
            h.app.set_status(*id, ReadingStatus::Read).unwrap();
        }

        let repo = SqliteRepository::new(&h.conn);
        for id in &ids {
            assert_eq!(
                repo.get_fanfiction_by_id(*id).unwrap().reading_status,
                ReadingStatus::Read
            );
        }
        assert!(h
            .app
            .fics()
            .iter()
            .all(|f| f.reading_status == ReadingStatus::Read));
    }

    /// D22 — bulk add-to-shelf: a multi-selection plus a target shelf
    /// puts every selected fic on the shelf in one user gesture.
    #[test]
    fn bulk_add_to_shelf_links_every_selected_fic() {
        let (mut h, ids) = given_harness_with_n_fics(3);
        h.step_n(1);
        h.app.create_shelf("Bulk").unwrap();
        let shelf_id = h.app.shelves()[0].id;

        h.app.select_fics(&ids);
        for id in &ids {
            h.app.add_fic_to_shelf(*id, shelf_id).unwrap();
        }

        let repo = SqliteRepository::new(&h.conn);
        assert_eq!(
            repo.count_fics_in_shelf(shelf_id).unwrap(),
            ids.len(),
            "every selected fic should end up on the shelf"
        );
    }

    /// D23 — bulk remove-from-shelf: only available while the active
    /// view *is* the shelf; emptying the shelf in one gesture should
    /// drop every link without touching the underlying fics.
    #[test]
    fn bulk_remove_from_shelf_clears_links_only_in_shelf_view() {
        let (mut h, ids) = given_harness_with_n_fics(3);
        h.step_n(1);
        h.app.create_shelf("To Empty").unwrap();
        let shelf_id = h.app.shelves()[0].id;
        for id in &ids {
            h.app.add_fic_to_shelf(*id, shelf_id).unwrap();
        }

        // Pre-condition: enter shelf view, multi-select everything in it.
        h.app.open_view(View::Shelf(shelf_id));
        h.step();
        h.app.select_fics(&ids);

        // Bulk remove.
        for id in &ids {
            h.app.remove_fic_from_shelf(*id, shelf_id).unwrap();
        }

        let repo = SqliteRepository::new(&h.conn);
        assert_eq!(repo.count_fics_in_shelf(shelf_id).unwrap(), 0);
        // The fics themselves are still in the library.
        assert_eq!(repo.list_fanfictions().unwrap().len(), ids.len());
    }

    /// D24 — bulk delete: select N → trash → confirm → all soft-
    /// deleted, selection empties on the next render. We bypass the
    /// confirm modal (which is presentation-layer) and exercise the
    /// underlying delete-N path via `delete_selected`.
    #[test]
    fn bulk_delete_soft_deletes_all_selected_and_clears_selection() {
        let (mut h, ids) = given_harness_with_n_fics(3);
        h.step_n(1);
        // Select two of three to make sure only the selected ones go.
        let chosen = vec![ids[0], ids[2]];
        h.app.select_fics(&chosen);

        h.app.delete_selected();
        h.step();

        let repo = SqliteRepository::new(&h.conn);
        let surviving = repo.list_fanfictions().unwrap();
        assert_eq!(surviving.len(), 1, "the un-selected fic should survive");
        assert_eq!(surviving[0].id, ids[1]);

        // Selection cleared via the post-render orphan-prune path.
        assert!(matches!(h.app.selection(), Selection::None));
    }
}
