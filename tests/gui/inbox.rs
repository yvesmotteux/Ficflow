#[cfg(test)]
mod tests {
    use ficflow::domain::inbox::MessageContent;
    use ficflow::interfaces::gui::View;

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    fn harness_with_outdated_fic(base_url: String) -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        let mut fic = fixtures::given_sample_fanfiction(53681185, "Outdated");
        fic.chapters_published = 18;
        fixtures::when_fanfiction_added_to_db(&conn, &fic).unwrap();
        let mut h = GuiHarness::with_db(vec![base_url], conn, db_path, td);
        h.step_n(1);
        h
    }

    fn refresh(h: &mut GuiHarness, fic_id: u64) {
        h.app.select_fic(fic_id);
        h.step();
        h.app.refresh_selected();
        assert!(h.wait_for_tasks(80), "refresh did not finish in time");
        h.step();
    }

    #[test]
    fn refreshing_a_fic_with_new_chapters_posts_an_unread_inbox_message() {
        let (server, fic_id) = fixtures::given_mock_up_to_date_ao3_server();
        let mut h = harness_with_outdated_fic(server.base_url());
        assert!(h.app.inbox_messages().is_empty());

        refresh(&mut h, fic_id);

        let messages = h.app.inbox_messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].is_unread());
        match &messages[0].content {
            MessageContent::NewChapters {
                fic_id: message_fic_id,
                previous_count,
                new_count,
                first_new_chapter_id,
                ..
            } => {
                assert_eq!(*message_fic_id, fic_id);
                assert_eq!((*previous_count, *new_count), (18, 32));
                assert_eq!(*first_new_chapter_id, Some(142624333));
            }
        }

        h.app.open_view(View::Inbox);
        h.step_n(2);
        assert_eq!(h.app.inbox_messages().len(), 1);
    }

    #[test]
    fn refreshing_an_up_to_date_fic_leaves_the_inbox_empty() {
        let (server, fic_id) = fixtures::given_mock_outdated_ao3_server();
        let mut h = harness_with_outdated_fic(server.base_url());

        refresh(&mut h, fic_id);

        assert!(h.app.inbox_messages().is_empty());
    }

    #[test]
    fn inbox_messages_can_be_read_deleted_and_cleared_and_survive_restarts() {
        let (server, fic_id) = fixtures::given_mock_up_to_date_ao3_server();
        let mut h = harness_with_outdated_fic(server.base_url());
        refresh(&mut h, fic_id);
        let first = h.app.inbox_messages()[0].id;

        let mut fic = h.app.fics()[0].clone();
        fic.chapters_published = 18;
        fixtures::when_fanfiction_added_to_db(&h.conn, &fic).unwrap();
        h.app.select_fic(fic_id);
        h.app.refresh_selected();
        assert!(
            h.wait_for_tasks(600),
            "second refresh waits out the AO3 request gap"
        );
        h.step();
        assert_eq!(h.app.inbox_messages().len(), 2);

        h.app.mark_inbox_message_read(first).unwrap();
        let unread: Vec<bool> = h
            .app
            .inbox_messages()
            .iter()
            .map(|m| m.is_unread())
            .collect();
        assert_eq!(unread, vec![true, false]);

        h.restart(vec![server.base_url()]);
        h.step();
        assert_eq!(h.app.inbox_messages().len(), 2);

        h.app.mark_all_inbox_read().unwrap();
        assert!(h.app.inbox_messages().iter().all(|m| !m.is_unread()));

        h.app.delete_inbox_message(first).unwrap();
        assert_eq!(h.app.inbox_messages().len(), 1);
        assert_ne!(h.app.inbox_messages()[0].id, first);

        h.app.clear_inbox().unwrap();
        assert!(h.app.inbox_messages().is_empty());
    }
}
