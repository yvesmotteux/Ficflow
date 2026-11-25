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
//!    (e.g. open the Add-Fic modal then submit), since plain egui
//!    doesn't give us a clean event-injection API the way kittest does

#![allow(dead_code)] // Suppress warnings for harness helpers used by future test files

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rusqlite::Connection;
use tempfile::TempDir;

use ficflow::application::background_refresh::RefreshSchedule;
use ficflow::error::FicflowError;
use ficflow::infrastructure::external::ao3::session::{Ao3Session, SessionStore};
use ficflow::interfaces::gui::{FicflowApp, FicflowConfig};

use crate::common::fixtures;

/// `AppConfig` reads `XDG_CONFIG_HOME` (via `dirs_next`, Linux-only — this
/// suite only runs on Linux, see `.github/workflows/test.yml`) to find
/// `config.toml`; overriding it per-harness keeps tests off the developer's
/// real `~/.config/ficflow/config.toml`. It's process-global state, so this
/// lock serializes every `GuiHarness` for the harness's whole lifetime
/// (held via the guard below) instead of gating just the `set_var` call.
static CONFIG_ENV_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default)]
pub struct MemorySessionStore(Mutex<Option<Ao3Session>>);

impl MemorySessionStore {
    pub fn set(&self, session: Option<Ao3Session>) {
        *self.0.lock().unwrap() = session;
    }
}

impl SessionStore for MemorySessionStore {
    fn load(&self) -> Option<Ao3Session> {
        self.0.lock().unwrap().clone()
    }

    fn save(&self, session: &Ao3Session) -> Result<(), FicflowError> {
        self.set(Some(session.clone()));
        Ok(())
    }

    fn clear(&self) -> Result<(), FicflowError> {
        self.set(None);
        Ok(())
    }
}

pub struct GuiHarness {
    pub app: FicflowApp,
    pub ctx: egui::Context,
    /// Independent connection to the same SQLite file the GUI uses.
    /// Lets tests seed fixtures and assert against the underlying DB.
    pub conn: Connection,
    pub db_path: PathBuf,
    pub session_store: Arc<MemorySessionStore>,
    background_refresh: Option<RefreshSchedule>,
    /// Held so the temp file doesn't get unlinked until the harness drops.
    _temp_dir: TempDir,
    /// Keeps `XDG_CONFIG_HOME` pinned to this harness's scratch dir for as
    /// long as the harness lives — released (and the lock freed for the
    /// next test) on drop.
    _config_env_guard: MutexGuard<'static, ()>,
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
        Self::build(
            ao3_urls,
            "http://127.0.0.1:1".into(),
            conn,
            db_path,
            temp_dir,
            None,
        )
    }

    pub fn with_background_refresh(
        ao3_urls: Vec<String>,
        conn: Connection,
        db_path: PathBuf,
        temp_dir: TempDir,
        schedule: RefreshSchedule,
    ) -> Self {
        Self::build(
            ao3_urls,
            "http://127.0.0.1:1".into(),
            conn,
            db_path,
            temp_dir,
            Some(schedule),
        )
    }

    pub fn with_latest_release_url(latest_release_url: String) -> Self {
        let (conn, db_path, temp_dir) = fixtures::given_test_database();
        Self::build(
            vec!["http://127.0.0.1:1".into()],
            latest_release_url,
            conn,
            db_path,
            temp_dir,
            None,
        )
    }

    fn build(
        ao3_urls: Vec<String>,
        latest_release_url: String,
        conn: Connection,
        db_path: PathBuf,
        temp_dir: TempDir,
        background_refresh: Option<RefreshSchedule>,
    ) -> Self {
        // A poisoned lock (previous test panicked while holding it) is
        // still safe to reuse here — we're about to overwrite the env var
        // ourselves regardless of what state it was left in.
        let config_env_guard = CONFIG_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // SAFETY: serialized by `CONFIG_ENV_LOCK`, held for this harness's
        // whole lifetime, so no other thread reads/writes the env at the
        // same time.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        }

        let ctx = egui::Context::default();
        let session_store = Arc::new(MemorySessionStore::default());
        let config = FicflowConfig {
            db_path: Some(db_path.clone()),
            session_store: session_store.clone(),
            session_site: ao3_urls[0].clone(),
            ao3_urls,
            // Fail-fast: tests should never sit through the production
            // 2- or 3-cycle retry storm.
            max_retry_cycles: 1,
            latest_release_url,
            background_refresh,
        };
        let app = FicflowApp::with_config(&ctx, config)
            .expect("FicflowApp::with_config failed in test harness");
        Self {
            app,
            ctx,
            conn,
            db_path,
            session_store,
            background_refresh,
            _temp_dir: temp_dir,
            _config_env_guard: config_env_guard,
        }
    }

    /// Rebuilds `self.app` against the same DB and config file, simulating
    /// an app restart. `self.conn`, `self.ctx`, and the `XDG_CONFIG_HOME`
    /// override are unaffected.
    pub fn restart(&mut self, ao3_urls: Vec<String>) {
        let config = FicflowConfig {
            db_path: Some(self.db_path.clone()),
            session_store: self.session_store.clone(),
            session_site: ao3_urls[0].clone(),
            ao3_urls,
            max_retry_cycles: 1,
            latest_release_url: "http://127.0.0.1:1".into(),
            background_refresh: self.background_refresh,
        };
        self.app = FicflowApp::with_config(&self.ctx, config)
            .expect("FicflowApp::with_config failed on restart");
    }

    /// Run one frame with no input. Discards `FullOutput`.
    pub fn step(&mut self) {
        // The window chrome rasterises its SVG atlas to a 4096x5462
        // texture; egui's `RawInput::default().max_texture_side` is
        // 2048, which would panic the upload. Production gets a much
        // larger limit from the wgpu adapter; mirror that in tests.
        let raw_input = egui::RawInput {
            max_texture_side: Some(8192),
            ..Default::default()
        };
        let _ = self.run(raw_input);
    }

    pub fn step_with_close_request(&mut self) -> egui::FullOutput {
        let mut raw_input = egui::RawInput {
            max_texture_side: Some(8192),
            ..Default::default()
        };
        raw_input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        self.run(raw_input)
    }

    fn run(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(raw_input, |ui| {
            app.render(ui);
        });
        out.textures_delta.clear();
        out
    }

    /// Run one frame with a single key press injected, so shortcut
    /// handlers (`handle_shortcuts`) see `key_pressed(key)` this frame.
    pub fn step_with_key(&mut self, key: egui::Key) {
        let mut raw_input = egui::RawInput {
            max_texture_side: Some(8192),
            ..Default::default()
        };
        raw_input.events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::default(),
        });
        let _ = self.run(raw_input);
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
