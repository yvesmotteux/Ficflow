mod library_file;
mod modals;
mod paint;
mod shortcuts;
mod window;

use std::path::PathBuf;

use egui_notify::Toasts;
use rusqlite::Connection;

use super::auto_shelf;
use super::config::{self, AppConfig, ColumnKey, SortDirection, SortPref, ThemeChoice};
use crate::application::{
    add_to_shelf::add_to_shelf, create_shelf::create_shelf, delete_fic, delete_shelf, move_shelf,
    pin_shelf::pin_shelf, remove_from_shelf, rename_shelf::rename_shelf, unpin_shelf::unpin_shelf,
    update_chapters, update_note, update_rating, update_read_count, update_status,
    upsert_auto_shelf,
};
use crate::domain::fanfiction::{Fanfiction, ReadingStatus, UserRating};
use crate::domain::shelf::{AutoShelfCriteria, Shelf, ShelfKind};
use crate::error::FicflowError;
use crate::infrastructure::SqliteRepository;
use crate::infrastructure::external::ao3::fetcher::ao3_urls_from_env;
use crate::infrastructure::persistence::database::connection::{
    open_configured_db, relocate_library, restore_backup,
};

use super::chrome::FrameChrome;
use super::library_cache::LibraryCache;
use super::selection::Selection;
use super::selection_controller::SelectionController;
use super::tasks::TaskExecutor;
use super::theme;
use super::view::View;
use super::views::details_panel::DetailsState;
use super::views::modals::add_fic_dialog::{self, AddFicState};
use super::views::modals::shelf_modals::{self, AutoShelfState, CreateState, RenameState};
use super::views::modals::{bulk_modals, column_picker, quit_modal, restore_modal};
use super::views::settings_view;
use super::views::tasks_view;
use super::views::{
    LibraryCounts, LibraryViewState, SelectionBarState, SidebarState, TableOutcome, TaskFilter,
    TasksViewState, details_panel, library_view, selection_bar, sidebar,
};

pub struct FicflowApp {
    connection: Connection,
    /// The library file the live `connection` was opened from. Also the
    /// source when the user relocates the library and the target that a
    /// restored backup is copied over.
    current_db_path: PathBuf,
    cache: LibraryCache,
    chrome: FrameChrome,
    config: AppConfig,
    /// Gate so the persisted maximized / fullscreen state is applied
    /// once at first frame and not re-fired every paint.
    initial_window_state_applied: bool,
    sort: SortPref,
    search_query: String,
    show_column_picker: bool,
    selection: SelectionController,
    current_view: View,
    active_modal: ActiveModal,
    /// A Library button was clicked this frame; the native picker is opened
    /// from `ui()` next, where the window handle is available to parent it.
    pending_library_request: Option<settings_view::LibraryRequest>,
    task_executor: TaskExecutor,
    quit_confirmed: bool,
    task_filter: TaskFilter,
    /// Set by Ctrl+F; consumed by `draw_search_field` on next paint.
    focus_search_pending: bool,
    toasts: Toasts,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("database init failed: {0}")]
    Database(FicflowError),
    #[error("chrome SVG init failed: {0}")]
    Chrome(resvg::usvg::Error),
}

/// Mutually-exclusive set of modal windows: only one can be open at a time.
pub enum ActiveModal {
    None,
    CreateShelf(CreateState),
    RenameShelf(RenameState),
    AutoShelf(AutoShelfState),
    DeleteShelf(u64),
    DeleteFics(Vec<u64>),
    RemoveOrDeleteFics { ids: Vec<u64>, shelf_id: u64 },
    AddFic(AddFicState),
    ConfirmQuit,
    ConfirmRestore(PathBuf),
}

/// Explicit wiring so embedders and integration tests can inject a
/// chosen SQLite file and AO3 endpoint without process-global env vars.
#[derive(Clone)]
pub struct FicflowConfig {
    pub db_path: Option<PathBuf>,
    pub ao3_urls: Vec<String>,
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
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, InitError> {
        Self::with_config(&cc.egui_ctx, FicflowConfig::default())
    }

    /// `&egui::Context` (not `&CreationContext`) so headless tests can
    /// build the app without an eframe runtime. The worker thread
    /// opens its own connection to the same `db_path` since
    /// `Connection: !Send`.
    pub fn with_config(ctx: &egui::Context, config: FicflowConfig) -> Result<Self, InitError> {
        theme::install(ctx);
        let mut app_config = AppConfig::load();
        let db_path = match &config.db_path {
            Some(path) => path.clone(),
            None => app_config.resolved_db_path().map_err(InitError::Database)?,
        };
        let connection = open_configured_db(&db_path).map_err(InitError::Database)?;
        let cache = LibraryCache::load(&connection);
        let chrome = FrameChrome::new().map_err(InitError::Chrome)?;
        app_config.text_zoom = config::set_zoom(ctx, app_config.text_zoom);
        config::set_theme(ctx, app_config.theme);
        let sort = app_config.default_sort;
        let current_view = app_config
            .last_view
            .and_then(|pv| View::from_persisted(pv, &cache.shelves))
            .unwrap_or_default();
        let task_executor =
            TaskExecutor::spawn(config.ao3_urls, config.max_retry_cycles, db_path.clone());
        let mut app = Self {
            connection,
            current_db_path: db_path,
            cache,
            chrome,
            config: app_config,
            initial_window_state_applied: false,
            sort,
            search_query: String::new(),
            show_column_picker: false,
            selection: SelectionController::new(),
            current_view,
            active_modal: ActiveModal::None,
            pending_library_request: None,
            task_executor,
            quit_confirmed: false,
            task_filter: TaskFilter::default(),
            focus_search_pending: false,
            toasts: Toasts::default(),
        };
        // Restoring straight into a shelf view skips the sidebar's
        // prev/post diff (see `paint_sidebar`) that normally populates
        // `shelf_members` on a view change, so do it explicitly here.
        app.refresh_shelf_members();
        Ok(app)
    }

    // ---- Read-only accessors ----
    // Kept narrow on purpose: tests should drive behaviour via the same
    // public methods the GUI uses, not by mutating internal fields.

    pub fn fics(&self) -> &[Fanfiction] {
        &self.cache.fics
    }

    pub fn shelves(&self) -> &[Shelf] {
        &self.cache.shelves
    }

    /// Sidebar count for a shelf; 0 for a missing or empty shelf. For
    /// auto-shelves this comes from the live-computed cache, not
    /// `fic_shelf` rows.
    pub fn shelf_count(&self, shelf_id: u64) -> usize {
        self.cache.shelf_counts.get(&shelf_id).copied().unwrap_or(0)
    }

    pub fn selection(&self) -> &Selection {
        self.selection.current()
    }

    pub fn selection_shelves(&self) -> &std::collections::HashSet<u64> {
        &self.cache.selection_shelf_ids
    }

    pub fn current_view(&self) -> &View {
        &self.current_view
    }

    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    pub fn sort(&self) -> SortPref {
        self.sort
    }

    pub fn set_sort(&mut self, column: ColumnKey, direction: SortDirection) {
        self.sort = SortPref { column, direction };
    }

    pub fn theme_choice(&self) -> ThemeChoice {
        self.config.theme
    }

    pub fn set_theme(&mut self, ctx: &egui::Context, choice: ThemeChoice) {
        self.config.theme = choice;
        config::set_theme(ctx, choice);
        self.save_config();
    }

    pub fn visible_ids(&self) -> Vec<u64> {
        library_view::visible_ids(
            &self.cache.fics,
            &self.current_view,
            &self.cache.shelf_members,
            &self.search_query,
            self.sort,
        )
    }

    pub fn details_panel_visible(&self) -> bool {
        matches!(self.selection.current(), Selection::Single(_))
            && self.current_view.shows_library()
    }

    pub fn has_running_tasks(&self) -> bool {
        self.task_executor.has_running()
    }

    pub fn confirm_quit_open(&self) -> bool {
        matches!(self.active_modal, ActiveModal::ConfirmQuit)
    }

    pub fn delete_fics_open(&self) -> bool {
        matches!(self.active_modal, ActiveModal::DeleteFics(_))
    }

    /// `Some(shelf_id)` when the "remove from shelf vs delete" chooser is
    /// the open modal.
    pub fn remove_or_delete_shelf(&self) -> Option<u64> {
        match self.active_modal {
            ActiveModal::RemoveOrDeleteFics { shelf_id, .. } => Some(shelf_id),
            _ => None,
        }
    }

    pub fn task_states(&self) -> Vec<crate::interfaces::gui::tasks::TaskState> {
        self.task_executor.snapshot()
    }

    // ---- Control surface ----
    // The `set_*` / `bulk_*` methods bypass widget glue (combo box,
    // DragValue, star widget). A test calling `set_status(id, Read)`
    // proves `update_reading_status` lands in the DB; it does not
    // prove the status combo emits `Read` for its "Read" entry.
    // Closing that gap needs an event-injection harness such as
    // egui_kittest.

    pub fn submit_add_fic(&self, input: impl Into<String>) {
        self.task_executor.enqueue_add(input.into());
    }

    pub fn confirm_quit(&mut self) {
        self.quit_confirmed = true;
        self.active_modal = ActiveModal::None;
    }

    pub fn select_fic(&mut self, fic_id: u64) {
        self.selection.select_single(fic_id);
        self.refresh_selection_shelf_ids();
    }

    pub fn select_fics(&mut self, ids: &[u64]) {
        self.selection.select_many(ids);
        self.refresh_selection_shelf_ids();
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.cache.selection_shelf_ids.clear();
    }

    pub fn open_view(&mut self, view: View) {
        self.current_view = view;
        self.refresh_shelf_members();
        self.persist_current_view();
    }

    /// Saves `current_view` into `AppConfig` if it's a restorable
    /// (library-facing) view. Called from both `open_view` and the
    /// sidebar's direct `current_view` mutation (`paint_sidebar`).
    fn persist_current_view(&mut self) {
        if let Some(persisted) = self.current_view.to_persisted() {
            self.config.last_view = Some(persisted);
            self.save_config();
        }
    }

    pub fn refresh_selected(&self) {
        if let Selection::Single(id) = *self.selection.current()
            && let Some(fic) = self.cache.fic(id)
        {
            self.task_executor.enqueue_refresh(id, fic.title.clone());
        }
    }

    pub fn delete_selected(&mut self) {
        let ids = self.selection.ids_vec();
        if !ids.is_empty() {
            self.handle_bulk_delete(&ids);
        }
    }

    pub fn create_shelf(&mut self, name: impl AsRef<str>) -> Result<(), FicflowError> {
        self.shelf_op(
            "create shelf",
            |repo| create_shelf(repo, name.as_ref(), None),
            |shelf| format!("Created shelf \u{201C}{}\u{201D}", shelf.name),
        )
        .map(drop)
    }

    /// Creates a new auto-shelf (`shelf_id: None`) or updates an
    /// existing one's name + criteria (`shelf_id: Some`).
    pub fn upsert_auto_shelf(
        &mut self,
        shelf_id: Option<u64>,
        name: impl AsRef<str>,
        criteria: AutoShelfCriteria,
    ) -> Result<(), FicflowError> {
        let verb = if shelf_id.is_some() {
            "Updated"
        } else {
            "Created"
        };
        self.shelf_op(
            "save auto-shelf",
            |repo| {
                upsert_auto_shelf::upsert_auto_shelf(repo, shelf_id, name.as_ref(), None, criteria)
            },
            |shelf| format!("{} auto-shelf \u{201C}{}\u{201D}", verb, shelf.name),
        )?;
        self.refresh_shelf_members();
        Ok(())
    }

    pub fn delete_shelf(&mut self, shelf_id: u64) -> Result<(), FicflowError> {
        self.shelf_op(
            "delete shelf",
            |repo| delete_shelf::delete_shelf(repo, shelf_id),
            |()| "Shelf deleted".to_string(),
        )?;
        if self.current_view == View::Shelf(shelf_id) {
            self.open_view(View::AllFics);
        }
        Ok(())
    }

    pub fn move_shelf(
        &mut self,
        shelf_id: u64,
        new_parent: Option<u64>,
    ) -> Result<(), FicflowError> {
        self.shelf_op(
            "move shelf",
            |repo| move_shelf::move_shelf(repo, shelf_id, new_parent),
            |shelf| format!("Moved shelf \u{201C}{}\u{201D}", shelf.name),
        )?;
        self.refresh_shelf_members();
        Ok(())
    }

    pub fn toggle_pin_shelf(&mut self, shelf_id: u64) -> Result<(), FicflowError> {
        let pinned = self
            .cache
            .shelf(shelf_id)
            .ok_or(FicflowError::ShelfNotFound { shelf_id })?
            .pinned;
        self.shelf_op(
            "update shelf",
            |repo| {
                if pinned {
                    unpin_shelf(repo, shelf_id)
                } else {
                    pin_shelf(repo, shelf_id)
                }
            },
            |shelf| {
                let verb = if shelf.pinned { "Pinned" } else { "Unpinned" };
                format!("{} shelf \u{201C}{}\u{201D}", verb, shelf.name)
            },
        )
        .map(drop)
    }

    pub fn rename_shelf(
        &mut self,
        shelf_id: u64,
        new_name: impl AsRef<str>,
    ) -> Result<(), FicflowError> {
        self.shelf_op(
            "rename shelf",
            |repo| rename_shelf(repo, shelf_id, new_name.as_ref()),
            |shelf| format!("Renamed shelf to \u{201C}{}\u{201D}", shelf.name),
        )
        .map(drop)
    }

    fn shelf_op<T>(
        &mut self,
        action: &str,
        op: impl FnOnce(&SqliteRepository<'_>) -> Result<T, FicflowError>,
        success_message: impl FnOnce(&T) -> String,
    ) -> Result<T, FicflowError> {
        let result = op(&self.repo());
        match &result {
            Ok(value) => {
                self.toasts.success(success_message(value));
                self.cache.reload_shelves(&self.connection);
                self.refresh_shelf_counts();
            }
            Err(err) => {
                self.toasts.error(format!("Couldn't {}: {}", action, err));
            }
        }
        result
    }

    pub fn add_fic_to_shelf(&mut self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError> {
        self.mutate(|repo| add_to_shelf(repo, fic_id, shelf_id))
    }

    pub fn remove_fic_from_shelf(
        &mut self,
        fic_id: u64,
        shelf_id: u64,
    ) -> Result<(), FicflowError> {
        self.mutate(|repo| remove_from_shelf::remove_from_shelf(repo, fic_id, shelf_id))
    }

    /// Status changes can't affect shelf membership, so this skips the
    /// `mutate()` funnel that the other bulk ops use.
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
        self.cache.replace_fics(updated_fics);
        (ids.len() - errors, errors)
    }

    pub fn bulk_add_to_shelf(&mut self, ids: &[u64], shelf_id: u64) -> (usize, usize) {
        let errors = self.mutate(|repo| {
            ids.iter()
                .filter(|id| add_to_shelf(repo, **id, shelf_id).is_err())
                .count()
        });
        (ids.len() - errors, errors)
    }

    pub fn bulk_remove_from_shelf(&mut self, ids: &[u64], shelf_id: u64) -> (usize, usize) {
        let errors = self.mutate(|repo| {
            ids.iter()
                .filter(|id| remove_from_shelf::remove_from_shelf(repo, **id, shelf_id).is_err())
                .count()
        });
        (ids.len() - errors, errors)
    }

    fn is_normal_shelf(&self, shelf_id: u64) -> bool {
        self.cache
            .shelves
            .iter()
            .any(|s| s.id == shelf_id && matches!(s.kind, ShelfKind::Normal))
    }

    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    pub fn set_status(&mut self, fic_id: u64, status: ReadingStatus) -> Result<(), FicflowError> {
        self.update_fic(|repo| update_status::update_reading_status(repo, fic_id, status))
    }

    pub fn set_last_chapter(&mut self, fic_id: u64, chapter: u32) -> Result<(), FicflowError> {
        self.update_fic(|repo| update_chapters::update_last_chapter_read(repo, fic_id, chapter))
    }

    pub fn set_read_count(&mut self, fic_id: u64, count: u32) -> Result<(), FicflowError> {
        self.update_fic(|repo| update_read_count::update_read_count(repo, fic_id, count))
    }

    pub fn set_user_rating(
        &mut self,
        fic_id: u64,
        rating: Option<UserRating>,
    ) -> Result<(), FicflowError> {
        self.update_fic(|repo| update_rating::update_user_rating(repo, fic_id, rating))
    }

    pub fn set_note(&mut self, fic_id: u64, note: Option<&str>) -> Result<(), FicflowError> {
        self.update_fic(|repo| update_note::update_personal_note(repo, fic_id, note))
    }

    fn update_fic(
        &mut self,
        op: impl FnOnce(&SqliteRepository<'_>) -> Result<Fanfiction, FicflowError>,
    ) -> Result<(), FicflowError> {
        let updated = op(&self.repo())?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    fn save_config(&self) {
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save config: {}", err);
        }
    }

    fn dispatch_details_outcome(&mut self, fic_id: u64, outcome: details_panel::Outcome) {
        use details_panel::Outcome;
        let (action, result) = match outcome {
            Outcome::None => return,
            Outcome::SetStatus(status) => ("update status", self.set_status(fic_id, status)),
            Outcome::SetLastChapter(n) => ("update chapter", self.set_last_chapter(fic_id, n)),
            Outcome::SetReadCount(n) => ("update read count", self.set_read_count(fic_id, n)),
            Outcome::SetUserRating(rating) => {
                ("update rating", self.set_user_rating(fic_id, rating))
            }
            Outcome::SetNote(value) => ("update note", self.set_note(fic_id, value.as_deref())),
            Outcome::AddToShelf(shelf_id) => {
                ("add to shelf", self.add_fic_to_shelf(fic_id, shelf_id))
            }
            Outcome::RemoveFromShelf(shelf_id) => (
                "remove from shelf",
                self.remove_fic_from_shelf(fic_id, shelf_id),
            ),
            Outcome::RequestDelete => return self.delete_selected(),
            Outcome::RequestRefresh => return self.refresh_selected(),
            Outcome::CreateAutoShelfFromTag(field, value) => {
                self.active_modal = ActiveModal::AutoShelf(AutoShelfState::prefilled(field, value));
                return;
            }
        };
        if let Err(err) = result {
            self.toasts.error(format!("Couldn't {}: {}", action, err));
        }
    }

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

    fn repo(&self) -> SqliteRepository<'_> {
        SqliteRepository::new(&self.connection)
    }

    /// Surfaces refresh failures as toasts; the op's own result is
    /// forwarded.
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

    fn refresh_shelf_members(&mut self) {
        if let Err(err) = self
            .cache
            .refresh_shelf_members(&self.connection, &self.current_view)
        {
            self.toasts
                .error(format!("Couldn't load shelf contents: {}", err));
        }
    }

    fn prune_selection_to_view(&mut self) {
        let changed = self.selection.prune_to_view(
            &self.cache.fics,
            &self.current_view,
            &self.cache.shelf_members,
        );
        // The post-render diff in `paint_central` captures `prev_selection`
        // *after* this prune, so it won't catch the change — refresh here.
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

    fn handle_bulk_delete(&mut self, ids: &[u64]) {
        let total = ids.len();
        let deleted: Vec<u64> = self.mutate(|repo| {
            ids.iter()
                .filter(|id| delete_fic::delete_fic(repo, **id).is_ok())
                .copied()
                .collect()
        });
        let errors = total - deleted.len();
        self.cache.remove_fics(&deleted);
        if errors == 0 && total == 1 {
            self.toasts.success("Fanfiction deleted");
        } else if errors == 0 {
            self.toasts
                .success(format!("Deleted {} fanfictions", total));
        } else if total == 1 {
            self.toasts.error("Couldn't delete the fanfiction");
        } else {
            self.toasts
                .error(format!("{}/{} deletions failed", errors, total));
        }
        self.clear_selection();
    }
}
