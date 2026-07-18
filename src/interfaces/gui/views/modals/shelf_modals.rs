use egui::{ComboBox, Context, Window};

use crate::domain::fanfiction::ReadingStatus;
use crate::domain::shelf::{AutoShelfCriteria, Clause, ClauseLogic, Shelf};
use crate::interfaces::gui::auto_shelf::DistinctValues;
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
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum CreateKind {
    #[default]
    Normal,
    Auto,
}

pub struct CreateState {
    pub name: String,
    pub kind: CreateKind,
    pub logic: ClauseLogic,
    pub clauses: Vec<ClauseRow>,
}

impl Default for CreateState {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: CreateKind::Normal,
            logic: ClauseLogic::And,
            clauses: Vec::new(),
        }
    }
}

impl CreateState {
    /// Pre-filled from a "create auto-shelf from this tag" context-menu
    /// click: starts in Auto mode with one clause already set.
    pub fn prefilled(field: ClauseFieldKind, value: String) -> Self {
        Self {
            name: String::new(),
            kind: CreateKind::Auto,
            logic: ClauseLogic::And,
            clauses: vec![ClauseRow::new(field, value)],
        }
    }
}

pub enum Outcome {
    None,
    SubmitNormal(String),
    SubmitAuto(String, AutoShelfCriteria),
    Cancel,
}

/// Renders the AND/OR toggle and clause rows shared by the create and
/// (future) edit-criteria modals. Returns whether the criteria built
/// from `state` is non-empty enough to submit.
fn draw_clause_builder(
    ui: &mut egui::Ui,
    state: &mut CreateState,
    distinct_values: &DistinctValues,
) {
    ui.horizontal(|ui| {
        ui.label("Match");
        ComboBox::from_id_salt("auto-shelf-logic")
            .selected_text(match state.logic {
                ClauseLogic::And => "ALL of",
                ClauseLogic::Or => "ANY of",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut state.logic, ClauseLogic::And, "ALL of");
                ui.selectable_value(&mut state.logic, ClauseLogic::Or, "ANY of");
            });
        ui.label("these:");
    });
    ui.add_space(4.0);

    let mut remove_index = None;
    for (i, row) in state.clauses.iter_mut().enumerate() {
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
                    .selected_text(row.status.to_string())
                    .show_ui(ui, |ui| {
                        for status in [
                            ReadingStatus::InProgress,
                            ReadingStatus::Read,
                            ReadingStatus::PlanToRead,
                            ReadingStatus::Paused,
                            ReadingStatus::Abandoned,
                        ] {
                            ui.selectable_value(&mut row.status, status, status.to_string());
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
        state.clauses.remove(i);
    }

    if ui.button("+ Add clause").clicked() {
        state
            .clauses
            .push(ClauseRow::new(ClauseFieldKind::Tag, String::new()));
    }
}

pub fn draw_create(
    ctx: &Context,
    state: &mut CreateState,
    distinct_values: &DistinctValues,
) -> Outcome {
    let mut still_open = true;
    let mut outcome = Outcome::None;
    Window::new("Create shelf")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.label("Name:");
            let resp = ui.text_edit_singleline(&mut state.name);
            // Auto-focus the field the first frame so users can type immediately.
            if !resp.has_focus() && state.name.is_empty() {
                resp.request_focus();
            }
            ui.add_space(6.0);

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

            if state.kind == CreateKind::Auto {
                draw_clause_builder(ui, state, distinct_values);
                ui.add_space(6.0);
            }

            ui.horizontal(|ui| {
                let name_ok = !state.name.trim().is_empty();
                let criteria = build_criteria(state);
                let submit_enabled = name_ok
                    && match state.kind {
                        CreateKind::Normal => true,
                        CreateKind::Auto => !criteria.clauses.is_empty(),
                    };
                let create_clicked = ui
                    .add_enabled(submit_enabled, egui::Button::new("Create"))
                    .clicked();
                let pressed_enter = resp.lost_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                    && submit_enabled;
                if create_clicked || pressed_enter {
                    outcome = match state.kind {
                        CreateKind::Normal => Outcome::SubmitNormal(state.name.trim().to_string()),
                        CreateKind::Auto => {
                            Outcome::SubmitAuto(state.name.trim().to_string(), criteria)
                        }
                    };
                }
                if ui.button("Cancel").clicked() {
                    outcome = Outcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = Outcome::Cancel;
    }
    outcome
}

fn build_criteria(state: &CreateState) -> AutoShelfCriteria {
    AutoShelfCriteria {
        logic: state.logic,
        clauses: state
            .clauses
            .iter()
            .filter_map(ClauseRow::to_clause)
            .collect(),
    }
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

pub enum RenameOutcome {
    None,
    Submit { shelf_id: u64, new_name: String },
    Cancel,
}

pub fn draw_rename(ctx: &Context, state: &mut RenameState) -> RenameOutcome {
    let mut still_open = true;
    let mut outcome = RenameOutcome::None;
    Window::new("Rename shelf")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
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
            ui.horizontal(|ui| {
                let submit_enabled = !state.name.trim().is_empty();
                let rename_clicked = ui
                    .add_enabled(submit_enabled, egui::Button::new("Rename"))
                    .clicked();
                let pressed_enter = resp.lost_focus()
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                    && submit_enabled;
                if rename_clicked || pressed_enter {
                    outcome = RenameOutcome::Submit {
                        shelf_id: state.shelf_id,
                        new_name: state.name.trim().to_string(),
                    };
                }
                if ui.button("Cancel").clicked() {
                    outcome = RenameOutcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = RenameOutcome::Cancel;
    }
    outcome
}

pub enum DeleteOutcome {
    None,
    Confirm(u64),
    Cancel,
}

/// Draws the delete-shelf confirmation modal. Caller is responsible
/// for only invoking this when `ActiveModal::DeleteShelf(_)` is the
/// current modal — the early-return guard is gone.
pub fn draw_delete_confirm(ctx: &Context, shelf_id: u64, shelves: &[Shelf]) -> DeleteOutcome {
    let shelf_name = shelves
        .iter()
        .find(|s| s.id == shelf_id)
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("(id {})", shelf_id));

    let mut still_open = true;
    let mut outcome = DeleteOutcome::None;
    Window::new("Delete shelf")
        .open(&mut still_open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
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
            ui.horizontal(|ui| {
                if ui.button("Delete").clicked() {
                    outcome = DeleteOutcome::Confirm(shelf_id);
                }
                if ui.button("Cancel").clicked() {
                    outcome = DeleteOutcome::Cancel;
                }
            });
        });
    if !still_open {
        outcome = DeleteOutcome::Cancel;
    }
    outcome
}
