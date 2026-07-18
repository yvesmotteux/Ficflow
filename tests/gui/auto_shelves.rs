//! Auto-shelves: live-computed membership, never backed by `fic_shelf`.

#[cfg(test)]
mod tests {
    use ficflow::domain::fanfiction::ReadingStatus;
    use ficflow::domain::shelf::ShelfOps;
    use ficflow::domain::shelf::{AutoShelfCriteria, Clause, ClauseLogic};
    use ficflow::infrastructure::SqliteRepository;
    use ficflow::interfaces::gui::View;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    #[test]
    fn auto_shelf_membership_and_count_track_matching_fics_with_no_fic_shelf_rows() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut alpha = fixtures::given_sample_fanfiction(601, "Alpha");
        alpha.fandoms = vec!["Star Wars".to_string()];
        let mut beta = fixtures::given_sample_fanfiction(602, "Beta");
        beta.fandoms = vec!["Star Trek".to_string()];
        let mut gamma = fixtures::given_sample_fanfiction(603, "Gamma");
        gamma.fandoms = vec!["Star Trek".to_string()];
        gamma.reading_status = ReadingStatus::Read;
        fixtures::when_fanfiction_added_to_db(&conn, &alpha).unwrap();
        fixtures::when_fanfiction_added_to_db(&conn, &beta).unwrap();
        fixtures::when_fanfiction_added_to_db(&conn, &gamma).unwrap();

        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        // "Star Trek fics OR Read fics" — Beta matches via fandom, Gamma
        // matches via both, Alpha matches neither.
        let criteria = AutoShelfCriteria {
            logic: ClauseLogic::Or,
            clauses: vec![
                Clause::Fandom("Star Trek".to_string()),
                Clause::Status(ReadingStatus::Read),
            ],
        };
        h.app
            .upsert_auto_shelf(None, "Trek & Read", criteria)
            .expect("valid auto-shelf");
        let shelf_id = h.app.shelves()[0].id;

        assert_eq!(h.app.shelf_count(shelf_id), 2);

        h.app.open_view(View::Shelf(shelf_id));
        h.step();
        let mut visible = h.app.visible_ids();
        visible.sort();
        assert_eq!(visible, vec![602, 603]);

        // No fic_shelf rows were ever written for this shelf.
        let repo = SqliteRepository::new(&h.conn);
        assert!(repo.list_fics_in_shelf(shelf_id).unwrap().is_empty());

        // A second auto-shelf whose match depends solely on status, so a
        // plain `set_status` call (the only field-editing method the
        // harness can drive without a fic re-fetch) can flip it out of
        // membership and prove the cache reacts on the next mutation,
        // same as normal-shelf membership does.
        let status_only = AutoShelfCriteria {
            logic: ClauseLogic::And,
            clauses: vec![Clause::Status(ReadingStatus::Read)],
        };
        h.app
            .upsert_auto_shelf(None, "Read", status_only)
            .expect("valid auto-shelf");
        let status_shelf_id = h
            .app
            .shelves()
            .iter()
            .find(|s| s.name == "Read")
            .unwrap()
            .id;
        assert_eq!(h.app.shelf_count(status_shelf_id), 1);

        h.app
            .set_status(gamma.id, ReadingStatus::PlanToRead)
            .unwrap();

        assert_eq!(h.app.shelf_count(status_shelf_id), 0);
        h.app.open_view(View::Shelf(status_shelf_id));
        h.step();
        assert!(h.app.visible_ids().is_empty());
    }

    #[test]
    fn add_fic_to_shelf_rejects_auto_shelf() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(701, "Solo");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        let criteria = AutoShelfCriteria {
            logic: ClauseLogic::And,
            clauses: vec![Clause::Tag("Tag 1".to_string())],
        };
        h.app.upsert_auto_shelf(None, "Auto", criteria).unwrap();
        let shelf_id = h.app.shelves()[0].id;

        let repo = SqliteRepository::new(&h.conn);
        let err = repo.add_fic_to_shelf(701, shelf_id).unwrap_err();
        assert!(matches!(err, ficflow::error::FicflowError::InvalidInput(_)));
    }

    #[test]
    fn nesting_a_shelf_under_an_auto_shelf_is_rejected() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        let criteria = AutoShelfCriteria {
            logic: ClauseLogic::And,
            clauses: vec![Clause::Tag("Tag 1".to_string())],
        };
        h.app.upsert_auto_shelf(None, "Auto", criteria).unwrap();
        let auto_id = h.app.shelves()[0].id;

        let repo = SqliteRepository::new(&h.conn);
        let err = repo.create_shelf("Child", Some(auto_id)).unwrap_err();
        assert!(matches!(err, ficflow::error::FicflowError::InvalidInput(_)));
    }

    #[test]
    fn auto_shelf_criteria_round_trips_through_reload() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        let criteria = AutoShelfCriteria {
            logic: ClauseLogic::Or,
            clauses: vec![
                Clause::Fandom("Star Trek".to_string()),
                Clause::Author("Test Author".to_string()),
            ],
        };
        h.app
            .upsert_auto_shelf(None, "Round Trip", criteria.clone())
            .unwrap();
        let shelf_id = h.app.shelves()[0].id;

        let repo = SqliteRepository::new(&h.conn);
        let reloaded = repo.get_shelf_by_id(shelf_id).unwrap();
        match reloaded.kind {
            ficflow::domain::shelf::ShelfKind::Auto(loaded) => assert_eq!(loaded, criteria),
            ficflow::domain::shelf::ShelfKind::Normal => panic!("expected an auto-shelf"),
        }
    }

    #[test]
    fn editing_auto_shelf_criteria_updates_persisted_criteria_and_live_membership() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut wars = fixtures::given_sample_fanfiction(801, "Wars Fic");
        wars.fandoms = vec!["Star Wars".to_string()];
        let mut trek = fixtures::given_sample_fanfiction(802, "Trek Fic");
        trek.fandoms = vec!["Star Trek".to_string()];
        fixtures::when_fanfiction_added_to_db(&conn, &wars).unwrap();
        fixtures::when_fanfiction_added_to_db(&conn, &trek).unwrap();

        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        let initial = AutoShelfCriteria {
            logic: ClauseLogic::Or,
            clauses: vec![Clause::Fandom("Star Wars".to_string())],
        };
        h.app.upsert_auto_shelf(None, "Editable", initial).unwrap();
        let shelf_id = h.app.shelves()[0].id;
        assert_eq!(h.app.shelf_count(shelf_id), 1);

        // The single upsert also lets the same edit rename the shelf.
        let updated = AutoShelfCriteria {
            logic: ClauseLogic::Or,
            clauses: vec![Clause::Fandom("Star Trek".to_string())],
        };
        h.app
            .upsert_auto_shelf(Some(shelf_id), "Renamed", updated.clone())
            .unwrap();

        assert_eq!(h.app.shelf_count(shelf_id), 1);
        h.app.open_view(View::Shelf(shelf_id));
        h.step();
        assert_eq!(h.app.visible_ids(), vec![802]);

        let repo = SqliteRepository::new(&h.conn);
        let reloaded = repo.get_shelf_by_id(shelf_id).unwrap();
        assert_eq!(reloaded.name, "Renamed");
        match reloaded.kind {
            ficflow::domain::shelf::ShelfKind::Auto(loaded) => assert_eq!(loaded, updated),
            ficflow::domain::shelf::ShelfKind::Normal => panic!("expected an auto-shelf"),
        }
    }

    #[test]
    fn blank_name_defaults_to_unnamed_instead_of_blocking_creation() {
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.step_n(1);

        let criteria = AutoShelfCriteria {
            logic: ClauseLogic::And,
            clauses: vec![Clause::Tag("Tag 1".to_string())],
        };
        h.app
            .upsert_auto_shelf(None, "   ", criteria)
            .expect("blank name should not block creation");
        assert_eq!(h.app.shelves()[0].name, "Unnamed");
    }
}
