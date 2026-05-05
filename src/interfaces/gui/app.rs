use std::path::PathBuf;

use egui_notify::Toasts;
use rusqlite::Connection;

use super::config::{AppConfig, ColumnKey, SortDirection, SortPref};
use crate::application::{
    add_to_shelf::add_to_shelf, create_shelf::create_shelf, delete_fic, delete_shelf,
    remove_from_shelf, update_chapters, update_note, update_rating, update_read_count,
    update_status,
};
use crate::domain::fanfiction::{Fanfiction, ReadingStatus, UserRating};
use crate::domain::shelf::Shelf;
use crate::error::FicflowError;
use crate::infrastructure::external::ao3::fetcher::ao3_urls_from_env;
use crate::infrastructure::persistence::database::connection::{
    establish_connection, open_configured_db,
};
use crate::infrastructure::SqliteRepository;

use super::fonts;
use super::library_cache::LibraryCache;
use super::selection::Selection;
use super::selection_controller::SelectionController;
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
    /// All in-memory caches (`fics`, `shelves`, `shelf_members`,
    /// `selection_shelf_ids`, `shelf_counts`) plus the `mutate()`
    /// funnel that keeps them coherent. See `LibraryCache` for the
    /// invariants each field upholds.
    cache: LibraryCache,
    config: AppConfig,
    /// Set after the first `update()` applies the persisted maximized /
    /// fullscreen state via `ViewportCommand`. Without this gate the
    /// command would re-fire every frame.
    initial_window_state_applied: bool,
    sort: SortPref,
    search_query: String,
    show_column_picker: bool,
    /// Library-table selection state + the row-click resolver. See
    /// `SelectionController` for how clicks/modifiers map to deltas.
    selection: SelectionController,
    current_view: View,
    /// Which (at most one) modal window is currently displayed. The
    /// enum guarantees exclusivity at the type level — opening any
    /// modal first overwrites the previous one, so two stacked
    /// windows can never happen.
    active_modal: ActiveModal,
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

/// Mutually-exclusive set of modal windows. Replaces what used to be
/// four independent fields (`create_shelf_modal`, `delete_shelf_pending`,
/// `delete_fics_pending`, `add_fic_modal`) — the enum form makes "at
/// most one modal open" a property the type system enforces, instead
/// of an invariant the code has to maintain by convention.
pub enum ActiveModal {
    None,
    /// Create-shelf dialog; payload is the in-progress name buffer.
    CreateShelf(CreateState),
    /// Confirmation modal before soft-deleting a shelf; payload is its id.
    DeleteShelf(u64),
    /// Confirmation modal before bulk-soft-deleting fics; payload is
    /// the list of ids the user selected.
    DeleteFics(Vec<u64>),
    /// Add-fic dialog; payload is the in-progress URL/ID buffer.
    AddFic(AddFicState),
}

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
        let (ao3_urls, max_retry_cycles) = ao3_urls_from_env();
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
        let cache = LibraryCache::load(&connection);
        let app_config = AppConfig::load();
        let sort = app_config.default_sort;
        let task_executor = TaskExecutor::spawn(
            config.ao3_urls,
            config.max_retry_cycles,
            config.db_path.clone(),
        );
        Ok(Self {
            connection,
            cache,
            config: app_config,
            initial_window_state_applied: false,
            sort,
            search_query: String::new(),
            show_column_picker: false,
            selection: SelectionController::new(),
            current_view: View::default(),
            active_modal: ActiveModal::None,
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
        &self.cache.fics
    }

    /// In-memory cache of all non-deleted shelves.
    pub fn shelves(&self) -> &[Shelf] {
        &self.cache.shelves
    }

    pub fn selection(&self) -> &Selection {
        self.selection.current()
    }

    /// Cached shelf-ids that the currently-selected single fic belongs
    /// to. Empty unless the selection is `Single(_)`. Exposed so tests
    /// can verify cache invariants — production callers should read
    /// `cache.selection_shelf_ids` through the dropdown's state struct.
    pub fn selection_shelves(&self) -> &std::collections::HashSet<u64> {
        &self.cache.selection_shelf_ids
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
            &self.cache.fics,
            &self.current_view,
            &self.cache.shelf_members,
            &self.search_query,
            self.sort,
        )
    }

    /// Whether the right-hand details panel is currently mounted.
    /// Mirrors the gating in `update()` — only on a single-fic
    /// selection inside a library view.
    pub fn details_panel_visible(&self) -> bool {
        matches!(self.selection.current(), Selection::Single(_))
            && self.current_view.shows_library()
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
    //
    // Important: these methods bypass the *widget* code path that
    // dispatches the application call (combo box → ReadingStatus,
    // drag-value → u32, star widget → Option<UserRating>). A test
    // that exercises `set_status(id, ReadingStatus::Read)` proves
    // that `update_reading_status` lands in the DB; it does NOT
    // prove that the status combo's selection mapping is correct.
    // We accept that gap on purpose: the per-widget glue is small
    // enough that human verification suffices, and a real
    // event-injection test harness would require either bumping to
    // egui_kittest 0.30+ (incompatible with the pinned 0.29 chrome
    // work) or a from-scratch raw-pointer-event simulator.

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
        self.selection.select_single(fic_id);
        self.refresh_selection_shelf_ids();
    }

    /// Set the selection to a list of fic ids. The variant is chosen
    /// from the slice length: empty → None, one → Single, more →
    /// Multi. Equivalent to the result of a shift-click range or a
    /// series of ctrl-click toggles.
    pub fn select_fics(&mut self, ids: &[u64]) {
        self.selection.select_many(ids);
        self.refresh_selection_shelf_ids();
    }

    /// Drop the current selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.cache.selection_shelf_ids.clear();
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
        if let Selection::Single(id) = *self.selection.current() {
            if let Some(fic) = self.cache.fics.iter().find(|f| f.id == id) {
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
        let ids = self.selection.ids_vec();
        if ids.is_empty() {
            return;
        }
        let surviving: Vec<u64> = self.mutate(|repo| {
            ids.iter()
                .filter_map(|id| match delete_fic::delete_fic(repo, *id) {
                    Ok(()) => Some(*id),
                    Err(_) => None,
                })
                .collect()
        });
        self.cache.remove_fics(&surviving);
        self.clear_selection();
    }

    /// Create a new shelf and refresh the in-memory caches. Mirrors the
    /// Create-Shelf modal's submit. The application layer rejects empty
    /// or whitespace-only names; toasts are surfaced either way for
    /// parity with the GUI flow.
    pub fn create_shelf(&mut self, name: impl AsRef<str>) -> Result<(), FicflowError> {
        let repo = self.repo();
        match create_shelf(&repo, name.as_ref()) {
            Ok(shelf) => {
                self.toasts
                    .success(format!("Created shelf \u{201C}{}\u{201D}", shelf.name));
                self.cache.reload_shelves(&self.connection);
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
        let repo = self.repo();
        match delete_shelf::delete_shelf(&repo, shelf_id) {
            Ok(()) => {
                self.toasts.success("Shelf deleted");
                if self.current_view == View::Shelf(shelf_id) {
                    self.current_view = View::AllFics;
                    self.cache.shelf_members.clear();
                }
                self.cache.reload_shelves(&self.connection);
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
        self.mutate(|repo| add_to_shelf(repo, fic_id, shelf_id))
    }

    /// Remove a fic from a shelf. Mirrors clicking the × on a shelf
    /// chip in the details-panel dropdown.
    pub fn remove_fic_from_shelf(
        &mut self,
        fic_id: u64,
        shelf_id: u64,
    ) -> Result<(), FicflowError> {
        self.mutate(|repo| remove_from_shelf::remove_from_shelf(repo, fic_id, shelf_id))
    }

    /// Bulk set status across many fics. Mirrors the "Change status"
    /// menu in the selection bar. Returns `(succeeded, failed)`.
    ///
    /// Unlike `bulk_add_to_shelf` / `bulk_remove_from_shelf` this does
    /// NOT route through `mutate()` — status changes can't affect
    /// shelf membership, so the per-call refresh of `shelf_counts` /
    /// `shelf_members` / `selection_shelf_ids` would be wasted work.
    /// Just patch the in-memory fics from each successful call.
    pub fn bulk_set_status(&mut self, ids: &[u64], status: ReadingStatus) -> (usize, usize) {
        let mut errors = 0usize;
        let mut updated_fics: Vec<Fanfiction> = Vec::with_capacity(ids.len());
        let repo = self.repo();
        for id in ids {
            match update_status::update_reading_status(&repo, *id, status) {
                Ok(updated) => updated_fics.push(updated),
                Err(_) => errors += 1,
            }
        }
        for fic in updated_fics {
            self.cache.replace_fic(fic);
        }
        (ids.len() - errors, errors)
    }

    /// Bulk add many fics to one shelf. Mirrors the "Add to shelf"
    /// menu in the selection bar. Returns `(succeeded, failed)`.
    pub fn bulk_add_to_shelf(&mut self, ids: &[u64], shelf_id: u64) -> (usize, usize) {
        let errors = self.mutate(|repo| {
            ids.iter()
                .filter(|id| add_to_shelf(repo, **id, shelf_id).is_err())
                .count()
        });
        (ids.len() - errors, errors)
    }

    /// Bulk remove many fics from one shelf. Mirrors the "Remove from
    /// shelf" button (only shown inside a shelf view). Returns
    /// `(succeeded, failed)`.
    pub fn bulk_remove_from_shelf(&mut self, ids: &[u64], shelf_id: u64) -> (usize, usize) {
        let errors = self.mutate(|repo| {
            ids.iter()
                .filter(|id| remove_from_shelf::remove_from_shelf(repo, **id, shelf_id).is_err())
                .count()
        });
        (ids.len() - errors, errors)
    }

    /// Set the search query. Mirrors typing into the search bar.
    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    /// Update the reading status of a fic. Mirrors the Status combo
    /// in the Your Info section.
    pub fn set_status(&mut self, fic_id: u64, status: ReadingStatus) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_status::update_reading_status(&repo, fic_id, status)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    /// Update the last-chapter-read marker. The application layer
    /// clamps to `chapters_total` and may auto-bump status / read_count
    /// when the final chapter is hit — mirrors the chapter DragValue.
    pub fn set_last_chapter(&mut self, fic_id: u64, chapter: u32) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_chapters::update_last_chapter_read(&repo, fic_id, chapter)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    /// Update the read counter. Mirrors the Reads DragValue.
    pub fn set_read_count(&mut self, fic_id: u64, count: u32) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_read_count::update_read_count(&repo, fic_id, count)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    /// Update the user's 5-star rating. `None` clears it. Mirrors the
    /// star widget in the Your Info section.
    pub fn set_user_rating(
        &mut self,
        fic_id: u64,
        rating: Option<UserRating>,
    ) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_rating::update_user_rating(&repo, fic_id, rating)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    /// Update the personal note. `None` removes it. Mirrors the Notes
    /// TextEdit's commit-on-focus-loss behaviour.
    pub fn set_note(&mut self, fic_id: u64, note: Option<&str>) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_note::update_personal_note(&repo, fic_id, note)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    fn save_config(&self) {
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save config: {}", err);
        }
    }

    /// Apply a single `details_panel::Outcome` for the fic with id
    /// `fic_id`. Single home for the toast-on-error behavior every
    /// widget used to inline. Mutating outcomes that change shelf
    /// membership (`AddToShelf`, `RemoveFromShelf`, `RequestDelete`)
    /// route through `self.mutate()` / `delete_selected()`, which
    /// already invalidates every dependent cache via the `LibraryCache`
    /// funnel — so this dispatch doesn't need to signal "refresh" back
    /// to the caller.
    fn dispatch_details_outcome(&mut self, fic_id: u64, outcome: details_panel::Outcome) {
        use details_panel::Outcome;
        match outcome {
            Outcome::None => {}
            Outcome::SetStatus(status) => {
                if let Err(err) = self.set_status(fic_id, status) {
                    self.toasts
                        .error(format!("Couldn't update status: {}", err));
                }
            }
            Outcome::SetLastChapter(n) => {
                if let Err(err) = self.set_last_chapter(fic_id, n) {
                    self.toasts
                        .error(format!("Couldn't update chapter: {}", err));
                }
            }
            Outcome::SetReadCount(n) => {
                if let Err(err) = self.set_read_count(fic_id, n) {
                    self.toasts
                        .error(format!("Couldn't update read count: {}", err));
                }
            }
            Outcome::SetUserRating(rating) => {
                if let Err(err) = self.set_user_rating(fic_id, rating) {
                    self.toasts
                        .error(format!("Couldn't update rating: {}", err));
                }
            }
            Outcome::SetNote(value) => {
                if let Err(err) = self.set_note(fic_id, value.as_deref()) {
                    self.toasts.error(format!("Couldn't update note: {}", err));
                }
            }
            Outcome::AddToShelf(shelf_id) => {
                if let Err(err) = self.add_fic_to_shelf(fic_id, shelf_id) {
                    self.toasts.error(format!("Couldn't add to shelf: {}", err));
                }
            }
            Outcome::RemoveFromShelf(shelf_id) => {
                if let Err(err) = self.remove_fic_from_shelf(fic_id, shelf_id) {
                    self.toasts
                        .error(format!("Couldn't remove from shelf: {}", err));
                }
            }
            Outcome::RequestDelete => {
                self.delete_selected();
                self.toasts.success("Fanfiction deleted");
            }
            Outcome::RequestRefresh => {
                self.refresh_selected();
            }
        }
    }

    /// Toast helper for bulk-action results. Aggregates per-fic
    /// outcomes into one of three messages: all-succeeded ("X
    /// fanfictions"), all-failed ("All N updates failed"), or partial
    /// ("F/N updates failed"). Used by the selection-bar dispatcher
    /// and any future bulk caller.
    fn toast_bulk_result(&mut self, action: &str, succeeded: usize, failed: usize) {
        if failed == 0 {
            self.toasts
                .success(format!("{}: {} fanfictions", action, succeeded));
        } else if succeeded == 0 {
            self.toasts.error(format!("All {} updates failed", failed));
        } else {
            self.toasts
                .error(format!("{}/{} updates failed", failed, succeeded + failed));
        }
    }

    /// Cheap repo accessor. The previous code repeated
    /// `let repo = self.repo();` at ~20
    /// sites — this collapses that to `self.repo()`.
    fn repo(&self) -> SqliteRepository<'_> {
        SqliteRepository::new(&self.connection)
    }

    /// Thin wrapper around `LibraryCache::mutate` that toasts any
    /// refresh errors. `op` is called with a fresh `SqliteRepository`
    /// and its return value is forwarded; the cache handles the
    /// post-op invalidation of `selection_shelf_ids`, `shelf_counts`,
    /// and (when on a shelf view) `shelf_members`.
    fn mutate<R>(&mut self, op: impl FnOnce(&SqliteRepository<'_>) -> R) -> R {
        let (result, refresh_errors) = self.cache.mutate(
            &self.connection,
            &self.current_view,
            self.selection.current(),
            op,
        );
        for err in refresh_errors {
            self.toasts
                .error(format!("Couldn't refresh after change: {}", err));
        }
        result
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
        if let Err(err) = self
            .cache
            .refresh_shelf_members(&self.connection, &self.current_view)
        {
            self.toasts
                .error(format!("Couldn't load shelf contents: {}", err));
        }
    }

    /// Drop selected fic ids that aren't visible in the current view, so the
    /// details panel never shows a fic the user can't see in the table.
    /// On non-library views (Tasks/Settings) the selection is cleared entirely.
    fn prune_selection_to_view(&mut self) {
        let changed = self.selection.prune_to_view(
            &self.cache.fics,
            &self.current_view,
            &self.cache.shelf_members,
        );
        // Refresh the per-fic shelf-membership cache here too: the regular
        // post-library-view diff captures `prev_selection` *after* this prune
        // runs, so it won't notice changes we made above.
        if changed {
            self.refresh_selection_shelf_ids();
        }
    }

    fn refresh_shelf_counts(&mut self) {
        self.cache.refresh_shelf_counts(&self.connection);
    }

    fn refresh_selection_shelf_ids(&mut self) {
        if let Err(err) = self
            .cache
            .refresh_selection_shelf_ids(&self.connection, self.selection.current())
        {
            self.toasts
                .error(format!("Couldn't load shelves for fic: {}", err));
        }
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
    ///
    /// Each `paint_*` helper is responsible for one screen region and
    /// every side effect that originates from it (toasts, modal-state
    /// transitions, cache invalidation). Keeping render() as just a
    /// dispatch list lets a reader skim the per-frame timeline without
    /// having to read every paint body.
    pub fn render(&mut self, ctx: &egui::Context) {
        self.sync_window_state(ctx);
        // Keyboard shortcuts run before the rest of the UI so reactions
        // (focus changes, modal opens, selection mutation) take effect this
        // same frame.
        self.handle_shortcuts(ctx);
        self.paint_header(ctx);
        // Sidebar can change `current_view`; paint_sidebar handles the
        // post-change cache refresh + selection prune internally.
        self.paint_sidebar(ctx);
        self.paint_details_panel(ctx);
        self.paint_selection_bar(ctx);
        self.paint_central(ctx);
        self.paint_modals(ctx);
        self.drain_worker_events(ctx);
        self.draw_drag_preview(ctx);
        self.toasts.show(ctx);
    }

    /// Brand header. The view title used to live here too but it now sits
    /// inside the central panel (closer to the search bar / action buttons
    /// it relates to), so this row is just the wordmark.
    fn paint_header(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("ficflow-header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("FICFLOW");
            });
            ui.add_space(4.0);
        });
    }

    /// Left sidebar. Library counts + shelf list + Tasks/Settings stubs.
    /// The view returns one `Outcome` describing the user action this
    /// frame (modal request, drop, …); view-changes happen in-place on
    /// `current_view` and are detected via a prev/post diff. Also
    /// handles the view-change side effects (refresh shelf-members,
    /// prune the selection) here, since this is the only paint that
    /// can mutate `current_view`.
    fn paint_sidebar(&mut self, ctx: &egui::Context) {
        let prev_view = self.current_view.clone();
        let library_counts = compute_library_counts(&self.cache.fics);
        let mut outcome = sidebar::Outcome::None;
        egui::SidePanel::left("ficflow-sidebar")
            .default_width(160.0)
            .width_range(140.0..=600.0)
            .resizable(true)
            .show(ctx, |ui| {
                outcome = sidebar::draw(
                    ui,
                    SidebarState {
                        current_view: &mut self.current_view,
                        shelves: &self.cache.shelves,
                        library_counts: &library_counts,
                        shelf_counts: &self.cache.shelf_counts,
                        running_tasks: self.task_executor.running_count(),
                    },
                );
            });
        match outcome {
            sidebar::Outcome::None => {}
            sidebar::Outcome::OpenCreateShelfModal => {
                self.active_modal = ActiveModal::CreateShelf(CreateState::default());
            }
            sidebar::Outcome::OpenDeleteShelfConfirm(id) => {
                self.active_modal = ActiveModal::DeleteShelf(id);
            }
            sidebar::Outcome::DropOnShelf { shelf_id, fic_ids } => {
                self.handle_drop_on_shelf(shelf_id, &fic_ids);
            }
        }
        if self.current_view != prev_view {
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            } else {
                self.cache.shelf_members.clear();
            }
            self.prune_selection_to_view();
        }
    }

    /// Right details panel. Shown only when exactly one fic is selected
    /// in a library view — multi-select doesn't make sense here (no
    /// single fic to detail) and Tasks/Settings views have their own
    /// central content. ~2x the sidebar's default width.
    ///
    /// Pure presentation: panel returns an `Outcome` we dispatch
    /// through the control surface. This is the path that used to fork
    /// — widgets calling `update_*` directly versus tests calling
    /// `app.set_*` — collapsed into one.
    fn paint_details_panel(&mut self, ctx: &egui::Context) {
        let Selection::Single(id) = *self.selection.current() else {
            return;
        };
        if !self.current_view.shows_library() {
            return;
        }
        // Clone the fic so the immutable borrow on `self.cache.fics`
        // releases before we dispatch outcomes through `&mut self`.
        // Cheap: a few Vec<String> + small primitives.
        let Some(fic) = self.cache.fics.iter().find(|f| f.id == id).cloned() else {
            return;
        };
        let mut outcome = details_panel::Outcome::None;
        egui::SidePanel::right("ficflow-details")
            .default_width(320.0)
            .width_range(280.0..=900.0)
            .resizable(true)
            .show(ctx, |ui| {
                outcome = details_panel::draw(
                    ui,
                    DetailsState {
                        fic: &fic,
                        all_shelves: &self.cache.shelves,
                        selection_shelf_ids: &self.cache.selection_shelf_ids,
                    },
                );
            });
        self.dispatch_details_outcome(id, outcome);
    }

    /// Bottom selection bar. Shown whenever there's an active selection
    /// — both single and multi — so the user can act on the selection
    /// without forcing a multi-select first. Pure presentation: returns
    /// an `Outcome` we dispatch through the control surface.
    fn paint_selection_bar(&mut self, ctx: &egui::Context) {
        let selection_ids = self.selection.ids_vec();
        if selection_ids.is_empty() || !self.current_view.shows_library() {
            return;
        }
        let mut outcome = selection_bar::Outcome::None;
        egui::TopBottomPanel::bottom("ficflow-selection-bar")
            .resizable(false)
            .show(ctx, |ui| {
                outcome = selection_bar::draw(
                    ui,
                    SelectionBarState {
                        selection_ids: &selection_ids,
                        current_view: &self.current_view,
                        all_shelves: &self.cache.shelves,
                    },
                );
            });
        match outcome {
            selection_bar::Outcome::None => {}
            selection_bar::Outcome::SetStatus(status) => {
                let (succeeded, failed) = self.bulk_set_status(&selection_ids, status);
                self.toast_bulk_result("Status updated", succeeded, failed);
            }
            selection_bar::Outcome::AddToShelf(shelf_id) => {
                let (succeeded, failed) = self.bulk_add_to_shelf(&selection_ids, shelf_id);
                self.toast_bulk_result("Added to shelf", succeeded, failed);
            }
            selection_bar::Outcome::RemoveFromShelf(shelf_id) => {
                let (succeeded, failed) = self.bulk_remove_from_shelf(&selection_ids, shelf_id);
                self.toast_bulk_result("Removed from shelf", succeeded, failed);
            }
            selection_bar::Outcome::RequestDelete => {
                self.active_modal = ActiveModal::DeleteFics(selection_ids);
            }
            selection_bar::Outcome::ClearSelection => {
                self.clear_selection();
            }
        }
    }

    /// Central panel: header row + library/tasks/settings content.
    /// Also handles the empty-area click (clear selection), the
    /// post-render selection-shelf refresh, the orphan-selection
    /// cleanup (selected fic was deleted), and saves the sort pref
    /// when a header click changes it.
    fn paint_central(&mut self, ctx: &egui::Context) {
        let mut sort_changed = false;
        let prev_selection = self.selection.current().clone();
        let view_title = self.current_view.header_title(&self.cache.shelves);
        let central = egui::CentralPanel::default().show(ctx, |ui| {
            self.draw_central_header(ui, &view_title);
            ui.add_space(6.0);
            if self.current_view.shows_library() {
                sort_changed = library_view::draw(
                    ui,
                    LibraryViewState {
                        fics: &self.cache.fics,
                        sort: &mut self.sort,
                        search_query: &self.search_query,
                        visible_columns: &self.config.visible_columns,
                        selection: &mut self.selection,
                        view: &self.current_view,
                        shelf_members: &self.cache.shelf_members,
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
            self.clear_selection();
        }
        if *self.selection.current() != prev_selection {
            self.refresh_selection_shelf_ids();
        }
        // If the fic that was selected got deleted (e.g. via the details
        // panel's "Delete Fic" button), drop the now-invalid selection so
        // the panel doesn't render a "not found" state next frame.
        if let Selection::Single(id) = *self.selection.current() {
            if !self.cache.fics.iter().any(|f| f.id == id) {
                self.clear_selection();
            }
        }
        if sort_changed {
            self.config.default_sort = self.sort;
            self.save_config();
        }
    }

    /// Modal dispatch + the column picker. Modals run after the rest
    /// of the UI so they overlay correctly. Column-picker writes happen
    /// only when a column actually toggled this frame (previously this
    /// saved every frame the picker was open — at egui's 60 Hz that
    /// meant ~60 disk writes/second while the user was just *looking*
    /// at the picker).
    fn paint_modals(&mut self, ctx: &egui::Context) {
        let columns_changed = column_picker::show(
            ctx,
            &mut self.show_column_picker,
            &mut self.config.visible_columns,
        );
        // Modal dispatch — exactly one variant of `active_modal` runs
        // per frame. Outcomes are extracted into a local `ModalAction`
        // before dispatch so the action handler (which takes `&mut
        // self`) doesn't fight the modal-borrow over `self.active_modal`.
        enum ModalAction {
            None,
            Close,
            CreateShelf(String),
            DeleteShelf(u64),
            DeleteFics(Vec<u64>),
            AddFic(String),
        }
        let action = match &mut self.active_modal {
            ActiveModal::None => ModalAction::None,
            ActiveModal::CreateShelf(state) => match shelf_modals::draw_create(ctx, state) {
                shelf_modals::Outcome::Submit(name) => ModalAction::CreateShelf(name),
                shelf_modals::Outcome::Cancel => ModalAction::Close,
                shelf_modals::Outcome::None => ModalAction::None,
            },
            ActiveModal::DeleteShelf(id) => {
                match shelf_modals::draw_delete_confirm(ctx, *id, &self.cache.shelves) {
                    shelf_modals::DeleteOutcome::Confirm(id) => ModalAction::DeleteShelf(id),
                    shelf_modals::DeleteOutcome::Cancel => ModalAction::Close,
                    shelf_modals::DeleteOutcome::None => ModalAction::None,
                }
            }
            ActiveModal::DeleteFics(ids) => {
                match bulk_modals::draw_delete_confirm(ctx, ids, &self.cache.fics) {
                    bulk_modals::DeleteOutcome::Confirm(ids) => ModalAction::DeleteFics(ids),
                    bulk_modals::DeleteOutcome::Cancel => ModalAction::Close,
                    bulk_modals::DeleteOutcome::None => ModalAction::None,
                }
            }
            ActiveModal::AddFic(state) => match add_fic_dialog::draw(ctx, state) {
                add_fic_dialog::Outcome::Submit(input) => ModalAction::AddFic(input),
                add_fic_dialog::Outcome::Cancel => ModalAction::Close,
                add_fic_dialog::Outcome::None => ModalAction::None,
            },
        };
        match action {
            ModalAction::None => {}
            ModalAction::Close => self.active_modal = ActiveModal::None,
            ModalAction::CreateShelf(name) => {
                let _ = self.create_shelf(name);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::DeleteShelf(id) => {
                let _ = self.delete_shelf(id);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::DeleteFics(ids) => {
                self.handle_bulk_delete(&ids);
                self.active_modal = ActiveModal::None;
            }
            ModalAction::AddFic(input) => {
                self.task_executor.enqueue_add(input);
                self.active_modal = ActiveModal::None;
            }
        }
        if columns_changed {
            self.save_config();
        }
    }

    /// Drains worker completions + refreshes from the background
    /// thread, reloads the in-memory caches, and schedules a repaint
    /// while any task is still running so spinner animations + task
    /// age strings tick over without requiring user input.
    fn drain_worker_events(&mut self, ctx: &egui::Context) {
        let completions = self.task_executor.take_completions();
        if !completions.is_empty() {
            for title in &completions {
                self.toasts
                    .success(format!("Added \u{201C}{}\u{201D}", title));
            }
            self.cache.reload_fics(&self.connection);
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            }
            self.refresh_selection_shelf_ids();
        }
        let refreshes = self.task_executor.take_refreshes();
        if !refreshes.is_empty() {
            self.toasts
                .success(format!("Refreshed {} fanfiction(s)", refreshes.len()));
            self.cache.reload_fics(&self.connection);
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            }
        }
        if self.task_executor.has_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
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
                &self.cache.fics,
                &self.current_view,
                &self.cache.shelf_members,
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
                    self.active_modal = ActiveModal::AddFic(AddFicState::new());
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
                .cache
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
        let attempted = fic_ids.len();
        let errors = self.mutate(|repo| {
            fic_ids
                .iter()
                .filter(|id| add_to_shelf(repo, **id, shelf_id).is_err())
                .count()
        });
        if errors == 0 {
            self.toasts
                .success(format!("Added {} fanfiction(s) to shelf", attempted));
        } else if errors == attempted {
            self.toasts.error(format!("All {} drops failed", attempted));
        } else {
            self.toasts
                .error(format!("{}/{} drops failed", errors, attempted));
        }
    }

    fn handle_bulk_delete(&mut self, ids: &[u64]) {
        let total = ids.len();
        let surviving: Vec<u64> = self.mutate(|repo| {
            ids.iter()
                .filter_map(|id| match delete_fic::delete_fic(repo, *id) {
                    Ok(()) => Some(*id),
                    Err(_) => None,
                })
                .collect()
        });
        let errors = total - surviving.len();
        self.cache.remove_fics(&surviving);
        if errors == 0 {
            self.toasts
                .success(format!("Deleted {} fanfictions", total));
        } else {
            self.toasts
                .error(format!("{}/{} deletions failed", errors, total));
        }
        self.clear_selection();
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
        // The search field only mounts in library views, so a stale
        // pending-focus flag would grab focus retroactively when the
        // user returned to the library — drop it the moment we're
        // off-library to keep the request scoped to this view.
        if !self.current_view.shows_library() {
            self.focus_search_pending = false;
        }
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

        if pressed_esc && !matches!(self.selection.current(), Selection::None) {
            self.clear_selection();
        }

        if pressed_delete && self.current_view.shows_library() {
            let ids = self.selection.ids_vec();
            if !ids.is_empty() {
                self.active_modal = ActiveModal::DeleteFics(ids);
            }
        }

        if ctrl_a && self.current_view.shows_library() {
            let ids = library_view::visible_ids(
                &self.cache.fics,
                &self.current_view,
                &self.cache.shelf_members,
                &self.search_query,
                self.sort,
            );
            self.selection.select_many(&ids);
            self.refresh_selection_shelf_ids();
        }

        if ctrl_n {
            self.active_modal = ActiveModal::CreateShelf(CreateState::default());
        }

        if ctrl_f && self.current_view.shows_library() {
            self.focus_search_pending = true;
        }
    }
}
