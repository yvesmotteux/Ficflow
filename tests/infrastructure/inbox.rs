use crate::common::fixtures;

use ficflow::application::{
    clear_inbox::clear_inbox,
    delete_inbox_message::delete_inbox_message,
    list_inbox::list_inbox,
    mark_inbox_read::{mark_all_read, mark_message_read},
};
use ficflow::domain::inbox::{InboxOps, MessageContent};
use ficflow::infrastructure::persistence::repository::SqliteRepository;

fn new_chapters(fic_id: u64, title: &str) -> MessageContent {
    MessageContent::NewChapters {
        fic_id,
        title: title.to_string(),
        authors: vec!["Author".to_string()],
        fandoms: vec!["Fandom".to_string()],
        previous_count: 3,
        new_count: 4,
        first_new_chapter_id: Some(1000 + fic_id),
    }
}

#[test]
fn posted_messages_are_listed_newest_first_and_unread() {
    let (conn, _path, _temp_dir) = fixtures::given_test_database();
    let repo = SqliteRepository::new(&conn);

    let first = repo.post_message(&new_chapters(1, "First")).unwrap();
    let second = repo.post_message(&new_chapters(2, "Second")).unwrap();

    let messages = list_inbox(&repo).unwrap();
    assert_eq!(messages, vec![second, first]);
    assert!(messages.iter().all(|m| m.is_unread()));
    assert_eq!(messages[1].content, new_chapters(1, "First"));
}

#[test]
fn messages_can_be_marked_read_one_by_one_or_all_at_once() {
    let (conn, _path, _temp_dir) = fixtures::given_test_database();
    let repo = SqliteRepository::new(&conn);
    let first = repo.post_message(&new_chapters(1, "First")).unwrap();
    repo.post_message(&new_chapters(2, "Second")).unwrap();
    repo.post_message(&new_chapters(3, "Third")).unwrap();

    mark_message_read(&repo, first.id).unwrap();
    let unread: Vec<u64> = list_inbox(&repo)
        .unwrap()
        .into_iter()
        .filter(|m| m.is_unread())
        .filter_map(|m| m.content.fic_id())
        .collect();
    assert_eq!(unread, vec![3, 2]);

    mark_all_read(&repo).unwrap();
    assert!(list_inbox(&repo).unwrap().iter().all(|m| !m.is_unread()));
}

#[test]
fn deleting_and_clearing_remove_messages_for_good() {
    let (conn, _path, _temp_dir) = fixtures::given_test_database();
    let repo = SqliteRepository::new(&conn);
    let first = repo.post_message(&new_chapters(1, "First")).unwrap();
    repo.post_message(&new_chapters(2, "Second")).unwrap();

    delete_inbox_message(&repo, first.id).unwrap();
    let remaining = list_inbox(&repo).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].content.fic_id(), Some(2));

    clear_inbox(&repo).unwrap();
    assert!(list_inbox(&repo).unwrap().is_empty());
    let rows: i64 = conn
        .query_row("SELECT COUNT(*) FROM inbox_message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}
