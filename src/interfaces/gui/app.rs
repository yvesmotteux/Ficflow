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

use super::chrome::FrameChrome;
use super::library_cache::LibraryCache;
use super::selection::Selection;
use super::selection_controller::SelectionController;
use super::tasks::TaskExecutor;
use super::theme;
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
    task_executor: TaskExecutor,
    task_filter: TaskFilter,
    /// Set by Ctrl+F; consumed by `draw_search_field` on next paint.
    focus_search_pending: bool,
    toasts: Toasts,
}

#[derive(Debug)]
pub enum InitError {
    Database(FicflowError),
    Chrome(resvg::usvg::Error),
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InitError::Database(e) => write!(f, "database init failed: {}", e),
            InitError::Chrome(e) => write!(f, "chrome SVG init failed: {}", e),
        }
    }
}

impl std::error::Error for InitError {}

/// Mutually-exclusive set of modal windows: only one can be open at a time.
pub enum ActiveModal {
    None,
    CreateShelf(CreateState),
    DeleteShelf(u64),
    DeleteFics(Vec<u64>),
    AddFic(AddFicState),
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
        let connection = match &config.db_path {
            Some(path) => open_configured_db(path).map_err(InitError::Database)?,
            None => establish_connection().map_err(InitError::Database)?,
        };
        let cache = LibraryCache::load(&connection);
        let chrome = FrameChrome::new().map_err(InitError::Chrome)?;
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
            chrome,
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

    // ---- Read-only accessors ----
    // Kept narrow on purpose: tests should drive behaviour via the same
    // public methods the GUI uses, not by mutating internal fields.

    pub fn fics(&self) -> &[Fanfiction] {
        &self.cache.fics
    }

    pub fn shelves(&self) -> &[Shelf] {
        &self.cache.shelves
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

    pub fn task_states(&self) -> Vec<crate::interfaces::gui::tasks::TaskState> {
        self.task_executor.snapshot()
    }

    // ---- Control surface ----
    // The `set_*` / `bulk_*` methods bypass widget glue (combo box,
    // DragValue, star widget). A test calling `set_status(id, Read)`
    // proves `update_reading_status` lands in the DB; it does not
    // prove the status combo emits `Read` for its "Read" entry.
    // Closing that gap needs an event-injection harness incompatible
    // with the pinned egui 0.29.

    pub fn submit_add_fic(&self, input: impl Into<String>) {
        self.task_executor.enqueue_add(input.into());
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
    }

    pub fn refresh_selected(&self) {
        if let Selection::Single(id) = *self.selection.current() {
            if let Some(fic) = self.cache.fics.iter().find(|f| f.id == id) {
                self.task_executor.enqueue_refresh(id, fic.title.clone());
            }
        }
    }

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
        for fic in updated_fics {
            self.cache.replace_fic(fic);
        }
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

    pub fn set_search(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
    }

    pub fn set_status(&mut self, fic_id: u64, status: ReadingStatus) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_status::update_reading_status(&repo, fic_id, status)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    pub fn set_last_chapter(&mut self, fic_id: u64, chapter: u32) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_chapters::update_last_chapter_read(&repo, fic_id, chapter)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

    pub fn set_read_count(&mut self, fic_id: u64, count: u32) -> Result<(), FicflowError> {
        let repo = self.repo();
        let updated = update_read_count::update_read_count(&repo, fic_id, count)?;
        self.cache.replace_fic(updated);
        Ok(())
    }

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

    /// eframe 0.29's window persistence drops `maximized` — track it
    /// ourselves: re-apply on first frame, then persist on toggle.
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

    /// Transparent so the chrome's painted edges show through the
    /// borderless undecorated window (see NativeOptions in `mod.rs`).
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }
}

impl FicflowApp {
    /// Headless-friendly entry point — `App::update` above is a thin
    /// delegate so tests can drive the app without an `eframe::Frame`.
    ///
    /// The host `CentralPanel` exists because egui's top-level panels
    /// (`SidePanel::show(ctx, …)`) claim from `ctx.screen_rect()` and
    /// would punch through the chrome. Nesting every panel inside a
    /// single `UiBuilder::max_rect(content_rect)` constrains them to
    /// the chrome's content area with a single anchor change.
    pub fn render(&mut self, ctx: &egui::Context) {
        self.sync_window_state(ctx);
        self.handle_shortcuts(ctx);

        let screen = ctx.screen_rect();
        self.chrome.paint_background(ctx, screen);

        egui::CentralPanel::default()
            .frame(egui::Frame::none())
            .show(ctx, |ui| {
                let controls_rect = self.chrome.draw_window_controls(ui, screen);
                self.chrome.handle_interactions(ui, screen, controls_rect);

                let content_rect = self.chrome.content_rect(screen);
                ui.allocate_new_ui(egui::UiBuilder::new().max_rect(content_rect), |host| {
                    self.paint_header(host);
                    self.paint_sidebar(host);
                    self.paint_details_panel(host);
                    self.paint_selection_bar(host);
                    self.paint_central(host);
                });
            });
        self.paint_modals(ctx);
        self.drain_worker_events(ctx);
        self.draw_drag_preview(ctx);
        self.toasts.show(ctx);

        // Force a steady repaint cadence so hover state recovers after
        // an OS-managed move/resize — winit doesn't forward a
        // pointer-enter event back, so egui's hover bookkeeping otherwise
        // goes stale until the user wiggles the mouse.
        ctx.request_repaint();
    }

    fn paint_header(&self, host: &mut egui::Ui) {
        egui::TopBottomPanel::top("ficflow-header").show_inside(host, |ui| {
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("FICFLOW")
                            .family(egui::FontFamily::Name(theme::NEUE_FAMILY.into()))
                            .size(20.0)
                            .color(theme::ACCENT),
                    )
                    .selectable(false),
                );
            });
            ui.add_space(4.0);
        });
    }

    /// Sidebar can mutate `current_view`; the prev/post diff at the
    /// end refreshes derived caches and prunes the selection.
    fn paint_sidebar(&mut self, host: &mut egui::Ui) {
        let prev_view = self.current_view.clone();
        let library_counts = compute_library_counts(&self.cache.fics);
        let mut outcome = sidebar::Outcome::None;
        egui::SidePanel::left("ficflow-sidebar")
            .default_width(160.0)
            .width_range(140.0..=600.0)
            .resizable(true)
            .show_inside(host, |ui| {
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

    fn paint_details_panel(&mut self, host: &mut egui::Ui) {
        let Selection::Single(id) = *self.selection.current() else {
            return;
        };
        if !self.current_view.shows_library() {
            return;
        }
        // Clone so the immutable borrow on `cache.fics` releases before
        // dispatching outcomes through `&mut self`.
        let Some(fic) = self.cache.fics.iter().find(|f| f.id == id).cloned() else {
            return;
        };
        let mut outcome = details_panel::Outcome::None;
        egui::SidePanel::right("ficflow-details")
            .default_width(320.0)
            .width_range(280.0..=900.0)
            .resizable(true)
            .show_inside(host, |ui| {
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

    fn paint_selection_bar(&mut self, host: &mut egui::Ui) {
        let selection_ids = self.selection.ids_vec();
        if selection_ids.is_empty() || !self.current_view.shows_library() {
            return;
        }
        let mut outcome = selection_bar::Outcome::None;
        egui::TopBottomPanel::bottom("ficflow-selection-bar")
            .resizable(false)
            .show_inside(host, |ui| {
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

    fn paint_central(&mut self, host: &mut egui::Ui) {
        let mut sort_changed = false;
        let mut empty_area_clicked = false;
        let prev_selection = self.selection.current().clone();
        let view_title = self.current_view.header_title(&self.cache.shelves);
        egui::CentralPanel::default().show_inside(host, |ui| {
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
            // Empty-area click-sink. Table rows have their own click
            // sense, so the panel's outer response never fires below
            // the last row — allocate the leftover space here.
            let remaining = ui.available_size();
            if remaining.x > 0.0 && remaining.y > 0.0 {
                let resp = ui.allocate_response(remaining, egui::Sense::click());
                if resp.clicked() {
                    empty_area_clicked = true;
                }
            }
        });
        if empty_area_clicked && self.current_view.shows_library() {
            self.clear_selection();
        }
        if *self.selection.current() != prev_selection {
            self.refresh_selection_shelf_ids();
        }
        // Selected fic got deleted this frame: drop the orphan selection.
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

    fn paint_modals(&mut self, ctx: &egui::Context) {
        let columns_changed = column_picker::show(
            ctx,
            &mut self.show_column_picker,
            &mut self.config.visible_columns,
        );
        // Outcomes extracted into a local `ModalAction` before dispatch
        // so the action handler (which takes `&mut self`) doesn't fight
        // the modal-borrow over `self.active_modal`.
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
    /// Search bar is overlaid via an `Area` anchored to the window's
    /// horizontal centre — so resizing the sidebar / details panel
    /// doesn't slide it around — but clamped to the central panel's
    /// rect so it never paints over the right panel.
    fn draw_central_header(&mut self, ui: &mut egui::Ui, view_title: &str) {
        // Header buttons collapse to icons (`+`, `≡`) below this
        // width to leave room for the search-bar overlay.
        let compact = ui.available_width() < 520.0;
        // `ui.horizontal(...)` claims only its children's width; we
        // need the panel's full rect to clamp the search overlay.
        let panel_rect = ui.available_rect_before_wrap();
        let row_resp = ui.horizontal(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(view_title)
                        .family(egui::FontFamily::Name(theme::NEUE_FAMILY.into()))
                        .size(20.0)
                        .color(theme::ACCENT),
                )
                .selectable(false),
            );
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let cols_label = if compact {
                    "\u{2261}"
                } else {
                    "Manage Columns"
                };
                if ui
                    .button(cols_label)
                    .on_hover_text("Manage Columns")
                    .clicked()
                {
                    self.show_column_picker = !self.show_column_picker;
                }
                let add_label = if compact { "+" } else { "+ Add Fic" };
                if ui.button(add_label).on_hover_text("Add Fic").clicked() {
                    self.active_modal = ActiveModal::AddFic(AddFicState::new());
                }
            });
        });

        if self.current_view.shows_library() {
            const SEARCH_W_MAX: f32 = 300.0;
            const SEARCH_W_MIN: f32 = 120.0;
            const SEARCH_H: f32 = 22.0;
            const EDGE_GAP: f32 = 8.0;
            // Approximate widths the row reserves for title+count on
            // the left and action buttons on the right; the search
            // bar fills whatever's left.
            let buttons_reserve = if compact { 90.0 } else { 220.0 };
            let title_reserve = 200.0;
            let avail_for_search =
                panel_rect.width() - buttons_reserve - title_reserve - 2.0 * EDGE_GAP;
            if avail_for_search >= SEARCH_W_MIN {
                let search_w = avail_for_search.min(SEARCH_W_MAX);
                let screen = ui.ctx().screen_rect();
                let row_rect = row_resp.response.rect;
                let desired_x = screen.center().x - search_w / 2.0;
                let max_x = panel_rect.right() - buttons_reserve - EDGE_GAP - search_w;
                let min_x = panel_rect.left() + title_reserve + EDGE_GAP;
                let clamped_x = desired_x.clamp(min_x, max_x);
                let pos = egui::pos2(clamped_x, row_rect.center().y - SEARCH_H / 2.0);
                egui::Area::new(egui::Id::new("ficflow-search-overlay"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(pos)
                    .show(ui.ctx(), |area_ui| {
                        self.draw_search_field(area_ui, search_w);
                    });
            }
        }
    }

    /// Magnifying-glass glyph + borderless TextEdit inside a Frame so
    /// the icon sits *inside* the apparent input boundary (a hint
    /// text would disappear once the user starts typing).
    fn draw_search_field(&mut self, ui: &mut egui::Ui, width: f32) {
        let stroke = ui.visuals().widgets.inactive.bg_stroke;
        let fill = ui.visuals().extreme_bg_color;
        let weak = ui.visuals().weak_text_color();

        ui.allocate_ui(egui::vec2(width, 22.0), |ui| {
            egui::Frame::default()
                .fill(fill)
                .stroke(stroke)
                .rounding(2.0)
                .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                .show(ui, |ui| {
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
    /// egui's built-in dnd doesn't ship a drag-preview, so we paint a
    /// label near the cursor whenever there's an active payload.
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
    /// Skipped while a text edit has focus so we don't hijack the user's
    /// typing (Ctrl+A in a TextEdit should select the text, not all rows).
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // The search field only mounts in library views; a stale
        // pending-focus flag would retroactively grab focus on return.
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

        if pressed_delete {
            let selected_fics = self.selection.ids_vec();
            if !selected_fics.is_empty() && self.current_view.shows_library() {
                self.active_modal = ActiveModal::DeleteFics(selected_fics);
            } else if let View::Shelf(shelf_id) = &self.current_view {
                // No fic selection on a shelf view → Delete targets the
                // shelf itself.
                self.active_modal = ActiveModal::DeleteShelf(*shelf_id);
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
