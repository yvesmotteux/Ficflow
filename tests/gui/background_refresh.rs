#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;
    use httpmock::Method::GET;
    use httpmock::MockServer;

    use ficflow::application::background_refresh::RefreshSchedule;
    use ficflow::domain::fanfiction::{Fanfiction, ReadingStatus};
    use ficflow::interfaces::gui::{TaskKind, TaskStatus};

    use crate::common::fixtures;
    use crate::harness::GuiHarness;

    const WIP_ID: u64 = 53681185;
    const OTHER_ID: u64 = 53960491;

    fn schedule() -> RefreshSchedule {
        RefreshSchedule {
            idle_poll: Duration::from_millis(20),
            frequent_every: chrono::Duration::hours(1),
            occasional_every: chrono::Duration::hours(1),
            ..RefreshSchedule::default()
        }
    }

    fn fic(id: u64, checked_days_ago: i64) -> Fanfiction {
        let mut fic = fixtures::given_sample_fanfiction(id, "Seeded");
        fic.chapters_published = 18;
        fic.chapters_total = None;
        fic.complete = false;
        fic.last_checked_date = Utc::now() - chrono::Duration::days(checked_days_ago);
        fic
    }

    fn boot(server: &MockServer, fics: &[Fanfiction], schedule: RefreshSchedule) -> GuiHarness {
        let (conn, db_path, td) = fixtures::given_test_database();
        for fic in fics {
            fixtures::when_fanfiction_added_to_db(&conn, fic).unwrap();
        }
        GuiHarness::with_background_refresh(vec![server.base_url()], conn, db_path, td, schedule)
    }

    fn wait_until(h: &mut GuiHarness, frames: usize, done: impl Fn(&GuiHarness) -> bool) -> bool {
        for _ in 0..frames {
            h.step();
            if done(h) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        false
    }

    fn idle(h: &mut GuiHarness) {
        wait_until(h, 30, |_| false);
    }

    fn serve<'a>(server: &'a MockServer, id: u64, fixture: &str) -> httpmock::Mock<'a> {
        let html = std::fs::read_to_string(format!("tests/fixtures/{fixture}")).unwrap();
        server.mock(|when, then| {
            when.method(GET).path(format!("/works/{id}"));
            then.status(200).body(html);
        })
    }

    #[test]
    fn unfinished_fics_are_checked_on_startup_without_any_task_showing() {
        let server = MockServer::start();
        let work = serve(&server, WIP_ID, "ao3_fic_up_to_date.html");
        let mut h = boot(&server, &[fic(WIP_ID, 2)], schedule());

        assert!(wait_until(&mut h, 200, |h| h.app.inbox_messages().len() == 1));
        assert!(wait_until(&mut h, 50, |h| h.app.fics()[0]
            .chapters_published
            == 32));
        assert!(h.app.task_states().is_empty());
        work.assert_calls(1);
    }

    #[test]
    fn recently_checked_fics_are_skipped_on_startup_but_checked_on_request() {
        let server = MockServer::start();
        let work = serve(&server, WIP_ID, "ao3_fic_up_to_date.html");
        let mut h = boot(&server, &[fic(WIP_ID, 0)], schedule());

        idle(&mut h);
        work.assert_calls(0);

        h.app.check_for_updates_now();
        assert!(wait_until(&mut h, 200, |h| h.app.inbox_messages().len() == 1));
        work.assert_calls(1);
    }

    #[test]
    fn finished_and_abandoned_fics_only_get_the_occasional_stats_refresh() {
        let server = MockServer::start();
        let stale_finished = serve(&server, WIP_ID, "ao3_fic_up_to_date.html");
        let abandoned = serve(&server, OTHER_ID, "ao3_fic_example1.html");
        let mut finished = fic(WIP_ID, 20);
        finished.complete = true;
        finished.reading_status = ReadingStatus::Read;
        let mut dropped = fic(OTHER_ID, 2);
        dropped.reading_status = ReadingStatus::Abandoned;
        let mut h = boot(&server, &[finished, dropped], schedule());

        assert!(wait_until(&mut h, 200, |h| {
            h.app
                .fics()
                .iter()
                .any(|f| f.id == WIP_ID && f.chapters_published == 32)
        }));
        idle(&mut h);
        stale_finished.assert_calls(1);
        abandoned.assert_calls(0);
    }

    #[test]
    fn a_fic_that_keeps_failing_shows_up_in_tasks_after_three_attempts() {
        let server = MockServer::start();
        let missing = server.mock(|when, then| {
            when.method(GET).path(format!("/works/{WIP_ID}"));
            then.status(500);
        });
        let every_poll = RefreshSchedule {
            frequent_every: chrono::Duration::zero(),
            ..schedule()
        };
        let mut h = boot(&server, &[fic(WIP_ID, 2)], every_poll);

        assert!(wait_until(&mut h, 1500, |h| h.app.failed_task_count() == 1));
        let tasks = h.app.task_states();
        assert_eq!(tasks[0].kind, TaskKind::Refresh { fic_id: WIP_ID });
        assert!(
            matches!(&tasks[0].status, TaskStatus::Failed(msg) if msg.starts_with("Automatic update check failed"))
        );
        idle(&mut h);
        missing.assert_calls(3);
    }

    #[test]
    fn cloudflare_pauses_every_automatic_check_and_says_so_once() {
        let server = MockServer::start();
        let shielded = server.mock(|when, then| {
            when.method(GET).path_includes("/works/");
            then.status(429)
                .header("cf-mitigated", "challenge")
                .body("<html>Just a moment...</html>");
        });
        let mut h = boot(&server, &[fic(WIP_ID, 2), fic(OTHER_ID, 3)], schedule());

        assert!(wait_until(&mut h, 200, |h| h.app.failed_task_count() == 1));
        idle(&mut h);
        let tasks = h.app.task_states();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].kind, TaskKind::BackgroundCheck);
        shielded.assert_calls(1);
    }

    #[test]
    fn turning_automatic_checks_off_stops_the_startup_check() {
        let server = MockServer::start();
        let work = serve(&server, WIP_ID, "ao3_fic_up_to_date.html");
        let mut h = boot(&server, &[], schedule());
        h.app.set_background_checks(false);
        fixtures::when_fanfiction_added_to_db(&h.conn, &fic(WIP_ID, 2)).unwrap();

        h.restart(vec![server.base_url()]);
        idle(&mut h);
        work.assert_calls(0);
        assert!(h.app.inbox_messages().is_empty());
    }
}
