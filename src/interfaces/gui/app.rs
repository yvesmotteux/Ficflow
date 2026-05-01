use std::collections::HashSet;

use egui_notify::Toasts;
use rusqlite::Connection;

use crate::application::{
    add_to_shelf::add_to_shelf, create_shelf::create_shelf, delete_fic, delete_shelf, list_fics,
    list_shelf_fics, list_shelves, list_shelves_for_fic,
};
use crate::domain::fanfiction::Fanfiction;
use crate::domain::shelf::Shelf;
use crate::error::FicflowError;
use crate::infrastructure::config::{AppConfig, SortPref};
use crate::infrastructure::persistence::database::connection::establish_connection;
use crate::infrastructure::SqliteRepository;

use super::selection::Selection;
use super::view::View;
use super::views::bulk_modals;
use super::views::details_panel::DetailsState;
use super::views::shelf_modals::{self, CreateState};
use super::views::{
    column_picker, details_panel, library_view, selection_bar, sidebar, LibraryViewState,
    SelectionBarState, SidebarState,
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
    config: AppConfig,
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

impl FicflowApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Result<Self, InitError> {
        let connection = establish_connection().map_err(InitError::Database)?;
        let fics = load_fics(&connection);
        let shelves = load_shelves(&connection);
        let config = AppConfig::load();
        let sort = config.default_sort;
        Ok(Self {
            connection,
            fics,
            shelves,
            shelf_members: HashSet::new(),
            selection_shelf_ids: HashSet::new(),
            config,
            sort,
            search_query: String::new(),
            show_column_picker: false,
            selection: Selection::default(),
            last_clicked_id: None,
            current_view: View::default(),
            create_shelf_modal: CreateState::new(),
            delete_shelf_pending: None,
            delete_fics_pending: None,
            toasts: Toasts::default(),
        })
    }

    fn save_config(&self) {
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save config: {}", err);
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

impl eframe::App for FicflowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Header — title varies with the current view.
        let header_title = self.current_view.header_title(&self.shelves);
        egui::TopBottomPanel::top("ficflow-header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("FICFLOW");
                ui.separator();
                ui.label(header_title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Manage Columns").clicked() {
                        self.show_column_picker = !self.show_column_picker;
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Sidebar.
        let mut create_request = false;
        let mut drop_on_shelf: Option<(u64, Vec<u64>)> = None;
        let prev_view = self.current_view.clone();
        egui::SidePanel::left("ficflow-sidebar")
            .default_width(160.0)
            .resizable(true)
            .show(ctx, |ui| {
                sidebar::draw(
                    ui,
                    SidebarState {
                        current_view: &mut self.current_view,
                        shelves: &self.shelves,
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

        // Details panel (right). Only meaningful for library views.
        let mut shelves_changed = false;
        egui::SidePanel::right("ficflow-details")
            .default_width(260.0)
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
                    },
                );
            });

        // Selection bar (bottom of the central area, only when multi-selecting).
        let mut bulk_shelves_changed = false;
        if matches!(self.selection, Selection::Multi(_)) && self.current_view.shows_library() {
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
        let central = egui::CentralPanel::default().show(ctx, |ui| {
            if self.current_view.shows_library() {
                sort_changed = library_view::draw(
                    ui,
                    LibraryViewState {
                        fics: &self.fics,
                        sort: &mut self.sort,
                        search_query: &mut self.search_query,
                        visible_columns: &self.config.visible_columns,
                        selection: &mut self.selection,
                        view: &self.current_view,
                        shelf_members: &self.shelf_members,
                        last_clicked_id: &mut self.last_clicked_id,
                    },
                );
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
            self.refresh_selection_shelf_ids();
            if matches!(self.current_view, View::Shelf(_)) {
                self.refresh_shelf_members();
            }
        }

        // Modals (run after the rest of the UI so they overlay correctly).
        column_picker::show(
            ctx,
            &mut self.show_column_picker,
            &mut self.config.visible_columns,
        );
        match shelf_modals::draw_create(ctx, &mut self.create_shelf_modal) {
            shelf_modals::Outcome::Submit(name) => self.handle_create_shelf(name),
            shelf_modals::Outcome::Cancel | shelf_modals::Outcome::None => {}
        }
        match shelf_modals::draw_delete_confirm(ctx, &mut self.delete_shelf_pending, &self.shelves)
        {
            shelf_modals::DeleteOutcome::Confirm(id) => self.handle_delete_shelf(id),
            shelf_modals::DeleteOutcome::Cancel | shelf_modals::DeleteOutcome::None => {}
        }
        match bulk_modals::draw_delete_confirm(ctx, &mut self.delete_fics_pending, &self.fics) {
            bulk_modals::DeleteOutcome::Confirm(ids) => self.handle_bulk_delete(&ids),
            bulk_modals::DeleteOutcome::Cancel | bulk_modals::DeleteOutcome::None => {}
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
    fn handle_create_shelf(&mut self, name: String) {
        let repo = SqliteRepository::new(&self.connection);
        match create_shelf(&repo, &name) {
            Ok(shelf) => {
                self.toasts
                    .success(format!("Created shelf \u{201C}{}\u{201D}", shelf.name));
                self.shelves = load_shelves(&self.connection);
            }
            Err(err) => {
                self.toasts.error(format!("Couldn't create shelf: {}", err));
            }
        }
    }

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
        if matches!(self.current_view, View::Shelf(_)) {
            self.refresh_shelf_members();
        }
    }

    fn handle_delete_shelf(&mut self, shelf_id: u64) {
        let repo = SqliteRepository::new(&self.connection);
        match delete_shelf::delete_shelf(&repo, shelf_id) {
            Ok(()) => {
                self.toasts.success("Shelf deleted");
                if self.current_view == View::Shelf(shelf_id) {
                    self.current_view = View::AllFics;
                    self.shelf_members.clear();
                }
                self.shelves = load_shelves(&self.connection);
            }
            Err(err) => {
                self.toasts.error(format!("Couldn't delete shelf: {}", err));
            }
        }
    }
}

fn draw_stub_view(ui: &mut egui::Ui, view: &View) {
    ui.add_space(8.0);
    let message = match view {
        View::Tasks => "Background tasks land in Phase 9.",
        View::Settings => "Settings panel lands in Phase 10.",
        _ => "",
    };
    ui.label(egui::RichText::new(message).italics().weak());
}
