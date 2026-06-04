#[cfg(test)]
mod tests {
    use std::time::Duration;

    use httpmock::Method::GET;
    use httpmock::MockServer;

    use crate::harness::GuiHarness;

    fn cancels_close(out: &egui::FullOutput) -> bool {
        out.viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|v| {
                v.commands
                    .iter()
                    .any(|c| matches!(c, egui::ViewportCommand::CancelClose))
            })
    }

    fn given_slow_mock_ao3_server() -> (MockServer, u64) {
        let fic_id = 53960491;
        let html = std::fs::read_to_string("tests/fixtures/ao3_fic_example1.html")
            .expect("Failed to read mock HTML file");
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path(format!("/works/{}", fic_id));
            then.status(200).delay(Duration::from_secs(2)).body(html);
        });
        (server, fic_id)
    }

    #[test]
    fn close_with_no_tasks_passes_through() {
        let mut h = GuiHarness::new(vec!["http://127.0.0.1:1".into()]);
        h.step_n(2);

        let out = h.step_with_close_request();

        assert!(!cancels_close(&out));
        assert!(!h.app.confirm_quit_open());
    }

    #[test]
    fn close_with_running_task_is_cancelled_until_confirmed() {
        let (server, fic_id) = given_slow_mock_ao3_server();
        let mut h = GuiHarness::new(vec![server.base_url()]);
        h.step_n(1);

        h.app.submit_add_fic(fic_id.to_string());
        h.step();
        assert!(h.app.has_running_tasks());

        let out = h.step_with_close_request();
        assert!(cancels_close(&out));
        assert!(h.app.confirm_quit_open());

        let out = h.step_with_close_request();
        assert!(cancels_close(&out));

        h.app.confirm_quit();
        assert!(!h.app.confirm_quit_open());
        assert!(h.app.has_running_tasks());

        let out = h.step_with_close_request();
        assert!(!cancels_close(&out));
    }
}
