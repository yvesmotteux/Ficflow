use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use egui_notify::Toasts;
use rusqlite::Connection;

use crate::application::{
    add_to_shelf::add_to_shelf, count_fics_in_shelf::count_fics_in_shelf,
    create_shelf::create_shelf, delete_fic, delete_shelf, list_fics, list_shelf_fics, list_shelves,
    list_shelves_for_fic, remove_from_shelf, update_chapters, update_note, update_rating,
    update_read_count, update_status,
};
use crate::domain::fanfiction::{Fanfiction, ReadingStatus, UserRating};
use crate::domain::shelf::Shelf;
use crate::error::FicflowError;
use crate::infrastructure::config::{AppConfig, ColumnKey, SortDirection, SortPref};
use crate::infrastructure::external::ao3::fetcher::{ALT_AO3_URL, PRIMARY_AO3_URL, PROXY_AO3_URL};
use crate::infrastructure::persistence::database::connection::{
    establish_connection, open_configured_db,
};
use crate::infrastructure::SqliteRepository;

use super::fonts;
use super::selection::Selection;
use super::tasks::TaskExecutor;
use super::view::View;
use super::views::details_panel::DetailsState;
use super::views::modals::add_fic_dialog::{self, AddFicState};
use super::views::modals::shelf_modals::{self, CreateState};
use super::views::modals::{bulk_modals, column_picker};
use super::views::settings_view;
use super::views::tasks_view;
use super::views::{
    details_panel, library_view, selection_bar, sidebar, LibraryCounts, LibraryViewState,
    SelectionBarState, SidebarState, TaskFilter, TasksViewState,
};

pub struct FicflowApp {
    connection: Connection,
    fics: Vec<Fanfiction>,
    shelves: Vec<Shelf>,
    /// Cached fic-id membership for the currently-selected shelf view. Empty
    /// (and unused) when `current_view` is anything other than `View::Shelf(_)`.
    shelf_members: HashSet<u64>,
    /// Cached shelf-ids that the currently-selected fic belongs to. Empty
    /// when `selection` is not `Single(_)`.
    selection_shelf_ids: HashSet<u64>,
    /// Cached per-shelf fic counts shown in the sidebar. Refreshed on app
    /// init and after any operation that can change shelf membership.
    shelf_counts: HashMap<u64, usize>,
    config: AppConfig,
    /// Set after the first `update()` applies the persisted maximized /
    /// fullscreen state via `ViewportCommand`. Without this gate the
    /// command would re-fire every frame.
    initial_window_state_applied: bool,
    sort: SortPref,
    search_query: String,
    show_column_picker: bool,
    selection: Selection,
    /// Anchor row id for shift-click range selection in the library table.
    last_clicked_id: Option<u64>,
    current_view: View,
    create_shelf_modal: CreateState,
    delete_shelf_pending: Option<u64>,
    delete_fics_pending: Option<Vec<u64>>,
    add_fic_modal: AddFicState,
    task_executor: TaskExecutor,
    task_filter: TaskFilter,
    /// Set by the Ctrl+F shortcut; library_view consumes it on next paint.
    focus_search_pending: bool,
    toasts: Toasts,
}

#[derive(Debug)]
pub enum InitError {
    Database(FicflowError),
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::Database(e) => write!(f, "database init failed: {}", e),
        }
    }
}

impl std::error::Error for InitError {}

/// Explicit wiring for `FicflowApp`. The production binary derives
/// this from the environment via `FicflowConfig::default()`; embedders
/// and integration tests construct it directly so they can point the
/// app at a chosen SQLite file and AO3 endpoint without going through
/// process-global env vars.
#[derive(Clone)]
pub struct FicflowConfig {
    /// Override for the SQLite DB. `None` falls back to
    /// `establish_connection()` (which checks `FICFLOW_DB_PATH` then
    /// the platform data dir).
    pub db_path: Option<PathBuf>,
    /// AO3 base URLs to round-robin during fetches.
    pub ao3_urls: Vec<String>,
    /// How many full URL-rotation cycles to attempt before giving up
    /// on a fetch. Lower values fail-fast.
    pub max_retry_cycles: u32,
}

impl Default for FicflowConfig {
    fn default() -> Self {
        let (ao3_urls, max_retry_cycles) = ao3_config_from_env();
        Self {
            db_path: None,
            ao3_urls,
            max_retry_cycles,
        }
    }
}

impl FicflowApp {
    /// Production entry point: derives config from the environment and
    /// the platform data dir.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, InitError> {
        Self::with_config(&cc.egui_ctx, FicflowConfig::default())
    }

    /// Test/embedding entry point: every dependency the app talks to
    /// (DB path, AO3 base URLs) comes through `config`. The worker
    /// thread inside `TaskExecutor` is told the same `db_path` so it
    /// opens the same SQLite file, otherwise the GUI and the worker
    /// would be looking at different stores.
    ///
    /// Takes `&egui::Context` (not `&CreationContext`) so headless
    /// tests can build the app without going through eframe runtime.
    pub fn with_config(ctx: &egui::Context, config: FicflowConfig) -> Result<Self, InitError> {
        fonts::install_system_fallback(ctx);
        let connection = match &config.db_path {
            Some(path) => open_configured_db(path).map_err(InitError::Database)?,
            None => establish_connection().map_err(InitError::Database)?,
        };
        let fics = load_fics(&connection);
        let shelves = load_shelves(&connection);
        let app_config = AppConfig::load();
        let sort = app_config.default_sort;
        let task_executor = TaskExecutor::spawn(
            config.ao3_urls,
            config.max_retry_cycles,
            config.db_path.clone(),
        );
        let shelf_counts = compute_shelf_counts(&connection, &shelves);
        Ok(Self {
            connection,
            fics,
            shelves,
            shelf_members: HashSet::new(),
            selection_shelf_ids: HashSet::new(),
            shelf_counts,
            config: app_config,
            initial_window_state_applied: false,
            sort,
            search_query: String::new(),
            show_column_picker: false,
            selection: Selection::default(),
            last_clicked_id: None,
            current_view: View::default(),
            create_shelf_modal: CreateState::new(),
            delete_shelf_pending: None,
            delete_fics_pending: None,
            add_fic_modal: AddFicState::new(),
            task_executor,
            task_filter: TaskFilter::default(),
            focus_search_pending: false,
            toasts: Toasts::default(),
        })
    }

    // ---- Test-friendly accessors --------------------------------------
    // These are read-only views over internal state. They're kept narrow
    // on purpose: tests should drive behaviour via the same code paths
    // the GUI uses (modal state mutations, simulated input), not by
    // mutating internal fields directly.

    /// In-memory cache of all non-deleted fics, in the order
    /// `list_fics` returned them.
    pub fn fics(&self) -> &[Fanfiction] {
        &self.fics
    }

    /// In-memory cache of all non-deleted shelves.
    pub fn shelves(&self) -> &[Shelf] {
        &self.shelves
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn current_view(&self) -> &View {
        &self.current_view
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// Current sort preference (column + direction). Mirrors what the
    /// header glyphs show.
    pub fn sort(&self) -> SortPref {
        self.sort
    }

    /// Override the sort preference. Equivalent to clicking a column
    /// header until it lands on the desired direction.
    pub fn set_sort(&mut self, column: ColumnKey, direction: SortDirection) {
        self.sort = SortPref { column, direction };
    }

    /// IDs of the fics that pass the current view filter + search
    /// query, in the active sort order. Same set the library table
    /// renders (and that `Ctrl+A` selects).
    pub fn visible_ids(&self) -> Vec<u64> {
        library_view::visible_ids(
            &self.fics,
            &self.current_view,
            &self.shelf_members,
            &self.search_query,
            self.sort,
        )
    }

    /// Whether the right-hand details panel is currently mounted.
    /// Mirrors the gating in `update()` — only on a single-fic
    /// selection inside a library view.
    pub fn details_panel_visible(&self) -> bool {
        matches!(self.selection, Selection::Single(_)) && self.current_view.shows_library()
    }

    /// True while at least one background task (Add or Refresh) is
    /// in-flight.
    pub fn has_running_tasks(&self) -> bool {
        self.task_executor.has_running()
    }

    /// Snapshot of every task the worker has handled this session
    /// (Running / Done / Failed).
    pub fn task_states(&self) -> Vec<crate::interfaces::gui::tasks::TaskState> {
        self.task_executor.snapshot()
    }

    // ---- Programmatic control surface ----------------------------------
    // Equivalents of the user actions the GUI dispatches. Useful for
    // keyboard-shortcut bindings, scripted scenarios, embedders that
    // drive the app from outside `eframe`, and integration tests. They
    // route through the same internal state transitions and the same
    // application-layer entry points as their pointer-driven cousins.

    /// Enqueue a background fetch to add a new fanfiction by URL or
    /// numeric ID. The worker thread runs the AO3 fetch + DB insert;
    /// the in-memory `fics()` cache reflects the result on the next
    /// `render()` after the task completes.
    pub fn submit_add_fic(&self, input: impl Into<String>) {
        self.task_executor.enqueue_add(input.into());
    }

    /// Mark a single fic as the current selection. Mirrors a plain
    /// (non-modifier) row click in the library table.
    pub fn select_fic(&mut self, fic_id: u64) {
        self.selection = Selection::Single(fic_id);
        self.last_clicked_id = Some(fic_id);
        self.refresh_selection_shelf_ids();
    }

    /// Set the selection to a list of fic ids. The variant is chosen
    /// from the slice length: empty → None, one → Single, more →
    /// Multi. Equivalent to the result of a shift-click range or a
    /// series of ctrl-click toggles.
    pub fn select_fics(&mut self, ids: &[u64]) {
        self.selection = match ids {
            [] => Selection::None,
            [single] => Selection::Single(*single),
            many => Selection::Multi(many.to_vec()),
        };
        self.last_clicked_id = ids.last().copied();
        self.refresh_selection_shelf_ids();
    }

    /// Drop the current selection.
    pub fn clear_selection(&mut self) {
        self.selection = Selection::None;
        self.last_clicked_id = None;
        self.selection_shelf_ids.clear();
    }

    /// Switch to a different view. Equivalent to clicking a sidebar
    /// entry — the next `render()` pass refreshes shelf-members and
    /// prunes any selection that's no longer visible.
    pub fn open_view(&mut self, view: View) {
        self.current_view = view;
    }

    /// Enqueue a background re-fetch of the currently-selected fic.
    /// No-op if the selection isn't a single fic.
    pub fn refresh_selected(&self) {
        if let Selection::Single(id) = self.selection {
            if let Some(fic) = self.fics.iter().find(|f| f.id == id) {
                self.task_executor.enqueue_refresh(id, fic.title.clone());
            }
        }
    }

    /// Soft-delete every fic in the current selection. Works for both
    /// `Single` (the details-panel red-button case) and `Multi` (the
    /// bulk-action 🗑 in the selection bar). Clears the selection
    /// immediately — every member just got deleted, so keeping it
    /// pointing at stale ids would be confusing.
    pub fn delete_selected(&mut self) {
        let ids: Vec<u64> = match &self.selection {
            Selection::None => return,
            Selection::Single(id) => vec![*id],
            Selection::Multi(ids) => ids.clone(),
        };
        let repo = SqliteRepository::new(&self.connection);
        for id in &ids {
            if delete_fic::delete_fic(&repo, *id).is_ok() {
                self.fics.retain(|f| f.id != *id);
            }
        }
        self.selection = Selection::None;
        self.last_clicked_id = None;
        self.refresh_selection_shelf_ids();
        self.refresh_shelf_counts();
        if matches!(self.current_view, View::Shelf(_)) {
            self.refresh_shelf_members();
        }
    }

    /// Create a new shelf and refresh the in-memory caches. Mirrors the
    /// Create-Shelf modal's submit. The application layer rejects empty
    /// or whitespace-only names; toasts are surfaced either way for
    /// parity with the GUI flow.
    pub fn create_shelf(&mut self, name: impl AsRef<str>) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        match create_shelf(&repo, name.as_ref()) {
            Ok(shelf) => {
                self.toasts
                    .success(format!("Created shelf \u{201C}{}\u{201D}", shelf.name));
                self.shelves = load_shelves(&self.connection);
                self.refresh_shelf_counts();
                Ok(())
            }
            Err(err) => {
                self.toasts.error(format!("Couldn't create shelf: {}", err));
                Err(err)
            }
        }
    }

    /// Soft-delete a shelf. If the active view is that shelf, falls
    /// back to All Fanfictions so the user isn't left staring at a
    /// stale shelf-only filter.
    pub fn delete_shelf(&mut self, shelf_id: u64) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        match delete_shelf::delete_shelf(&repo, shelf_id) {
            Ok(()) => {
                self.toasts.success("Shelf deleted");
                if self.current_view == View::Shelf(shelf_id) {
                    self.current_view = View::AllFics;
                    self.shelf_members.clear();
                }
                self.shelves = load_shelves(&self.connection);
                self.refresh_shelf_counts();
                Ok(())
            }
            Err(err) => {
                self.toasts.error(format!("Couldn't delete shelf: {}", err));
                Err(err)
            }
        }
    }

    /// Add a fic to a shelf. Mirrors checking the shelf's box in the
    /// details-panel multi-select dropdown.
    pub fn add_fic_to_shelf(&mut self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        add_to_shelf(&repo, fic_id, shelf_id)?;
        self.refresh_selection_shelf_ids();
        self.refresh_shelf_counts();
        if matches!(self.current_view, View::Shelf(_)) {
            self.refresh_shelf_members();
        }
        Ok(())
    }

    /// Remove a fic from a shelf. Mirrors clicking the × on a shelf
    /// chip in the details-panel dropdown.
    pub fn remove_fic_from_shelf(
        &mut self,
        fic_id: u64,
        shelf_id: u64,
    ) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        remove_from_shelf::remove_from_shelf(&repo, fic_id, shelf_id)?;
        self.refresh_selection_shelf_ids();
        self.refresh_shelf_counts();
        if matches!(self.current_view, View::Shelf(_)) {
            self.refresh_shelf_members();
        }
        Ok(())
    }

    /// Set the search query. Mirrors typing into the search bar.
    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    /// Update the reading status of a fic. Mirrors the Status combo
    /// in the Your Info section.
    pub fn set_status(&mut self, fic_id: u64, status: ReadingStatus) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        let updated = update_status::update_reading_status(&repo, fic_id, status_payload(status))?;
        replace_in_cache(&mut self.fics, updated);
        Ok(())
    }

    /// Update the last-chapter-read marker. The application layer
    /// clamps to `chapters_total` and may auto-bump status / read_count
    /// when the final chapter is hit — mirrors the chapter DragValue.
    pub fn set_last_chapter(&mut self, fic_id: u64, chapter: u32) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        let updated = update_chapters::update_last_chapter_read(&repo, fic_id, chapter)?;
        replace_in_cache(&mut self.fics, updated);
        Ok(())
    }

    /// Update the read counter. Mirrors the Reads DragValue.
    pub fn set_read_count(&mut self, fic_id: u64, count: u32) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        let updated = update_read_count::update_read_count(&repo, fic_id, count)?;
        replace_in_cache(&mut self.fics, updated);
        Ok(())
    }

    /// Update the user's 5-star rating. `None` clears it. Mirrors the
    /// star widget in the Your Info section.
    pub fn set_user_rating(
        &mut self,
        fic_id: u64,
        rating: Option<UserRating>,
    ) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        let updated = update_rating::update_user_rating(&repo, fic_id, rating_payload(rating))?;
        replace_in_cache(&mut self.fics, updated);
        Ok(())
    }

    /// Update the personal note. `None` removes it. Mirrors the Notes
    /// TextEdit's commit-on-focus-loss behaviour.
    pub fn set_note(&mut self, fic_id: u64, note: Option<&str>) -> Result<(), FicflowError> {
        let repo = SqliteRepository::new(&self.connection);
        let updated = update_note::update_personal_note(&repo, fic_id, note)?;
        replace_in_cache(&mut self.fics, updated);
        Ok(())
    }

    fn save_config(&self) {
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save config: {}", err);
        }
    }

    /// First frame: re-apply the maximized/fullscreen state we saw last
    /// session via `ViewportCommand`. Subsequent frames: watch the live
    /// viewport state and persist it when the user toggles. The saved
    /// flags survive even when eframe's own window persistence drops them
    /// (notably: eframe 0.29 doesn't track `maximized`).
    fn sync_window_state(&mut self, ctx: &egui::Context) {
        if !self.initial_window_state_applied {
            if self.config.window_fullscreen {
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
            } else if self.config.window_maximized {
                ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
            }
            self.initial_window_state_applied = true;
            return;
        }

        let (maximized, fullscreen) = ctx.input(|i| {
            (
                i.viewport().maximized.unwrap_or(false),
                i.viewport().fullscreen.unwrap_or(false),
            )
        });
        if maximized != self.config.window_maximized || fullscreen != self.config.window_fullscreen
        {
            self.config.window_maximized = maximized;
            self.config.window_fullscreen = fullscreen;
            self.save_config();
        }
    }

    fn refresh_shelf_members(&mut self) {
        self.shelf_members = match self.current_view {
            View::Shelf(id) => {
                let repo = SqliteRepository::new(&self.connection);
                match list_shelf_fics::list_shelf_fics(&repo, id) {
                    Ok(fics) => fics.into_iter().map(|f| f.id).collect(),
                    Err(err) => {
                        self.toasts
                            .error(format!("Couldn't load shelf contents: {}", err));
                        HashSet::new()
                    }
                }
            }
            _ => HashSet::new(),
        };
    }

    /// Drop selected fic ids that aren't visible in the current view, so the
    /// details panel never shows a fic the user can't see in the table.
    /// On non-library views (Tasks/Settings) the selection is cleared entirely.
    fn prune_selection_to_view(&mut self) {
        let before = self.selection.clone();

        if !self.current_view.shows_library() {
            self.selection = Selection::None;
            self.last_clicked_id = None;
        } else {
            let visible_ids: Vec<u64> = match &self.selection {
                Selection::None => Vec::new(),
                Selection::Single(id) => self
                    .fics
                    .iter()
                    .find(|f| f.id == *id)
                    .filter(|f| self.current_view.includes(f, &self.shelf_members))
                    .map(|f| f.id)
                    .into_iter()
                    .collect(),
                Selection::Multi(ids) => ids
                    .iter()
                    .filter_map(|id| {
                        self.fics
                            .iter()
                            .find(|f| f.id == *id)
                            .filter(|f| self.current_view.includes(f, &self.shelf_members))
                            .map(|f| f.id)
                    })
                    .collect(),
            };
            self.selection = match visible_ids.len() {
                0 => Selection::None,
                1 => Selection::Single(visible_ids[0]),
                _ => Selection::Multi(visible_ids),
            };
            if matches!(self.selection, Selection::None) {
                self.last_clicked_id = None;
            }
        }

        // Refresh the per-fic shelf-membership cache here too: the regular
        // post-library-view diff captures `prev_selection` *after* this prune
        // runs, so it won't notice changes we made above.
        if self.selection != before {
            self.refresh_selection_shelf_ids();
        }
    }

    fn refresh_shelf_counts(&mut self) {
        self.shelf_counts = compute_shelf_counts(&self.connection, &self.shelves);
    }

    fn refresh_selection_shelf_ids(&mut self) {
        self.selection_shelf_ids = match self.selection {
            Selection::Single(id) => {
                let repo = SqliteRepository::new(&self.connection);
                match list_shelves_for_fic::list_shelves_for_fic(&repo, id) {
                    Ok(shelves) => shelves.into_iter().map(|s| s.id).collect(),
                    Err(err) => {
                        self.toasts
                            .error(format!("Couldn't load shelves for fic: {}", err));
                        HashSet::new()
                    }
                }
            }
            _ => HashSet::new(),
        };
    }
}

/// AO3 URL list + retry-cycle count, identical to the CLI's logic in main.rs.
/// `AO3_BASE_URL` pins to a single URL with extra cycles (used by integration
/// tests); otherwise we round-robin the primary, alt, and proxy URLs.
fn ao3_config_from_env() -> (Vec<String>, u32) {
    match std::env::var("AO3_BASE_URL") {
        Ok(url) => (vec![url], 3),
        Err(_) => (
            vec![
                PRIMARY_AO3_URL.to_string(),
                ALT_AO3_URL.to_string(),
                PROXY_AO3_URL.to_string(),
            ],
            2,
        ),
    }
}

fn load_fics(connection: &Connection) -> Vec<Fanfiction> {
    let repo = SqliteRepository::new(connection);
    match list_fics::list_fics(&repo) {
        Ok(fics) => fics,
        Err(err) => {
            log::error!("Failed to load fanfictions: {}", err);
            Vec::new()
        }
    }
}

fn load_shelves(connection: &Connection) -> Vec<Shelf> {
    let repo = SqliteRepository::new(connection);
    match list_shelves::list_shelves(&repo) {
        Ok(shelves) => shelves,
        Err(err) => {
            log::error!("Failed to load shelves: {}", err);
            Vec::new()
        }
    }
}

/// One COUNT query per shelf — fine for the small number of shelves users
/// actually have. Failures are silently swallowed (the row just shows 0)
/// because a transient DB hiccup shouldn't replace the whole sidebar with
/// an error toast on every frame.
fn compute_shelf_counts(connection: &Connection, shelves: &[Shelf]) -> HashMap<u64, usize> {
    let repo = SqliteRepository::new(connection);
    shelves
        .iter()
        .filter_map(|s| count_fics_in_shelf(&repo, s.id).ok().map(|n| (s.id, n)))
        .collect()
}

/// Replace the entry with the same `id` in `fics` with `updated`. No-op
/// if no entry matches (which would mean the cache is out of sync with
/// the DB — a harmless transient).
fn replace_in_cache(fics: &mut [Fanfiction], updated: Fanfiction) {
    if let Some(slot) = fics.iter_mut().find(|f| f.id == updated.id) {
        *slot = updated;
    }
}

/// Canonical CLI/application-layer payload string for a status. The
/// application functions accept a string for parity with the CLI and
/// JSON config; this helper keeps the conversion in one place.
fn status_payload(status: ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "inprogress",
        ReadingStatus::Read => "read",
        ReadingStatus::PlanToRead => "plantoread",
        ReadingStatus::Paused => "paused",
        ReadingStatus::Abandoned => "abandoned",
    }
}

fn rating_payload(rating: Option<UserRating>) -> &'static str {
    match rating {
        Some(UserRating::One) => "1",
        Some(UserRating::Two) => "2",
        Some(UserRating::Three) => "3",
        Some(UserRating::Four) => "4",
        Some(UserRating::Five) => "5",
        None => "none",
    }
}

fn compute_library_counts(fics: &[Fanfiction]) -> LibraryCounts {
    let mut counts = LibraryCounts {
        all: fics.len(),
        ..LibraryCounts::default()
    };
    for f in fics {
        match f.reading_status {
            ReadingStatus::InProgress => counts.in_progress += 1,
            ReadingStatus::Read => counts.read += 1,
            ReadingStatus::PlanToRead => counts.plan_to_read += 1,
            ReadingStatus::Paused => counts.paused += 1,
            ReadingStatus::Abandoned => counts.abandoned += 1,
        }
    }
    counts
}

impl eframe::App for FicflowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.render(ctx);
    }
}

impl FicflowApp {
    /// Headless-friendly entry point — the actual per-frame body lives
    /// here so tests can drive the app without an `eframe::Frame` (which
    /// can't be constructed outside the eframe runtime). The eframe
    /// `App::update` impl above is a thin delegate.
    pub fn render(&mut self, ctx: &egui::Context) {
        self.sync_window_state(ctx);

        // Keyboard shortcuts run before the rest of the UI so reactions
        // (focus changes, modal opens, selection mutation) take effect this
        // same frame.
        self.handle_shortcuts(ctx);

        // Brand header. The view title used to live here too but it now sits
        // inside the central panel (closer to the search bar / action buttons
        // it relates to), so this row is just the wordmark.
        egui::TopBottomPanel::top("ficflow-header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("FICFLOW");
            });
            ui.add_space(4.0);
        });

        // Sidebar.
        let mut create_request = false;
        let mut drop_on_shelf: Option<(u64, Vec<u64>)> = None;
        let prev_view = self.current_view.clone();
        let library_counts = compute_library_counts(&self.fics);
        egui::SidePanel::left("ficflow-sidebar")
            .default_width(160.0)
            .width_range(140.0..=600.0)
            .resizable(true)
            .show(ctx, |ui| {
                sidebar::draw(
                    ui,
                    SidebarState {
                        current_view: &mut self.current_view,
                        shelves: &self.shelves,
                        library_counts: &library_counts,
                        shelf_counts: &self.shelf_counts,
                        running_tasks: self.task_executor.running_count(),
                        create_shelf_request: &mut create_request,
                        delete_shelf_request: &mut self.delete_shelf_pending,
                        drop_on_shelf: &mut drop_on_shelf,
                    },
                );
            });
        if create_request {
            self.create_shelf_modal.request_open();
        }
        if let Some((shelf_id, fic_ids)) = drop_on_shelf {
            self.handle_drop_on_shelf(shelf_id, &fic_ids);
        }
        // If the view changed: refresh the shelf-membership cache (if needed)
        // and prune the selection so the details panel doesn't keep showing
        // a fic that's no longer visible.
        if self.current_view != prev_view {
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            } else {
                self.shelf_members.clear();
            }
            self.prune_selection_to_view();
        }

        // Details panel (right). Shown only when exactly one fic is
        // selected in a library view — multi-select doesn't make sense
        // here (no single fic to detail) and Tasks/Settings views have
        // their own central content. ~2x the sidebar's default width.
        let mut shelves_changed = false;
        let show_details =
            matches!(self.selection, Selection::Single(_)) && self.current_view.shows_library();
        if show_details {
            egui::SidePanel::right("ficflow-details")
                .default_width(320.0)
                .width_range(280.0..=900.0)
                .resizable(true)
                .show(ctx, |ui| {
                    shelves_changed = details_panel::draw(
                        ui,
                        DetailsState {
                            selection: &self.selection,
                            fics: &mut self.fics,
                            conn: &self.connection,
                            toasts: &mut self.toasts,
                            all_shelves: &self.shelves,
                            selection_shelf_ids: &self.selection_shelf_ids,
                            task_executor: &self.task_executor,
                        },
                    );
                });
        }

        // Selection bar (bottom of the central area). Shown whenever there's
        // an active selection — both single and multi — so the user can act
        // on the selection without forcing a multi-select first.
        let mut bulk_shelves_changed = false;
        if !matches!(self.selection, Selection::None) && self.current_view.shows_library() {
            egui::TopBottomPanel::bottom("ficflow-selection-bar")
                .resizable(false)
                .show(ctx, |ui| {
                    bulk_shelves_changed = selection_bar::draw(
                        ui,
                        SelectionBarState {
                            selection: &mut self.selection,
                            fics: &mut self.fics,
                            conn: &self.connection,
                            toasts: &mut self.toasts,
                            current_view: &self.current_view,
                            all_shelves: &self.shelves,
                            delete_pending: &mut self.delete_fics_pending,
                        },
                    );
                });
        }

        // Center.
        let mut sort_changed = false;
        let prev_selection = self.selection.clone();
        let view_title = self.current_view.header_title(&self.shelves);
        let central = egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_central_header(ui, &view_title);
            ui.add_space(6.0);
            if self.current_view.shows_library() {
                sort_changed = library_view::draw(
                    ui,
                    LibraryViewState {
                        fics: &self.fics,
                        sort: &mut self.sort,
                        search_query: &self.search_query,
                        visible_columns: &self.config.visible_columns,
                        selection: &mut self.selection,
                        view: &self.current_view,
                        shelf_members: &self.shelf_members,
                        last_clicked_id: &mut self.last_clicked_id,
                    },
                );
            } else if matches!(self.current_view, View::Tasks) {
                tasks_view::draw(
                    ui,
                    TasksViewState {
                        executor: &self.task_executor,
                        filter: &mut self.task_filter,
                    },
                );
            } else if matches!(self.current_view, View::Settings) {
                settings_view::draw(ui);
            } else {
                draw_stub_view(ui, &self.current_view);
            }
        });
        if central.response.clicked() && self.current_view.shows_library() {
            self.selection = Selection::None;
        }
        if self.selection != prev_selection {
            self.refresh_selection_shelf_ids();
        }
        if shelves_changed || bulk_shelves_changed {
            // Refresh fic-shelf link cache, plus the shelf-view membership
            // cache if the affected shelf happens to be the active view.
            // Sidebar counts also depend on shelf membership.
            self.refresh_selection_shelf_ids();
            self.refresh_shelf_counts();
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            }
        }

        // If the fic that was selected got deleted (e.g. via the details
        // panel's "Delete Fic" button), drop the now-invalid selection so
        // the panel doesn't render a "not found" state next frame.
        if let Selection::Single(id) = self.selection {
            if !self.fics.iter().any(|f| f.id == id) {
                self.selection = Selection::None;
                self.last_clicked_id = None;
            }
        }

        // Modals (run after the rest of the UI so they overlay correctly).
        column_picker::show(
            ctx,
            &mut self.show_column_picker,
            &mut self.config.visible_columns,
        );
        match shelf_modals::draw_create(ctx, &mut self.create_shelf_modal) {
            shelf_modals::Outcome::Submit(name) => {
                let _ = self.create_shelf(name);
            }
            shelf_modals::Outcome::Cancel | shelf_modals::Outcome::None => {}
        }
        match shelf_modals::draw_delete_confirm(ctx, &mut self.delete_shelf_pending, &self.shelves)
        {
            shelf_modals::DeleteOutcome::Confirm(id) => {
                let _ = self.delete_shelf(id);
            }
            shelf_modals::DeleteOutcome::Cancel | shelf_modals::DeleteOutcome::None => {}
        }
        match bulk_modals::draw_delete_confirm(ctx, &mut self.delete_fics_pending, &self.fics) {
            bulk_modals::DeleteOutcome::Confirm(ids) => self.handle_bulk_delete(&ids),
            bulk_modals::DeleteOutcome::Cancel | bulk_modals::DeleteOutcome::None => {}
        }
        match add_fic_dialog::draw(ctx, &mut self.add_fic_modal) {
            add_fic_dialog::Outcome::Submit(input) => {
                self.task_executor.enqueue_add(input);
                // Stay on the user's current view — the toast on success
                // (or "Failed" tab in Tasks) tells them the result; jumping
                // away is jarring.
            }
            add_fic_dialog::Outcome::Cancel | add_fic_dialog::Outcome::None => {}
        }

        // Pull in newly-added fics from the worker and toast each completion.
        let completions = self.task_executor.take_completions();
        if !completions.is_empty() {
            for title in &completions {
                self.toasts
                    .success(format!("Added \u{201C}{}\u{201D}", title));
            }
            self.fics = load_fics(&self.connection);
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            }
            self.refresh_selection_shelf_ids();
        }

        // Same drill for refreshes — reload so the new metadata and the
        // bumped `last_checked_date` show in the details panel.
        let refreshes = self.task_executor.take_refreshes();
        if !refreshes.is_empty() {
            self.toasts
                .success(format!("Refreshed {} fanfiction(s)", refreshes.len()));
            self.fics = load_fics(&self.connection);
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            }
        }
        // Keep painting while a task is running so the spinner animates and
        // task age strings tick over without requiring user input.
        if self.task_executor.has_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        if sort_changed {
            self.config.default_sort = self.sort;
            self.save_config();
        }
        // visible_columns may have changed via the picker; save unconditionally
        // when picker output isn't tracked separately. (Picker writes to the
        // same Vec we hand it; cheap to over-save here, but kept off the hot
        // path by only saving when the picker is open.)
        if self.show_column_picker {
            self.save_config();
        }

        self.draw_drag_preview(ctx);
        self.toasts.show(ctx);
    }
}

impl FicflowApp {
    /// Header row at the top of the central panel: view title + (for library
    /// views) a visible-fic count, the search bar, and the +Add Fic / Manage
    /// Columns buttons. For Tasks/Settings the row collapses to just the title.
    fn draw_central_header(&mut self, ui: &mut egui::Ui, view_title: &str) {
        ui.horizontal(|ui| {
            ui.heading(view_title);
            if !self.current_view.shows_library() {
                return;
            }
            let visible = library_view::visible_count(
                &self.fics,
                &self.current_view,
                &self.shelf_members,
                &self.search_query,
            );
            let suffix = if visible == 1 { "fic" } else { "fics" };
            ui.label(egui::RichText::new(format!("{} {}", visible, suffix)).weak());
            // Lay out from the right so the buttons hug the right edge,
            // then centre the search bar in whatever horizontal space
            // remains between the count and the leftmost button.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Manage Columns").clicked() {
                    self.show_column_picker = !self.show_column_picker;
                }
                if ui.button("+ Add Fic").clicked() {
                    self.add_fic_modal.request_open();
                }
                let search_w = 300.0;
                let pad = ((ui.available_width() - search_w) / 2.0).max(0.0);
                // In right_to_left, add_space *before* the next item moves
                // the cursor leftwards — i.e. it leaves whitespace to the
                // RIGHT of the next item. Combined with the same-sized
                // empty stretch that ends up on the LEFT (avail - search - pad),
                // this centres the search bar.
                ui.add_space(pad);
                self.draw_search_field(ui);
            });
        });
    }

    /// Search field rendered as a fixed-width Frame containing a static
    /// magnifying-glass glyph followed by a borderless TextEdit, so the
    /// icon sits *inside* the apparent input boundary and stays visible
    /// even while the user is typing (unlike a hint text).
    fn draw_search_field(&mut self, ui: &mut egui::Ui) {
        const WIDTH: f32 = 300.0;
        let stroke = ui.visuals().widgets.inactive.bg_stroke;
        let fill = ui.visuals().extreme_bg_color;
        let weak = ui.visuals().weak_text_color();

        ui.allocate_ui(egui::vec2(WIDTH, 22.0), |ui| {
            egui::Frame::default()
                .fill(fill)
                .stroke(stroke)
                .rounding(2.0)
                .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                .show(ui, |ui| {
                    // Force left_to_right explicitly: `ui.horizontal()`
                    // copies the parent's "prefer_right_to_left" flag, so
                    // when this is rendered inside the right_to_left
                    // button row the icon would otherwise end up on the
                    // right of the TextEdit instead of the left.
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        ui.label(egui::RichText::new("\u{1F50D}").color(weak));
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search title, author, fandom, tags…")
                                .frame(false)
                                .desired_width(f32::INFINITY),
                        );
                        if self.focus_search_pending {
                            resp.request_focus();
                            self.focus_search_pending = false;
                        }
                    });
                });
        });
    }
}

impl FicflowApp {
    /// Renders a small popup near the cursor showing what's being dragged
    /// (the title of the single fic, or "N fanfictions" for a multi-drag).
    /// egui's built-in dnd doesn't ship a drag-preview, so we paint one
    /// ourselves whenever there's an active payload of our type.
    fn draw_drag_preview(&self, ctx: &egui::Context) {
        let Some(payload) = egui::DragAndDrop::payload::<Vec<u64>>(ctx) else {
            return;
        };
        let Some(pointer) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        let label = match payload.as_slice() {
            [single] => self
                .fics
                .iter()
                .find(|f| f.id == *single)
                .map(|f| f.title.clone())
                .unwrap_or_else(|| "(unknown)".to_string()),
            ids => format!("{} fanfictions", ids.len()),
        };
        egui::Area::new(egui::Id::new("ficflow-drag-preview"))
            .fixed_pos(pointer + egui::Vec2::new(14.0, 14.0))
            .order(egui::Order::Tooltip)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(260.0);
                    ui.add(egui::Label::new(label).truncate());
                });
            });
    }
}

impl FicflowApp {
    fn handle_drop_on_shelf(&mut self, shelf_id: u64, fic_ids: &[u64]) {
        let repo = SqliteRepository::new(&self.connection);
        let mut errors = 0usize;
        for id in fic_ids {
            if add_to_shelf(&repo, *id, shelf_id).is_err() {
                errors += 1;
            }
        }
        let attempted = fic_ids.len();
        if errors == 0 {
            self.toasts
                .success(format!("Added {} fanfiction(s) to shelf", attempted));
        } else if errors == attempted {
            self.toasts.error(format!("All {} drops failed", attempted));
        } else {
            self.toasts
                .error(format!("{}/{} drops failed", errors, attempted));
        }
        self.refresh_selection_shelf_ids();
        self.refresh_shelf_counts();
        if matches!(self.current_view, View::Shelf(_)) {
            self.refresh_shelf_members();
        }
    }

    fn handle_bulk_delete(&mut self, ids: &[u64]) {
        let repo = SqliteRepository::new(&self.connection);
        let mut errors = 0usize;
        for id in ids {
            if delete_fic::delete_fic(&repo, *id).is_err() {
                errors += 1;
            } else {
                self.fics.retain(|f| f.id != *id);
            }
        }
        if errors == 0 {
            self.toasts
                .success(format!("Deleted {} fanfictions", ids.len()));
        } else {
            self.toasts
                .error(format!("{}/{} deletions failed", errors, ids.len()));
        }
        // Clear selection now that the underlying fics are gone, and refresh
        // shelf caches because deletes cascade through `fic_shelf`.
        self.selection = Selection::None;
        self.last_clicked_id = None;
        self.refresh_selection_shelf_ids();
        self.refresh_shelf_counts();
        if matches!(self.current_view, View::Shelf(_)) {
            self.refresh_shelf_members();
        }
    }
}

fn draw_stub_view(ui: &mut egui::Ui, _view: &View) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Nothing to show here yet.")
            .italics()
            .weak(),
    );
}

impl FicflowApp {
    /// Application-wide keyboard shortcuts. Skipped while a text edit has
    /// focus so we don't fight the user's typing (Ctrl+A in a TextEdit, for
    /// instance, should select the text, not all rows).
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        use egui::{Key, KeyboardShortcut, Modifiers};

        let pressed_esc = ctx.input(|i| i.key_pressed(Key::Escape));
        let pressed_delete = ctx.input(|i| i.key_pressed(Key::Delete));
        let ctrl_a = ctx
            .input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::A)));
        let ctrl_n = ctx
            .input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::N)));
        let ctrl_f = ctx
            .input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::F)));

        if pressed_esc && !matches!(self.selection, Selection::None) {
            self.selection = Selection::None;
            self.last_clicked_id = None;
            self.refresh_selection_shelf_ids();
        }

        if pressed_delete && self.current_view.shows_library() {
            let ids = match &self.selection {
                Selection::Single(id) => vec![*id],
                Selection::Multi(ids) => ids.clone(),
                Selection::None => Vec::new(),
            };
            if !ids.is_empty() {
                self.delete_fics_pending = Some(ids);
            }
        }

        if ctrl_a && self.current_view.shows_library() {
            let ids = library_view::visible_ids(
                &self.fics,
                &self.current_view,
                &self.shelf_members,
                &self.search_query,
                self.sort,
            );
            self.selection = match ids.len() {
                0 => Selection::None,
                1 => Selection::Single(ids[0]),
                _ => Selection::Multi(ids),
            };
            self.refresh_selection_shelf_ids();
        }

        if ctrl_n {
            self.create_shelf_modal.request_open();
        }

        if ctrl_f && self.current_view.shows_library() {
            self.focus_search_pending = true;
        }
    }
}
