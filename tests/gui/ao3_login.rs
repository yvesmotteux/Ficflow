#[cfg(test)]
mod tests {
    use std::time::Duration;

    use httpmock::Method::{GET, POST};
    use httpmock::{Mock, MockServer};

    use ficflow::infrastructure::external::ao3::session::{Ao3Session, SessionStore};
    use ficflow::interfaces::gui::{TaskStatus, View};

    use crate::harness::GuiHarness;

    const FIC_ID: u64 = 53960491;

    fn fic_html() -> String {
        std::fs::read_to_string("tests/fixtures/ao3_fic_example1.html").unwrap()
    }

    fn given_login_endpoints(server: &MockServer) -> Mock<'_> {
        server.mock(|when, then| {
            when.method(GET).path("/token_dispenser.json");
            then.status(200)
                .json_body(serde_json::json!({ "token": "tok123" }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .path("/users/login")
                .form_urlencoded_tuple("authenticity_token", "tok123")
                .form_urlencoded_tuple("user[login]", "reader")
                .form_urlencoded_tuple("user[password]", "wrong");
            then.status(200)
                .body("<p>The password or user name you entered doesn't match our records.</p>");
        });
        server.mock(|when, then| {
            when.method(GET).path("/users/reader");
            then.status(200)
                .body(r#"<a href="/users/logout">Log Out</a>"#);
        });
        server.mock(|when, then| {
            when.method(POST)
                .path("/users/login")
                .form_urlencoded_tuple("authenticity_token", "tok123")
                .form_urlencoded_tuple("user[login]", "reader")
                .form_urlencoded_tuple("user[password]", "hunter2")
                .form_urlencoded_tuple("user[remember_me]", "1");
            then.status(302)
                .header("Location", "/users/reader")
                .header("Set-Cookie", "remember_user_token=abc; path=/; HttpOnly");
        })
    }

    fn given_restricted_work(server: &MockServer) -> (Mock<'_>, Mock<'_>) {
        let with_session = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/works/{FIC_ID}"))
                .header_includes("cookie", "remember_user_token=abc");
            then.status(200).body(fic_html());
        });
        let without_session = server.mock(|when, then| {
            when.method(GET)
                .path(format!("/works/{FIC_ID}"))
                .header_missing("cookie");
            then.status(302)
                .header("Location", "/users/login?restricted=true");
        });
        server.mock(|when, then| {
            when.method(GET).path("/users/login");
            then.status(200).body("<html>Log In</html>");
        });
        (with_session, without_session)
    }

    fn log_in(h: &mut GuiHarness, password: &str) {
        h.app.open_view(View::Settings);
        h.step();
        let ctx = h.ctx.clone();
        h.app.log_in_to_ao3(&ctx, "reader", password);
        for _ in 0..200 {
            h.step();
            if !h.app.ao3_logging_in() {
                return;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        panic!("login did not finish in time");
    }

    fn stored_session() -> Ao3Session {
        Ao3Session {
            username: "reader".into(),
            cookie: "remember_user_token=abc".into(),
        }
    }

    fn failure_message(h: &GuiHarness) -> String {
        match &h.app.task_states()[0].status {
            TaskStatus::Failed(msg) => msg.clone(),
            other => panic!("expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn logging_in_lets_a_restricted_fic_be_added() {
        let server = MockServer::start();
        let login = given_login_endpoints(&server);
        let (with_session, _) = given_restricted_work(&server);
        let mut h = GuiHarness::new(vec![server.base_url()]);

        log_in(&mut h, "hunter2");

        login.assert_calls(1);
        assert_eq!(h.app.ao3_username(), Some("reader"));
        assert_eq!(h.app.ao3_login_error(), None);
        let saved = h.session_store.load().expect("session not saved");
        assert_eq!(saved.username, "reader");
        assert!(saved.cookie.contains("remember_user_token=abc"));
        assert!(!saved.cookie.contains("hunter2"));

        h.app.submit_add_fic(FIC_ID.to_string());
        assert!(h.wait_for_tasks(80));
        with_session.assert_calls(1);
        assert_eq!(h.app.fics().len(), 1);
        assert_eq!(h.app.fics()[0].id, FIC_ID);
    }

    #[test]
    fn a_wrong_password_shows_an_error_and_stays_logged_out() {
        let server = MockServer::start();
        given_login_endpoints(&server);
        let mut h = GuiHarness::new(vec![server.base_url()]);

        log_in(&mut h, "wrong");

        assert_eq!(h.app.ao3_username(), None);
        assert_eq!(
            h.app.ao3_login_error(),
            Some("AO3 didn't accept that user name and password")
        );
        assert!(h.session_store.load().is_none());
    }

    #[test]
    fn an_expired_login_is_forgotten_and_reported() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path(format!("/works/{FIC_ID}"));
            then.status(302)
                .header("Location", "/users/login?restricted=true");
        });
        server.mock(|when, then| {
            when.method(GET).path("/users/login");
            then.status(200).body("<html>Log In</html>");
        });
        let mut h = GuiHarness::new(vec![server.base_url()]);
        h.session_store.set(Some(stored_session()));

        h.app.submit_add_fic(FIC_ID.to_string());
        assert!(h.wait_for_tasks(80));

        assert!(failure_message(&h).contains("AO3 login has expired"));
        assert!(h.session_store.load().is_none());
        h.app.open_view(View::Settings);
        h.step();
        assert_eq!(h.app.ao3_username(), None);
    }

    #[test]
    fn the_login_is_never_sent_to_mirrors() {
        let primary = MockServer::start();
        primary.mock(|when, then| {
            when.method(GET).path(format!("/works/{FIC_ID}"));
            then.status(503);
        });
        let mirror = MockServer::start();
        let leaked = mirror.mock(|when, then| {
            when.method(GET).header_exists("cookie");
            then.status(500);
        });
        mirror.mock(|when, then| {
            when.method(GET).path(format!("/works/{FIC_ID}"));
            then.status(200).body(fic_html());
        });
        let mut h = GuiHarness::new(vec![primary.base_url(), mirror.base_url()]);
        h.session_store.set(Some(stored_session()));

        h.app.submit_add_fic(FIC_ID.to_string());
        assert!(h.wait_for_tasks(600));

        leaked.assert_calls(0);
        assert_eq!(h.app.fics().len(), 1);
    }

    #[test]
    fn a_connection_failure_shows_its_real_cause() {
        let mut h = GuiHarness::new(vec!["http://127.0.0.1:1".into()]);

        h.app.submit_add_fic(FIC_ID.to_string());
        assert!(h.wait_for_tasks(200));

        let message = failure_message(&h);
        assert!(
            message.contains("Connection refused"),
            "unexpected message: {message}"
        );
    }

    #[test]
    fn logging_out_forgets_the_login() {
        let server = MockServer::start();
        given_restricted_work(&server);
        let mut h = GuiHarness::new(vec![server.base_url()]);
        h.session_store.set(Some(stored_session()));
        h.app.open_view(View::Settings);
        h.step();
        assert_eq!(h.app.ao3_username(), Some("reader"));

        h.app.log_out_of_ao3();

        assert_eq!(h.app.ao3_username(), None);
        assert!(h.session_store.load().is_none());
        h.app.submit_add_fic(FIC_ID.to_string());
        assert!(h.wait_for_tasks(80));
        assert!(failure_message(&h).contains("only visible to logged-in AO3 users"));
    }
}
