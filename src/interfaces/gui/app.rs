use egui::{RichText, Ui};
use rusqlite::Connection;

use crate::application::list_fics;
use crate::domain::fanfiction::Fanfiction;
use crate::infrastructure::config::{AppConfig, SortPref};
use crate::infrastructure::persistence::database::connection::establish_connection;
use crate::infrastructure::SqliteRepository;

use super::views::{column_picker, library_view, LibraryViewState};

pub struct FicflowApp {
    /// Held by the GUI for the lifetime of the app: needed by later phases
    /// (selection details fetch, status/chapter/note edits) that re-query
    /// or write via `SqliteRepository::new(&self.connection)`.
    #[allow(dead_code)]
    connection: Connection,
    fics: Vec<Fanfiction>,
    config: AppConfig,
    sort: SortPref,
    search_query: String,
    show_column_picker: bool,
}

#[derive(Debug)]
pub enum InitError {
    Database(crate::error::FicflowError),
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
        let config = AppConfig::load();
        let sort = config.default_sort;
        Ok(Self {
            connection,
            fics,
            config,
            sort,
            search_query: String::new(),
            show_column_picker: false,
        })
    }

    fn save_config(&self) {
        if let Err(err) = self.config.save() {
            log::warn!("Failed to save config: {}", err);
        }
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

impl eframe::App for FicflowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("ficflow-header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("FICFLOW");
                ui.separator();
                ui.label("ALL FICTIONS");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Manage Columns").clicked() {
                        self.show_column_picker = !self.show_column_picker;
                    }
                });
            });
            ui.add_space(4.0);
        });

        egui::SidePanel::left("ficflow-sidebar")
            .default_width(160.0)
            .resizable(true)
            .show(ctx, |ui| {
                draw_sidebar(ui);
            });

        egui::SidePanel::right("ficflow-details")
            .default_width(260.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.label("Select a fanfiction to see its details.");
            });

        let mut sort_changed = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            sort_changed = library_view::draw(
                ui,
                LibraryViewState {
                    fics: &self.fics,
                    sort: &mut self.sort,
                    search_query: &mut self.search_query,
                    visible_columns: &self.config.visible_columns,
                },
            );
        });

        let columns_changed = column_picker::show(
            ctx,
            &mut self.show_column_picker,
            &mut self.config.visible_columns,
        );

        // Persist preference changes immediately. TOML write is small and
        // robust against crashes that would skip an `on_exit` save.
        if sort_changed {
            self.config.default_sort = self.sort;
        }
        if sort_changed || columns_changed {
            self.save_config();
        }
    }
}

fn draw_sidebar(ui: &mut Ui) {
    ui.add_space(4.0);
    ui.label(RichText::new("LIBRARY").weak());
    for label in [
        "All Fanfictions",
        "In Progress",
        "Read",
        "Plan to Read",
        "Paused",
        "Abandoned",
    ] {
        let _ = ui.selectable_label(false, label);
    }
    ui.add_space(8.0);
    ui.label(RichText::new("SHELVES").weak());
    ui.label(RichText::new("(none yet)").italics().weak());
}
