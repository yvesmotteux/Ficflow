#[cfg(test)]
mod tests {
    use chrono::Utc;
    use ficflow::domain::fanfiction::ReadingStatus;
    use ficflow::domain::series::{Series, SeriesOps, SeriesWork};
    use ficflow::infrastructure::persistence::repository::SqliteRepository;
    use ficflow::interfaces::gui::{ColumnKey, RowKey, Selection, SortDirection, View};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    use RowKey::{Fic, Missing, Series as Group};

    fn series(id: u64, works: &[(u64, u32)]) -> Series {
        Series {
            id,
            title: format!("Series {id}"),
            authors: vec!["Author".to_string()],
            description: Some("All the parts".to_string()),
            notes: None,
            date_begun: None,
            date_updated: None,
            words: 5000,
            works_count: works.len() as u32,
            complete: false,
            bookmarks: 0,
            last_checked_date: Utc::now(),
            works: works
                .iter()
                .map(|&(work_id, part)| SeriesWork {
                    work_id,
                    part,
                    title: format!("Part {part}"),
                    authors: vec!["Author".to_string()],
                })
                .collect(),
        }
    }

    fn given_library_with_series() -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        for id in 1..=5 {
            let fic = fixtures::given_sample_fanfiction(id, &format!("Fic {id}"));
            fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        }
        let repo = SqliteRepository::new(&conn);
        repo.save_series(&series(10, &[(1, 1), (2, 2), (99, 3), (3, 4)]))
            .unwrap();
        repo.save_series(&series(20, &[(1, 1), (5, 2)])).unwrap();
        let mut h = GuiHarness::with_db(vec!["http://127.0.0.1:1".into()], conn, db_path, td);
        h.app.set_sort(ColumnKey::Title, SortDirection::Ascending);
        h.step_n(1);
        h
    }

    const MISSING_PART: RowKey = Missing {
        series_id: 10,
        work_id: 99,
    };

    #[test]
    fn series_are_grouped_with_their_missing_parts_and_fics_in_two_series_show_twice() {
        let h = given_library_with_series();

        assert_eq!(
            h.app.library_rows(),
            vec![
                Group(10),
                Fic(1),
                Fic(2),
                MISSING_PART,
                Fic(3),
                Group(20),
                Fic(1),
                Fic(5),
                Fic(4),
            ]
        );
        assert_eq!(h.app.visible_ids(), vec![1, 2, 3, 5, 4]);
    }

    #[test]
    fn collapsed_series_stay_collapsed_after_a_restart() {
        let mut h = given_library_with_series();
        h.app.select_row(Group(10));

        h.step_with_key(egui::Key::ArrowLeft);
        h.step();

        assert_eq!(
            h.app.library_rows(),
            vec![Group(10), Group(20), Fic(1), Fic(5), Fic(4)]
        );
        h.restart(vec!["http://127.0.0.1:1".into()]);
        assert_eq!(h.app.collapsed_series(), &[10]);

        h.app.select_row(Group(10));
        h.step_with_key(egui::Key::ArrowRight);
        assert!(h.app.collapsed_series().is_empty());
    }

    #[test]
    fn arrow_keys_walk_through_series_and_missing_parts() {
        let mut h = given_library_with_series();

        h.step_with_key(egui::Key::ArrowDown);
        assert_eq!(h.app.selection(), &Selection::Series(10));
        for _ in 0..3 {
            h.step_with_key(egui::Key::ArrowDown);
        }
        assert_eq!(
            h.app.selection(),
            &Selection::Missing {
                series_id: 10,
                work_id: 99
            }
        );
        assert!(h.app.selection_shelves().is_empty());
    }

    #[test]
    fn search_groups_a_series_only_when_several_parts_or_its_title_match() {
        let mut h = given_library_with_series();

        h.app.set_search("fic 2");
        assert_eq!(h.app.library_rows(), vec![Fic(2)]);

        h.app.set_search("series 20");
        assert_eq!(h.app.library_rows(), vec![Group(20), Fic(1), Fic(5)]);

        h.app.set_search("fic");
        assert_eq!(
            h.app.library_rows()[..4],
            [Group(10), Fic(1), Fic(2), Fic(3)]
        );
    }

    #[test]
    fn status_views_group_matching_parts_without_placeholders() {
        let mut h = given_library_with_series();
        h.app.set_status(1, ReadingStatus::Read).unwrap();
        h.app.set_status(2, ReadingStatus::Read).unwrap();

        h.app.open_view(View::ByStatus(ReadingStatus::Read));

        assert_eq!(
            h.app.library_rows(),
            vec![Group(10), Fic(1), Fic(2), Group(20), Fic(1)]
        );
    }

    #[test]
    fn shelves_group_a_series_only_when_it_holds_all_of_its_fics() {
        let mut h = given_library_with_series();
        h.app.create_shelf("Shelf").unwrap();
        let shelf = h.app.shelves()[0].id;
        for id in [1, 2, 3] {
            h.app.add_fic_to_shelf(id, shelf).unwrap();
        }

        h.app.open_view(View::Shelf(shelf));

        assert_eq!(
            h.app.library_rows(),
            vec![Group(10), Fic(1), Fic(2), MISSING_PART, Fic(3)]
        );
    }
}
