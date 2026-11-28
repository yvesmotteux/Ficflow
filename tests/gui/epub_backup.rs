#[cfg(test)]
mod tests {
    use httpmock::Method::GET;
    use httpmock::{Mock, MockServer};

    use ficflow::infrastructure::external::ao3::session::Ao3Session;
    use ficflow::interfaces::gui::{TaskKind, TaskState, TaskStatus};

    use crate::harness::GuiHarness;

    const FIC_ID: u64 = 53960491;
    const EPUB_PATH: &str = "/downloads/53960491/Featherlight.epub";

    fn given_work(server: &MockServer) {
        let page = std::fs::read_to_string("tests/fixtures/ao3_fic_example1.html").unwrap();
        server.mock(|when, then| {
            when.method(GET).path(format!("/works/{FIC_ID}"));
            then.status(200).body(page);
        });
    }

    fn given_epub<'a>(server: &'a MockServer, bytes: &'static [u8]) -> Mock<'a> {
        server.mock(|when, then| {
            when.method(GET).path(EPUB_PATH);
            then.status(200)
                .header("content-type", "application/epub+zip")
                .body(bytes);
        })
    }

    fn given_harness(server: &MockServer) -> GuiHarness {
        let mut h = GuiHarness::new(vec![server.base_url()]);
        h.step();
        h
    }

    fn add_fic(h: &mut GuiHarness) {
        h.app.submit_add_fic(FIC_ID.to_string());
        assert!(h.wait_for_tasks(400), "add did not finish");
    }

    fn refresh_fic(h: &mut GuiHarness) {
        h.app.select_fic(FIC_ID);
        h.app.refresh_selected();
        assert!(h.wait_for_tasks(400), "refresh did not finish");
    }

    fn wait_until(h: &mut GuiHarness, done: impl Fn(&GuiHarness) -> bool) {
        for _ in 0..400 {
            if done(h) {
                return;
            }
            h.step();
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        panic!("condition never became true");
    }

    fn wait_for_epub(h: &mut GuiHarness, bytes: &[u8]) {
        wait_until(h, |h| saved_epub(h).as_deref() == Some(bytes));
    }

    fn let_worker_settle(h: &mut GuiHarness) {
        std::thread::sleep(std::time::Duration::from_millis(200));
        h.step_n(3);
    }

    fn saved_epub(h: &GuiHarness) -> Option<Vec<u8>> {
        h.app
            .epub_path(FIC_ID)
            .map(|path| std::fs::read(path).unwrap())
    }

    fn epub_failure(h: &GuiHarness) -> Option<TaskState> {
        h.app
            .task_states()
            .into_iter()
            .find(|t| t.kind == TaskKind::EpubBackup { fic_id: FIC_ID })
    }

    #[test]
    fn nothing_is_downloaded_while_the_option_is_off() {
        let server = MockServer::start();
        given_work(&server);
        let epub = given_epub(&server, b"epub v1");
        let mut h = given_harness(&server);

        add_fic(&mut h);
        let_worker_settle(&mut h);

        epub.assert_calls(0);
        assert_eq!(saved_epub(&h), None);
        assert!(!h.db_path.parent().unwrap().join("epubs").exists());
    }

    #[test]
    fn adding_a_fic_saves_its_epub_next_to_the_library_using_the_login() {
        let server = MockServer::start();
        given_work(&server);
        let epub = server.mock(|when, then| {
            when.method(GET)
                .path(EPUB_PATH)
                .header_includes("cookie", "remember_user_token=abc");
            then.status(200).body(b"epub v1");
        });
        let mut h = given_harness(&server);
        h.session_store.set(Some(Ao3Session {
            username: "reader".into(),
            cookie: "remember_user_token=abc".into(),
        }));
        h.app.set_epub_backups(true);

        add_fic(&mut h);
        wait_for_epub(&mut h, b"epub v1");

        epub.assert_calls(1);
        let expected = h
            .db_path
            .parent()
            .unwrap()
            .join("epubs")
            .join(format!("{FIC_ID}.epub"));
        assert_eq!(h.app.epub_path(FIC_ID), Some(expected));
    }

    #[test]
    fn a_refresh_replaces_the_saved_epub() {
        let server = MockServer::start();
        given_work(&server);
        let mut epub = given_epub(&server, b"epub v1");
        let mut h = given_harness(&server);
        h.app.set_epub_backups(true);
        add_fic(&mut h);
        wait_for_epub(&mut h, b"epub v1");

        epub.delete();
        given_epub(&server, b"epub v2");
        refresh_fic(&mut h);

        wait_for_epub(&mut h, b"epub v2");
    }

    #[test]
    fn turning_it_off_keeps_saved_epubs_and_stops_downloads() {
        let server = MockServer::start();
        given_work(&server);
        let epub = given_epub(&server, b"epub v1");
        let mut h = given_harness(&server);
        h.app.set_epub_backups(true);
        add_fic(&mut h);
        wait_for_epub(&mut h, b"epub v1");

        h.app.set_epub_backups(false);
        refresh_fic(&mut h);
        let_worker_settle(&mut h);

        epub.assert_calls(1);
        assert_eq!(saved_epub(&h).as_deref(), Some(&b"epub v1"[..]));
    }

    #[test]
    fn a_failed_download_keeps_the_fic_and_can_be_retried_from_tasks() {
        let server = MockServer::start();
        given_work(&server);
        let mut broken = server.mock(|when, then| {
            when.method(GET).path(EPUB_PATH);
            then.status(500);
        });
        let mut h = given_harness(&server);
        h.app.set_epub_backups(true);

        add_fic(&mut h);
        wait_until(&mut h, |h| epub_failure(h).is_some());

        assert_eq!(h.app.fics().len(), 1);
        assert_eq!(saved_epub(&h), None);
        let failure = epub_failure(&h).expect("no EPUB backup row in Tasks");
        assert!(
            matches!(&failure.status, TaskStatus::Failed(msg) if msg.starts_with("EPUB backup failed")),
            "{:?}",
            failure.status
        );

        broken.delete();
        given_epub(&server, b"epub v1");
        h.app.retry_task(failure.id);
        assert!(h.wait_for_tasks(400), "retry did not finish");

        assert_eq!(saved_epub(&h).as_deref(), Some(&b"epub v1"[..]));
        assert!(matches!(
            epub_failure(&h).map(|t| t.status),
            Some(TaskStatus::Done)
        ));
    }

    #[test]
    fn deleting_a_fic_deletes_its_epub() {
        let server = MockServer::start();
        given_work(&server);
        given_epub(&server, b"epub v1");
        let mut h = given_harness(&server);
        h.app.set_epub_backups(true);
        add_fic(&mut h);
        wait_for_epub(&mut h, b"epub v1");
        let path = h.app.epub_path(FIC_ID).expect("EPUB should be saved");

        h.app.select_fic(FIC_ID);
        h.app.delete_selected();
        h.step();

        assert!(h.app.fics().is_empty());
        assert!(!path.exists());
    }
}
