use egui::{ComboBox, Context};

use super::{ModalOutcome, action_row, show_modal};

use crate::domain::fanfiction::ReadingStatus;
use crate::domain::shelf::{AutoShelfCriteria, Clause, ClauseLogic, Shelf, ShelfKind};
use crate::interfaces::gui::auto_shelf::DistinctValues;
use crate::interfaces::gui::format::format_status;
use crate::interfaces::gui::widgets::autocomplete_input;

/// Which field a query-builder clause row matches against. Distinct
/// from `domain::shelf::Clause` — this is the UI's picker state, only
/// converted into a `Clause` (with its value parsed/typed) on submit.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ClauseFieldKind {
    #[default]
    Tag,
    Fandom,
    Relationship,
    Character,
    Author,
    Status,
}

impl ClauseFieldKind {
    pub const ALL: [ClauseFieldKind; 6] = [
        ClauseFieldKind::Tag,
        ClauseFieldKind::Fandom,
        ClauseFieldKind::Relationship,
        ClauseFieldKind::Character,
        ClauseFieldKind::Author,
        ClauseFieldKind::Status,
    ];

    fn label(&self) -> &'static str {
        match self {
            ClauseFieldKind::Tag => "Tag",
            ClauseFieldKind::Fandom => "Fandom",
            ClauseFieldKind::Relationship => "Relationship",
            ClauseFieldKind::Character => "Character",
            ClauseFieldKind::Author => "Author",
            ClauseFieldKind::Status => "Status",
        }
    }
}

pub struct ClauseRow {
    pub field: ClauseFieldKind,
    pub value: String,
    pub status: ReadingStatus,
}

impl ClauseRow {
    fn new(field: ClauseFieldKind, value: String) -> Self {
        Self {
            field,
            value,
            status: ReadingStatus::PlanToRead,
        }
    }

    fn to_clause(&self) -> Option<Clause> {
        if self.field == ClauseFieldKind::Status {
            return Some(Clause::Status(self.status));
        }
        let value = self.value.trim();
        if value.is_empty() {
            return None;
        }
        Some(match self.field {
            ClauseFieldKind::Tag => Clause::Tag(value.to_string()),
            ClauseFieldKind::Fandom => Clause::Fandom(value.to_string()),
            ClauseFieldKind::Relationship => Clause::Relationship(value.to_string()),
            ClauseFieldKind::Character => Clause::Character(value.to_string()),
            ClauseFieldKind::Author => Clause::Author(value.to_string()),
            ClauseFieldKind::Status => unreachable!(),
        })
    }

    fn from_clause(clause: &Clause) -> Self {
        match clause {
            Clause::Tag(v) => Self::new(ClauseFieldKind::Tag, v.clone()),
            Clause::Fandom(v) => Self::new(ClauseFieldKind::Fandom, v.clone()),
            Clause::Relationship(v) => Self::new(ClauseFieldKind::Relationship, v.clone()),
            Clause::Character(v) => Self::new(ClauseFieldKind::Character, v.clone()),
            Clause::Author(v) => Self::new(ClauseFieldKind::Author, v.clone()),
            Clause::Status(status) => {
                let mut row = Self::new(ClauseFieldKind::Status, String::new());
                row.status = *status;
                row
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CreateKind {
    #[default]
    Normal,
    Auto,
}

/// State for the initial "+" picker: a plain name for Normal shelves,
/// or a hand-off into `AutoShelfState` for Auto ones (see
/// `CreateChoice::Auto`) — the auto-shelf name/criteria form itself is never
/// duplicated here.
#[derive(Default)]
pub struct CreateState {
    pub name: String,
    pub kind: CreateKind,
}

pub enum CreateChoice {
    Normal(String),
    Auto,
}

/// Renders the AND/OR toggle and clause rows shared by the create and
/// edit-criteria modals.
fn draw_clause_builder(
    ui: &mut egui::Ui,
    logic: &mut ClauseLogic,
    clauses: &mut Vec<ClauseRow>,
    distinct_values: &DistinctValues,
) {
    ui.horizontal(|ui| {
        ui.label("Match");
        ComboBox::from_id_salt("auto-shelf-logic")
            .selected_text(match logic {
                ClauseLogic::And => "ALL of",
                ClauseLogic::Or => "ANY of",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(logic, ClauseLogic::And, "ALL of");
                ui.selectable_value(logic, ClauseLogic::Or, "ANY of");
            });
        ui.label("these:");
    });
    ui.add_space(4.0);

    let mut remove_index = None;
    for (i, row) in clauses.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ComboBox::from_id_salt(("auto-shelf-field", i))
                .selected_text(row.field.label())
                .show_ui(ui, |ui| {
                    for field in ClauseFieldKind::ALL {
                        ui.selectable_value(&mut row.field, field, field.label());
                    }
                });
            if row.field == ClauseFieldKind::Status {
                ComboBox::from_id_salt(("auto-shelf-status", i))
                    .selected_text(format_status(&row.status))
                    .show_ui(ui, |ui| {
                        for status in ReadingStatus::ALL {
                            ui.selectable_value(&mut row.status, status, format_status(&status));
                        }
                    });
            } else {
                let options = match row.field {
                    ClauseFieldKind::Tag => &distinct_values.tags,
                    ClauseFieldKind::Fandom => &distinct_values.fandoms,
                    ClauseFieldKind::Relationship => &distinct_values.relationships,
                    ClauseFieldKind::Character => &distinct_values.characters,
                    ClauseFieldKind::Author => &distinct_values.authors,
                    ClauseFieldKind::Status => unreachable!(),
                };
                autocomplete_input::draw(ui, ("auto-shelf-value", i), &mut row.value, options);
            }
            if ui.button("\u{2715}").clicked() {
                remove_index = Some(i);
            }
        });
    }
    if let Some(i) = remove_index {
        clauses.remove(i);
    }

    if ui.button("+ Add clause").clicked() {
        clauses.push(ClauseRow::new(ClauseFieldKind::Tag, String::new()));
    }
}

pub fn draw_create(ctx: &Context, state: &mut CreateState) -> ModalOutcome<CreateChoice> {
    show_modal(ctx, "Create shelf", |ui| {
        ComboBox::from_id_salt("shelf-kind")
            .selected_text(match state.kind {
                CreateKind::Normal => "Normal shelf",
                CreateKind::Auto => "Auto-shelf",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.kind, CreateKind::Normal, "Normal shelf");
                ui.selectable_value(&mut state.kind, CreateKind::Auto, "Auto-shelf");
            });
        ui.add_space(6.0);

        match state.kind {
            CreateKind::Normal => {
                ui.label("Name:");
                let resp = ui.text_edit_singleline(&mut state.name);
                // Auto-focus the field the first frame so users can type immediately.
                if !resp.has_focus() && state.name.is_empty() {
                    resp.request_focus();
                }
                ui.add_space(6.0);
                let name = state.name.trim();
                action_row(ui, "Create", !name.is_empty(), Some(&resp), || {
                    CreateChoice::Normal(name.to_string())
                })
            }
            CreateKind::Auto => {
                ui.label(
                    egui::RichText::new(
                        "The next step lets you name the shelf and build its criteria.",
                    )
                    .weak(),
                );
                ui.add_space(6.0);
                action_row(ui, "Continue", true, None, || CreateChoice::Auto)
            }
        }
    })
}

fn build_criteria(logic: ClauseLogic, clauses: &[ClauseRow]) -> AutoShelfCriteria {
    AutoShelfCriteria {
        logic,
        clauses: clauses.iter().filter_map(ClauseRow::to_clause).collect(),
    }
}

/// Shared by both creating a new auto-shelf (`shelf_id: None`) and
/// editing an existing one's name + criteria (`shelf_id: Some`) — one
/// state type and one modal for both, since the two only differ in
/// whether a shelf id already exists to upsert against.
pub struct AutoShelfState {
    pub shelf_id: Option<u64>,
    pub name: String,
    pub logic: ClauseLogic,
    pub clauses: Vec<ClauseRow>,
}

impl AutoShelfState {
    pub fn new() -> Self {
        Self {
            shelf_id: None,
            name: String::new(),
            logic: ClauseLogic::And,
            clauses: Vec::new(),
        }
    }

    /// Pre-filled from a "create auto-shelf from this tag" context-menu
    /// click: one clause already set.
    pub fn prefilled(field: ClauseFieldKind, value: String) -> Self {
        Self {
            clauses: vec![ClauseRow::new(field, value)],
            ..Self::new()
        }
    }

    /// `None` if `shelf` isn't an auto-shelf.
    pub fn from_shelf(shelf: &Shelf) -> Option<Self> {
        match &shelf.kind {
            ShelfKind::Auto(criteria) => Some(Self {
                shelf_id: Some(shelf.id),
                name: shelf.name.clone(),
                logic: criteria.logic,
                clauses: criteria
                    .clauses
                    .iter()
                    .map(ClauseRow::from_clause)
                    .collect(),
            }),
            ShelfKind::Normal => None,
        }
    }
}

pub struct AutoShelfSubmit {
    pub shelf_id: Option<u64>,
    pub name: String,
    pub criteria: AutoShelfCriteria,
}

pub fn draw_auto_shelf(
    ctx: &Context,
    state: &mut AutoShelfState,
    distinct_values: &DistinctValues,
) -> ModalOutcome<AutoShelfSubmit> {
    let is_edit = state.shelf_id.is_some();
    let title = if is_edit {
        "Edit auto-shelf"
    } else {
        "Create auto-shelf"
    };
    show_modal(ctx, title, |ui| {
        ui.label("Name:");
        let resp = ui.text_edit_singleline(&mut state.name);
        if !is_edit && !resp.has_focus() && state.name.is_empty() {
            resp.request_focus();
        }
        ui.add_space(6.0);

        draw_clause_builder(ui, &mut state.logic, &mut state.clauses, distinct_values);
        ui.add_space(6.0);

        let submit_enabled = state.clauses.iter().any(|row| row.to_clause().is_some());
        let submit_label = if is_edit { "Save" } else { "Create" };
        action_row(ui, submit_label, submit_enabled, None, || AutoShelfSubmit {
            shelf_id: state.shelf_id,
            name: state.name.trim().to_string(),
            criteria: build_criteria(state.logic, &state.clauses),
        })
    })
}

pub struct RenameState {
    pub shelf_id: u64,
    pub name: String,
    focused_once: bool,
}

impl RenameState {
    pub fn new(shelf: &Shelf) -> Self {
        Self {
            shelf_id: shelf.id,
            name: shelf.name.clone(),
            focused_once: false,
        }
    }
}

pub fn draw_rename(ctx: &Context, state: &mut RenameState) -> ModalOutcome<String> {
    show_modal(ctx, "Rename shelf", |ui| {
        ui.label("Name:");
        let resp = ui.text_edit_singleline(&mut state.name);
        if !state.focused_once {
            resp.request_focus();
            if let Some(mut tes) = egui::TextEdit::load_state(ctx, resp.id) {
                let end = state.name.chars().count();
                tes.cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(end),
                    )));
                tes.store(ctx, resp.id);
            }
            state.focused_once = true;
        }
        ui.add_space(6.0);
        let name = state.name.trim();
        action_row(ui, "Rename", !name.is_empty(), Some(&resp), || {
            name.to_string()
        })
    })
}

pub fn draw_delete_confirm(ctx: &Context, shelf_id: u64, shelves: &[Shelf]) -> ModalOutcome<()> {
    let shelf_name = shelves
        .iter()
        .find(|s| s.id == shelf_id)
        .map_or_else(|| format!("(id {})", shelf_id), |s| s.name.clone());

    show_modal(ctx, "Delete shelf", |ui| {
        ui.label(format!("Delete shelf \u{201C}{}\u{201D}?", shelf_name));
        ui.label(
            egui::RichText::new("Fanfictions in the shelf are not deleted.")
                .weak()
                .italics(),
        );
        if shelves.iter().any(|s| s.parent_shelf_id == Some(shelf_id)) {
            ui.label(
                egui::RichText::new("Sub-shelves move up to the parent level.")
                    .weak()
                    .italics(),
            );
        }
        ui.add_space(6.0);
        action_row(ui, "Delete", true, None, || ())
    })
}
