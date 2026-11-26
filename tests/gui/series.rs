#[cfg(test)]
mod tests {
    use chrono::Utc;
    use ficflow::domain::fanfiction::ReadingStatus;
    use ficflow::domain::series::{Series, SeriesOps, SeriesWork};
    use ficflow::infrastructure::persistence::repository::SqliteRepository;
    use ficflow::interfaces::gui::{ColumnKey, RowKey, Selection, SortDirection, View};
    use httpmock::Method::GET;
    use httpmock::MockServer;

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
        given_library_with_series_on("http://127.0.0.1:1".into())
    }

    fn given_library_with_series_on(ao3_url: String) -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        for id in 1..=5 {
            let fic = fixtures::given_sample_fanfiction(id, &format!("Fic {id}"));
            fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        }
        let repo = SqliteRepository::new(&conn);
        repo.save_series(&series(10, &[(1, 1), (2, 2), (99, 3), (3, 4)]))
            .unwrap();
        repo.save_series(&series(20, &[(1, 1), (5, 2)])).unwrap();
        let mut h = GuiHarness::with_db(vec![ao3_url], conn, db_path, td);
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

    fn fixture(path: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{path}.html")).unwrap()
    }

    #[test]
    fn delete_on_a_series_deletes_its_fics_and_the_series_goes_away() {
        let mut h = given_library_with_series();
        h.app.select_row(Group(20));
        assert!(h.app.details_panel_visible());

        h.step_with_key(egui::Key::Delete);
        assert!(h.app.delete_fics_open());
        h.app.delete_selected();
        h.step();

        assert_eq!(
            h.app.library_rows(),
            vec![
                Group(10),
                Missing {
                    series_id: 10,
                    work_id: 1
                },
                Fic(2),
                MISSING_PART,
                Fic(3),
                Fic(4)
            ]
        );
        assert_eq!(h.app.selection(), &Selection::None);
        h.restart(vec!["http://127.0.0.1:1".into()]);
        assert!(!h.app.library_rows().contains(&Group(20)));
    }

    #[test]
    fn adding_a_missing_part_fills_its_place_in_the_series() {
        let server = MockServer::start();
        let work = fixture("ao3_fic_anonymous").replace("/series/5747376", "/series/10");
        server.mock(|when, then| {
            when.method(GET).path("/works/99");
            then.status(200).body(work);
        });
        let mut h = given_library_with_series_on(server.base_url());
        h.app.select_row(MISSING_PART);

        h.app.add_series_parts(vec![99]);
        assert!(h.wait_for_tasks(400));
        h.step_n(2);

        assert_eq!(
            h.app.library_rows()[..5],
            [Group(10), Fic(1), Fic(2), Fic(99), Fic(3)]
        );
        assert_eq!(h.app.selection(), &Selection::Single(99));
    }

    #[test]
    fn refreshing_a_series_picks_up_its_new_parts() {
        let server = MockServer::start();
        for (page, name) in [(None, "series_p1"), (Some("2"), "series_p2")] {
            let body = fixture(&format!("lists/{name}"));
            server.mock(|when, then| {
                let when = when.method(GET).path("/series/6653231");
                match page {
                    Some(page) => when.query_param("page", page),
                    None => when.query_param_missing("page"),
                };
                then.status(200).body(body);
            });
        }
        let (conn, db_path, td) = fixtures::given_test_database();
        let fic = fixtures::given_sample_fanfiction(93626551, "A Matter of Perspective");
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let mut stale = series(6653231, &[(93626551, 1)]);
        stale.title = "Old title".to_string();
        SqliteRepository::new(&conn).save_series(&stale).unwrap();
        let mut h = GuiHarness::with_db(vec![server.base_url()], conn, db_path, td);
        h.step();

        h.app.refresh_series(6653231);
        assert!(h.wait_for_tasks(400));
        h.step_n(2);

        let rows = h.app.library_rows();
        assert_eq!(rows.len(), 32);
        assert_eq!(rows[..2], [Group(6653231), Fic(93626551)]);
        let repo = SqliteRepository::new(&h.conn);
        assert_eq!(
            repo.get_series(6653231).unwrap().title,
            "Arslan-tober 2026: Tiny Tales"
        );
    }
}
