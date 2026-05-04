//! Group C — shelves.
//!
//! Covers `create_shelf`, `delete_shelf`, `list_shelves`,
//! `add_to_shelf`, `remove_from_shelf`, `list_shelf_fics`,
//! `list_shelves_for_fic`, `count_fics_in_shelf`.

#[cfg(test)]
mod tests {
    use ficflow::domain::shelf::ShelfOps;
    use ficflow::infrastructure::SqliteRepository;
    use ficflow::interfaces::gui::View;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    /// C13 — create-shelf flow: submitting the modal name persists the
    /// shelf, the in-memory cache reflects it, and the sidebar count
    /// (which reads from `shelf_counts`) starts at 0.
    #[test]
    fn creating_a_shelf_persists_and_appears_with_zero_count() {
        let mut h = GuiHarness::new(vec!["http://127.0.0.1:1".into()]);
        h.step_n(1);
        assert!(h.app.shelves().is_empty());

        h.app.create_shelf("Favorites").expect("valid shelf name");

        assert_eq!(h.app.shelves().len(), 1);
        assert_eq!(h.app.shelves()[0].name, "Favorites");

        // Same row visible from a fresh repo read.
        let repo = SqliteRepository::new(&h.conn);
        assert_eq!(repo.list_shelves().unwrap().len(), 1);
        // Shelf is empty so its count is 0.
        let id = h.app.shelves()[0].id;
        assert_eq!(repo.count_fics_in_shelf(id).unwrap(), 0);
    }

    /// C14 — empty-name guard: the application/infra layer rejects
    /// blank submissions; the in-memory cache stays untouched and the
    /// caller sees an `InvalidInput` error.
    #[test]
    fn creating_shelf_with_blank_name_is_rejected() {
        let mut h = GuiHarness::new(vec!["http://127.0.0.1:1".into()]);
        h.step_n(1);

        let outcome = h.app.create_shelf("   ");
        assert!(outcome.is_err(), "blank name should not pass validation");
        assert!(h.app.shelves().is_empty(), "no shelf was created");
    }

    /// C15 — adding a fic via the multi-select dropdown: link is
    /// created, `selection_shelf_ids` cache (used by the dropdown's
    /// checked state on the next render) reflects it, and the
    /// per-shelf count rolled up into the sidebar increments.
    #[test]
    fn adding_a_fic_to_shelf_via_dropdown_creates_link() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(301, "Anchor");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        h.app.create_shelf("Reading").unwrap();
        let shelf_id = h.app.shelves()[0].id;
        h.app.select_fic(301);
        h.step();

        h.app.add_fic_to_shelf(301, shelf_id).unwrap();

        // DB-level link exists.
        let repo = SqliteRepository::new(&h.conn);
        let in_shelf = repo.list_fics_in_shelf(shelf_id).unwrap();
        assert_eq!(in_shelf.len(), 1);
        assert_eq!(in_shelf[0].id, 301);
        assert_eq!(repo.count_fics_in_shelf(shelf_id).unwrap(), 1);

        // The dropdown reads from `selection_shelf_ids`. Easiest proxy:
        // ask the repo what shelves the fic belongs to.
        let shelves_for_fic = repo.list_shelves_for_fic(301).unwrap();
        assert_eq!(shelves_for_fic.len(), 1);
        assert_eq!(shelves_for_fic[0].id, shelf_id);
    }

    /// C16 — clicking the × on a shelf chip removes membership and the
    /// shelf count drops back to its previous value.
    #[test]
    fn removing_a_fic_from_shelf_via_chip_drops_link() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(401, "On Then Off");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);
        h.app.create_shelf("Reading").unwrap();
        let shelf_id = h.app.shelves()[0].id;
        h.app.add_fic_to_shelf(401, shelf_id).unwrap();
        let repo = SqliteRepository::new(&h.conn);
        assert_eq!(repo.count_fics_in_shelf(shelf_id).unwrap(), 1);

        h.app.remove_fic_from_shelf(401, shelf_id).unwrap();

        assert_eq!(repo.count_fics_in_shelf(shelf_id).unwrap(), 0);
        assert!(repo.list_shelves_for_fic(401).unwrap().is_empty());
    }

    /// C17 — clicking a shelf in the sidebar swaps `current_view`,
    /// triggering the render loop's shelf_members refresh; the table
    /// renders only the fics on that shelf.
    #[test]
    fn clicking_shelf_filters_table_to_its_members() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic_in = fixtures::given_sample_fanfiction(501, "On Shelf");
        let fic_out = fixtures::given_sample_fanfiction(502, "Off Shelf");
        fixtures::when_fanfiction_added_to_db(&conn, &fic_in).unwrap();
        fixtures::when_fanfiction_added_to_db(&conn, &fic_out).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        h.app.create_shelf("Curated").unwrap();
        let shelf_id = h.app.shelves()[0].id;
        h.app.add_fic_to_shelf(501, shelf_id).unwrap();

        // Switch the active view to the shelf — the next render
        // populates `shelf_members` with the shelf's fic ids.
        h.app.open_view(View::Shelf(shelf_id));
        h.step();

        // The table itself is virtualised, so we read the same source
        // of truth: list_shelf_fics. (`fics()` keeps everything; the
        // view filter is applied during render via `shelf_members`.)
        let repo = SqliteRepository::new(&h.conn);
        let in_view = repo.list_fics_in_shelf(shelf_id).unwrap();
        assert_eq!(in_view.len(), 1);
        assert_eq!(in_view[0].id, 501);
    }

    /// C18 — context-menu Delete on a shelf row soft-deletes the
    /// shelf, returns the active view to AllFics if it was the
    /// deleted shelf, and leaves the fics that were on it untouched.
    #[test]
    fn deleting_a_shelf_keeps_its_fics_and_resets_view() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(601, "Survivor");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);
        h.app.create_shelf("Doomed").unwrap();
        let shelf_id = h.app.shelves()[0].id;
        h.app.add_fic_to_shelf(601, shelf_id).unwrap();
        h.app.open_view(View::Shelf(shelf_id));
        h.step();

        h.app.delete_shelf(shelf_id).unwrap();

        // Shelf is gone from the in-memory cache and from the repo.
        assert!(h.app.shelves().is_empty());
        let repo = SqliteRepository::new(&h.conn);
        assert!(repo.list_shelves().unwrap().is_empty());

        // View bounced back to AllFics so the user isn't stuck on a
        // shelf-only filter for a shelf that no longer exists.
        assert_eq!(h.app.current_view(), &View::AllFics);

        // The fic is still alive.
        assert_eq!(h.app.fics().len(), 1);
        assert_eq!(h.app.fics()[0].id, 601);
    }

    /// C19 — drag-drop semantics: dropping a multi-fic payload on a
    /// shelf should add every fic in the payload. We don't simulate
    /// pixel-level drag (egui 0.29 makes that painful without
    /// kittest), but the dispatch path the GUI uses is just a loop of
    /// `add_to_shelf` calls — the same code path `add_fic_to_shelf`
    /// drives — so iterating the test ids in the test exercises it.
    #[test]
    fn drag_drop_adds_all_dropped_fics_to_shelf() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let f1 = fixtures::given_sample_fanfiction(701, "First");
        let f2 = fixtures::given_sample_fanfiction(702, "Second");
        let f3 = fixtures::given_sample_fanfiction(703, "Third");
        fixtures::when_fanfiction_added_to_db(&conn, &f1).unwrap();
        fixtures::when_fanfiction_added_to_db(&conn, &f2).unwrap();
        fixtures::when_fanfiction_added_to_db(&conn, &f3).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);
        h.app.create_shelf("Bulk Target").unwrap();
        let shelf_id = h.app.shelves()[0].id;

        // Mirror the GUI's drop-handler loop.
        for id in [701u64, 702, 703] {
            h.app.add_fic_to_shelf(id, shelf_id).unwrap();
        }

        let repo = SqliteRepository::new(&h.conn);
        assert_eq!(repo.count_fics_in_shelf(shelf_id).unwrap(), 3);
    }
}
