//! Headless test harness for `FicflowApp`. Owns:
//!  - an `egui::Context` it ticks with empty `RawInput` (no real window)
//!  - the app under test (built via `FicflowApp::with_config`)
//!  - a `Connection` to the same temp DB the app uses, so test code can
//!    seed fixtures and assert against rows directly
//!  - the `TempDir` keeping the DB file alive for the lifetime of the
//!    harness (drops it at end-of-test, removing the file)
//!
//! Tests interact with the harness through three knobs:
//!  - `step()` / `step_n(N)` — advance the egui frame loop, no input
//!  - `wait_for_tasks(max_frames)` — poll until background fetches finish
//!  - `app` (public field) — direct access to FicflowApp for state
//!    inspection and for triggering UI flows by mutating modal state
//!    (e.g. open the Add-Fic modal then submit), since plain egui 0.29
//!    doesn't give us a clean event-injection API the way kittest does

#![allow(dead_code)] // Suppress warnings for harness helpers used by future test files

use std::path::PathBuf;
use std::time::Duration;

use rusqlite::Connection;
use tempfile::TempDir;

use ficflow::interfaces::gui::{FicflowApp, FicflowConfig};

use crate::common::fixtures;

pub struct GuiHarness {
    pub app: FicflowApp,
    pub ctx: egui::Context,
    /// Independent connection to the same SQLite file the GUI uses.
    /// Lets tests seed fixtures and assert against the underlying DB.
    pub conn: Connection,
    pub db_path: PathBuf,
    /// Held so the temp file doesn't get unlinked until the harness drops.
    _temp_dir: TempDir,
}

impl GuiHarness {
    /// Builds a harness wired to a fresh temp DB and the supplied AO3
    /// base URLs (typically a single `httpmock` server URL). The mock
    /// must already be set up before calling — the worker thread starts
    /// inside `with_config` and may begin issuing requests immediately.
    pub fn new(ao3_urls: Vec<String>) -> Self {
        let (conn, db_path, _temp_dir) = fixtures::given_test_database();
        Self::with_db(ao3_urls, conn, db_path, _temp_dir)
    }

    /// Variant that takes a pre-seeded connection + path. Useful when
    /// the test wants to insert fixtures *before* the GUI boots so the
    /// initial `load_fics` already sees them.
    pub fn with_db(
        ao3_urls: Vec<String>,
        conn: Connection,
        db_path: PathBuf,
        temp_dir: TempDir,
    ) -> Self {
        let ctx = egui::Context::default();
        let config = FicflowConfig {
            db_path: Some(db_path.clone()),
            ao3_urls,
            // Fail-fast: tests should never sit through the production
            // 2- or 3-cycle retry storm.
            max_retry_cycles: 1,
        };
        let app = FicflowApp::with_config(&ctx, config)
            .expect("FicflowApp::with_config failed in test harness");
        Self {
            app,
            ctx,
            conn,
            db_path,
            _temp_dir: temp_dir,
        }
    }

    /// Run one frame with no input. Discards `FullOutput` — tests that
    /// need it can call `Context::run` directly.
    pub fn step(&mut self) {
        // The Phase 12 chrome rasterises its SVG atlas to a 4096x5462
        // texture; egui's `RawInput::default().max_texture_side` is
        // 2048, which would panic the upload. Production gets a much
        // larger limit from the wgpu adapter; mirror that in tests.
        let raw_input = egui::RawInput {
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let app = &mut self.app;
        let _ = self.ctx.run(raw_input, |ctx| {
            app.render(ctx);
        });
    }

    pub fn step_n(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    /// Tick frames until the worker thread reports no in-flight tasks
    /// or `max_frames` is exhausted. Returns whether we observed idle
    /// (false ⇒ test should panic / fail with timeout).
    ///
    /// Sleeps briefly between frames so the worker thread (which the
    /// GUI just woke up via `enqueue_*`) has a chance to make progress
    /// — without the sleep we'd burn through `max_frames` before the
    /// worker even picks up the channel message.
    pub fn wait_for_tasks(&mut self, max_frames: usize) -> bool {
        for _ in 0..max_frames {
            if !self.app.has_running_tasks() {
                // One more tick to drain `recent_completions` /
                // `recent_refreshes` queues into the GUI state.
                self.step();
                return true;
            }
            self.step();
            std::thread::sleep(Duration::from_millis(15));
        }
        false
    }
}
