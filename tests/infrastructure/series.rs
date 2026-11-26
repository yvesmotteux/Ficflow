use std::fs;
use std::time::Duration;

use chrono::Utc;
use httpmock::prelude::*;

use crate::common::fixtures;
use ficflow::application::add_fic::add_fanfiction;
use ficflow::domain::fanfiction::FanfictionOps;
use ficflow::domain::series::{Series, SeriesOps, SeriesRef, SeriesWork};
use ficflow::error::FicflowError;
use ficflow::infrastructure::external::ao3::Ao3Fetcher;
use ficflow::infrastructure::persistence::repository::SqliteRepository;

fn work(work_id: u64, part: u32) -> SeriesWork {
    SeriesWork {
        work_id,
        part,
        title: format!("Part {part}"),
        authors: vec!["Author".to_string()],
    }
}

fn series(id: u64, works: Vec<SeriesWork>) -> Series {
    Series {
        id,
        title: format!("Series {id}"),
        authors: vec!["Author".to_string()],
        description: Some("About".to_string()),
        notes: None,
        date_begun: Some(Utc::now()),
        date_updated: None,
        words: 1000,
        works_count: works.len() as u32,
        complete: false,
        bookmarks: 2,
        last_checked_date: Utc::now(),
        works,
    }
}

fn fic_in(id: u64, series_id: u64, part: u32) -> ficflow::domain::fanfiction::Fanfiction {
    let mut fic = fixtures::given_sample_fanfiction(id, &format!("Fic {id}"));
    fic.series = vec![SeriesRef {
        id: series_id,
        title: format!("Series {series_id}"),
        part,
    }];
    fic
}

#[test]
fn a_series_lives_as_long_as_one_of_its_fics() {
    let (conn, _path, _temp_dir) = fixtures::given_test_database();
    let repo = SqliteRepository::new(&conn);
    let (first, second) = (fic_in(1, 10, 1), fic_in(3, 10, 3));
    repo.save_fanfiction(&first).unwrap();
    repo.save_fanfiction(&second).unwrap();
    repo.save_series(&series(10, vec![work(3, 3), work(1, 1), work(2, 2)]))
        .unwrap();

    let listed = repo.list_series().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].description.as_deref(), Some("About"));
    let parts: Vec<u64> = listed[0].works.iter().map(|w| w.work_id).collect();
    assert_eq!(parts, vec![1, 2, 3]);

    repo.delete_fanfiction(1).unwrap();
    assert_eq!(repo.get_series(10).unwrap().id, 10);

    repo.delete_fanfiction(3).unwrap();
    assert!(matches!(
        repo.get_series(10),
        Err(FicflowError::SeriesNotFound { series_id: 10 })
    ));
    assert!(repo.list_series().unwrap().is_empty());

    repo.save_fanfiction(&first).unwrap();
    repo.save_series(&series(10, vec![work(1, 1), work(2, 2), work(3, 3)]))
        .unwrap();
    assert_eq!(repo.get_series(10).unwrap().works.len(), 3);
}

#[test]
fn a_fic_hidden_from_the_series_listing_stays_linked() {
    let (conn, _path, _temp_dir) = fixtures::given_test_database();
    let repo = SqliteRepository::new(&conn);
    repo.save_series(&series(10, vec![work(1, 1)])).unwrap();
    let hidden = fic_in(2, 10, 2);
    repo.save_fanfiction(&hidden).unwrap();
    repo.link_fic_to_series(&hidden).unwrap();

    repo.save_series(&series(10, vec![work(1, 1)])).unwrap();
    let works = repo.get_series(10).unwrap().works;
    assert_eq!(
        works.iter().map(|w| w.work_id).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(works[1].title, "Fic 2");

    let mut left = hidden.clone();
    left.series.clear();
    repo.link_fic_to_series(&left).unwrap();
    assert!(repo.get_series(10).is_err());
}

#[test]
fn adding_a_fic_stores_its_series() {
    let server = MockServer::start();
    let work = fs::read_to_string("tests/fixtures/ao3_fic_anonymous.html")
        .unwrap()
        .replace("/series/5747376", "/series/6653231");
    server.mock(|when, then| {
        when.method(GET).path("/works/85157066");
        then.status(200).body(work);
    });
    let (first, second) = (
        fs::read_to_string("tests/fixtures/lists/series_p1.html").unwrap(),
        fs::read_to_string("tests/fixtures/lists/series_p2.html").unwrap(),
    );
    server.mock(|when, then| {
        when.method(GET)
            .path("/series/6653231")
            .query_param("page", "2");
        then.status(200).body(second);
    });
    let series_page = server.mock(|when, then| {
        when.method(GET)
            .path("/series/6653231")
            .query_param_missing("page");
        then.status(200).body(first);
    });
    let fetcher =
        Ao3Fetcher::with_min_gap(vec![server.base_url()], 1, Duration::ZERO, Duration::ZERO)
            .unwrap();
    let (conn, _path, _temp_dir) = fixtures::given_test_database();
    let repo = SqliteRepository::new(&conn);

    add_fanfiction(&fetcher, &repo, &repo, 85157066).unwrap();
    add_fanfiction(&fetcher, &repo, &repo, 85157066).unwrap_err();

    let series = repo.get_series(6653231).unwrap();
    assert_eq!(series.title, "Arslan-tober 2026: Tiny Tales");
    assert_eq!(series.works.len(), 32);
    assert!(
        series
            .works
            .iter()
            .any(|w| w.work_id == 85157066 && w.part == 3)
    );
    series_page.assert_calls(1);

    repo.delete_fanfiction(85157066).unwrap();
    assert!(repo.list_series().unwrap().is_empty());
}
