//! Group G — remembering the last opened tab/shelf across restarts.

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::ReadingStatus;
    use ficflow::interfaces::gui::View;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    fn given_harness() -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td)
    }

    /// G1 — switching to a shelf tab and restarting reopens that shelf.
    #[test]
    fn restart_reopens_last_shelf_view() {
        let mut h = given_harness();
        h.app.create_shelf("Favorites").unwrap();
        let shelf_id = h.app.shelves()[0].id;

        h.app.open_view(View::Shelf(shelf_id));
        h.restart(vec!["http://127.0.0.1:1".into()]);

        assert_eq!(*h.app.current_view(), View::Shelf(shelf_id));
    }

    /// G2 — same for a status tab.
    #[test]
    fn restart_reopens_last_status_view() {
        let mut h = given_harness();

        h.app.open_view(View::ByStatus(ReadingStatus::Read));
        h.restart(vec!["http://127.0.0.1:1".into()]);

        assert_eq!(*h.app.current_view(), View::ByStatus(ReadingStatus::Read));
    }

    /// G3 — Tasks/Settings aren't persisted: quitting from one of them
    /// keeps whatever library tab was open before.
    #[test]
    fn tasks_and_settings_are_not_persisted() {
        let mut h = given_harness();
        h.app.create_shelf("Favorites").unwrap();
        let shelf_id = h.app.shelves()[0].id;

        h.app.open_view(View::Shelf(shelf_id));
        h.app.open_view(View::Settings);
        h.restart(vec!["http://127.0.0.1:1".into()]);

        assert_eq!(*h.app.current_view(), View::Shelf(shelf_id));
    }

    /// G4 — if the persisted shelf was deleted in the meantime, startup
    /// falls back to the all-fics view instead of a dangling reference.
    #[test]
    fn restart_falls_back_to_all_fics_when_shelf_was_deleted() {
        let mut h = given_harness();
        h.app.create_shelf("Doomed").unwrap();
        let shelf_id = h.app.shelves()[0].id;

        h.app.open_view(View::Shelf(shelf_id));
        h.app.delete_shelf(shelf_id).unwrap();
        h.restart(vec!["http://127.0.0.1:1".into()]);

        assert_eq!(*h.app.current_view(), View::AllFics);
    }

    /// G5 — with no prior session (fresh config), startup defaults to
    /// the all-fics view, same as before this feature existed.
    #[test]
    fn fresh_config_defaults_to_all_fics() {
        let h = given_harness();
        assert_eq!(*h.app.current_view(), View::AllFics);
    }
}
