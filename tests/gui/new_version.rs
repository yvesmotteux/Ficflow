#[cfg(test)]
mod tests {
    use std::time::Duration;

    use httpmock::Method::GET;
    use httpmock::{Mock, MockServer};

    use ficflow::interfaces::gui::View;

    use crate::harness::GuiHarness;

    fn given_latest_release<'a>(server: &'a MockServer, tag: &str) -> Mock<'a> {
        server.mock(|when, then| {
            when.method(GET).path("/releases/latest");
            then.status(200).json_body(serde_json::json!({
                "tag_name": tag,
                "html_url": format!("https://github.com/yvesmotteux/Ficflow/releases/tag/{tag}"),
            }));
        })
    }

    fn open_settings_and_wait_for_check(h: &mut GuiHarness, mock: &Mock) {
        h.app.open_view(View::Settings);
        for _ in 0..200 {
            h.step();
            if mock.calls() > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        for _ in 0..10 {
            std::thread::sleep(Duration::from_millis(15));
            h.step();
        }
    }

    #[test]
    fn settings_flags_a_newer_release() {
        let server = MockServer::start();
        let mock = given_latest_release(&server, "v99.0.0");
        let mut h = GuiHarness::with_latest_release_url(server.url("/releases/latest"));

        open_settings_and_wait_for_check(&mut h, &mock);

        let release = h.app.newer_release().expect("newer release not flagged");
        assert_eq!(release.tag_name, "v99.0.0");
        assert_eq!(
            release.html_url,
            "https://github.com/yvesmotteux/Ficflow/releases/tag/v99.0.0"
        );
    }

    #[test]
    fn settings_ignores_an_older_or_equal_release() {
        for tag in ["v0.0.1", &format!("v{}", env!("CARGO_PKG_VERSION"))] {
            let server = MockServer::start();
            let mock = given_latest_release(&server, tag);
            let mut h = GuiHarness::with_latest_release_url(server.url("/releases/latest"));

            open_settings_and_wait_for_check(&mut h, &mock);

            mock.assert_calls(1);
            assert!(h.app.newer_release().is_none(), "{tag} flagged as newer");
        }
    }

    #[test]
    fn release_check_waits_until_settings_is_opened() {
        let server = MockServer::start();
        let mock = given_latest_release(&server, "v99.0.0");
        let mut h = GuiHarness::with_latest_release_url(server.url("/releases/latest"));

        h.step_n(5);
        std::thread::sleep(Duration::from_millis(100));

        mock.assert_calls(0);
        assert!(h.app.newer_release().is_none());
    }

    #[test]
    fn unreachable_release_server_shows_no_warning() {
        let mut h = GuiHarness::with_latest_release_url("http://127.0.0.1:1".into());

        h.app.open_view(View::Settings);
        h.step_n(20);

        assert!(h.app.newer_release().is_none());
    }
}
