use chrono::Utc;

use crate::common::fixtures;
use ficflow::domain::fanfiction::FanfictionOps;
use ficflow::domain::series::{Series, SeriesOps, SeriesRef, SeriesWork};
use ficflow::error::FicflowError;
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
