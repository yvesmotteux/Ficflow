//! Delete key on a shelf view — routing between "remove from shelf"
//! and "delete from Ficflow".
//!
//! Pressing Delete with fics selected used to jump straight to a full
//! delete confirmation, even on a shelf. On a normal shelf it must now
//! open the remove-or-delete chooser instead. Auto-shelves (membership
//! is derived, nothing to remove from) and non-shelf library views keep
//! the plain delete-fics confirmation.

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::ReadingStatus;
    use ficflow::domain::shelf::{AutoShelfCriteria, Clause, ClauseLogic};
    use ficflow::interfaces::gui::View;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

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

    #[test]
    fn delete_key_on_normal_shelf_opens_remove_or_delete_chooser() {
        let (mut h, ids) = given_harness_with_n_fics(3);
        h.step_n(1);
        h.app.create_shelf("Beloveds").unwrap();
        let shelf_id = h.app.shelves()[0].id;
        for id in &ids {
            h.app.add_fic_to_shelf(*id, shelf_id).unwrap();
        }
        h.app.open_view(View::Shelf(shelf_id));
        h.step();
        h.app.select_fics(&ids);

        h.step_with_key(egui::Key::Delete);

        assert_eq!(h.app.remove_or_delete_shelf(), Some(shelf_id));
        assert!(!h.app.delete_fics_open());
    }

    #[test]
    fn delete_key_on_auto_shelf_falls_back_to_delete_confirm() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut fic = fixtures::given_sample_fanfiction(601, "Solo");
        fic.reading_status = ReadingStatus::Read;
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        let criteria = AutoShelfCriteria {
            logic: ClauseLogic::Or,
            clauses: vec![Clause::Status(ReadingStatus::Read)],
        };
        h.app.upsert_auto_shelf(None, "Read", criteria).unwrap();
        let shelf_id = h.app.shelves()[0].id;
        h.app.open_view(View::Shelf(shelf_id));
        h.step();
        h.app.select_fics(&[601]);

        h.step_with_key(egui::Key::Delete);

        assert!(h.app.delete_fics_open());
        assert_eq!(h.app.remove_or_delete_shelf(), None);
    }

    #[test]
    fn delete_key_on_all_fics_view_opens_delete_confirm() {
        let (mut h, ids) = given_harness_with_n_fics(2);
        h.step_n(1);
        assert!(matches!(h.app.current_view(), View::AllFics));
        h.app.select_fics(&ids);

        h.step_with_key(egui::Key::Delete);

        assert!(h.app.delete_fics_open());
        assert_eq!(h.app.remove_or_delete_shelf(), None);
    }
}
