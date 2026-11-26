#[cfg(test)]
mod tests {
    use ficflow::interfaces::gui::Selection;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    fn given_harness_with_fics_updated_in_order(n: u64) -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        for id in 1..=n {
            let mut fic = fixtures::given_sample_fanfiction(id, &format!("Fic {}", id));
            fic.date_updated = format!("2025-01-0{}T12:00:00Z", id).parse().unwrap();
            fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        }
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);
        h
    }

    #[test]
    fn down_arrow_with_nothing_selected_selects_the_first_row() {
        let mut h = given_harness_with_fics_updated_in_order(3);

        h.step_with_key(egui::Key::ArrowDown);

        assert_eq!(h.app.selection(), &Selection::Single(3));
    }

    #[test]
    fn up_arrow_with_nothing_selected_selects_the_last_row() {
        let mut h = given_harness_with_fics_updated_in_order(3);

        h.step_with_key(egui::Key::ArrowUp);

        assert_eq!(h.app.selection(), &Selection::Single(1));
    }

    #[test]
    fn arrows_move_the_selection_one_row_and_stop_at_the_edges() {
        let mut h = given_harness_with_fics_updated_in_order(3);
        h.app.select_fics(&[2]);

        h.step_with_key(egui::Key::ArrowDown);
        assert_eq!(h.app.selection(), &Selection::Single(1));

        h.step_with_key(egui::Key::ArrowDown);
        assert_eq!(h.app.selection(), &Selection::Single(1));

        h.step_with_key(egui::Key::ArrowUp);
        h.step_with_key(egui::Key::ArrowUp);
        h.step_with_key(egui::Key::ArrowUp);
        assert_eq!(h.app.selection(), &Selection::Single(3));
    }

    #[test]
    fn arrows_collapse_a_multi_selection_to_a_single_row() {
        let mut h = given_harness_with_fics_updated_in_order(3);
        h.app.select_fics(&[3, 2]);

        h.step_with_key(egui::Key::ArrowDown);

        assert_eq!(h.app.selection(), &Selection::Single(1));
    }

    #[test]
    fn arrows_do_nothing_while_a_modal_is_open() {
        let mut h = given_harness_with_fics_updated_in_order(3);
        h.app.select_fics(&[2]);
        h.step_with_key(egui::Key::Delete);
        assert!(h.app.delete_fics_open());

        h.step_with_key(egui::Key::ArrowDown);

        assert_eq!(h.app.selection(), &Selection::Single(2));
    }
}
