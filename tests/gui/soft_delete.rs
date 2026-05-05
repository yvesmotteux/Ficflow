//! Group E — soft-delete + re-add invariant.
//!
//! This is the highest-value scenario in the existing CLI e2e suite:
//! re-adding a previously-deleted fic must reset every user-owned
//! field (rating, note, read_count, last_chapter_read, status, etc.)
//! AND drop every shelf membership the original had. The infra-layer
//! `save_fanfiction` enforces this via its "reviving" branch (deletes
//! `fic_shelf` rows, then `INSERT OR REPLACE` with `deleted_at = NULL`).
//!
//! Replaces `tests/e2e.rs::test_soft_delete_revive_resets_fanfiction`
//! and `tests/e2e.rs::test_soft_delete_revive_drops_shelf_membership`.
//!
//! Note on timeouts: the worker's "reviving" path on the SECOND add
//! encounters more SQLite contention with the GUI thread's render-loop
//! reads than a fresh add does (three writes vs one), so it occasionally
//! takes a few seconds to clear. We give `wait_for_tasks` a generous
//! 9-second budget for that step.

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::{FanfictionOps, ReadingStatus, UserRating};
    use ficflow::domain::shelf::ShelfOps;
    use ficflow::infrastructure::SqliteRepository;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    /// E25 — full lifecycle: add → personalise + put on shelf → delete
    /// → re-add. The revived fic must come back with default user
    /// fields and zero shelf memberships.
    #[test]
    fn re_adding_after_delete_resets_user_fields_and_shelf_membership() {
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let mut h = GuiHarness::new(vec![mock_server.base_url()]);
        h.step_n(1);

        // ----- Add ------------------------------------------------------
        h.app.submit_add_fic(fic_id.to_string());
        assert!(h.wait_for_tasks(80), "initial add did not complete");
        assert_eq!(h.app.fics().len(), 1);

        // ----- Personalise + put on a shelf -----------------------------
        h.app.create_shelf("Beloveds").unwrap();
        let shelf_id = h.app.shelves()[0].id;
        h.app.select_fic(fic_id);
        h.app.set_status(fic_id, ReadingStatus::Read).unwrap();
        h.app
            .set_user_rating(fic_id, Some(UserRating::Five))
            .unwrap();
        h.app.set_read_count(fic_id, 7).unwrap();
        h.app
            .set_note(
                fic_id,
                Some("Personal favourite — re-read every Halloween."),
            )
            .unwrap();
        h.app.add_fic_to_shelf(fic_id, shelf_id).unwrap();

        // Sanity: every personalisation actually landed. Scope the
        // repo borrow so it doesn't conflict with the `h.step()` /
        // `wait_for_tasks` calls below (which need `&mut h`).
        {
            let repo = SqliteRepository::new(&h.conn);
            let before = repo.get_fanfiction_by_id(fic_id).unwrap();
            assert_eq!(before.reading_status, ReadingStatus::Read);
            assert_eq!(before.user_rating, Some(UserRating::Five));
            assert_eq!(before.read_count, 7);
            assert!(before.personal_note.is_some());
            assert_eq!(repo.count_fics_in_shelf(shelf_id).unwrap(), 1);
        }

        // ----- Delete (red button in the details panel) -----------------
        h.app.delete_selected();
        h.step();
        assert!(h.app.fics().is_empty());

        // ----- Re-add via the dialog ------------------------------------
        h.app.submit_add_fic(fic_id.to_string());
        assert!(
            h.wait_for_tasks(600),
            "re-add did not complete within 9s; states={:?}",
            h.app
                .task_states()
                .iter()
                .map(|t| (t.id, t.kind.clone(), t.status.clone()))
                .collect::<Vec<_>>()
        );

        // Fic is back, with EVERY user field reset and NOT on any
        // shelf — the `fic_shelf` row was wiped during revive so old
        // memberships don't ghost back into the UI.
        let repo = SqliteRepository::new(&h.conn);
        let revived = repo.get_fanfiction_by_id(fic_id).unwrap();
        assert_eq!(revived.reading_status, ReadingStatus::PlanToRead);
        assert_eq!(revived.user_rating, None);
        assert_eq!(revived.read_count, 0);
        assert!(revived.personal_note.is_none());
        assert_eq!(revived.last_chapter_read, None);
        assert_eq!(repo.count_fics_in_shelf(shelf_id).unwrap(), 0);
        assert!(repo.list_shelves_for_fic(fic_id).unwrap().is_empty());
    }
}
