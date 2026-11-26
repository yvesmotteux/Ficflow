use egui::{Context, RichText};

use super::super::super::tasks::ImportSource;
use super::super::super::theme;
use super::{ModalOutcome, action_row, show_modal};
use crate::interfaces::utils::url_parser::{
    ImportInput, parse_import_input, profile_list, subscriptions_list,
};

/// Buffer for the in-progress AO3 URL or fic id. The "is the modal
/// open?" answer lives in the parent `ActiveModal` enum — when this
/// struct exists at all, the modal is open.
#[derive(Default)]
pub struct AddFicState {
    pub input: String,
    pub profile_bookmarks: bool,
}

pub enum AddRequest {
    Single(String),
    Import(ImportSource),
}

pub fn draw(
    ctx: &Context,
    state: &mut AddFicState,
    username: Option<&str>,
) -> ModalOutcome<AddRequest> {
    show_modal(ctx, "Add fanfictions", |ui| {
        ui.label("AO3 link, fic ID, or IDs (1, 2, 3):");
        let resp = ui.text_edit_singleline(&mut state.input);
        // Auto-focus on first frame so the user can type immediately.
        if !resp.has_focus() && state.input.is_empty() {
            resp.request_focus();
        }
        let trimmed = state.input.trim().to_string();
        let parsed = parse_import_input(&trimmed);
        let request = match &parsed {
            ImportInput::Single(_) => Some(AddRequest::Single(trimmed.clone())),
            ImportInput::Ids(ids) => Some(AddRequest::Import(ImportSource::Ids(ids.clone()))),
            ImportInput::List(list) => Some(AddRequest::Import(ImportSource::List(list.clone()))),
            ImportInput::Profile { user } => Some(AddRequest::Import(ImportSource::List(
                profile_list(user, state.profile_bookmarks),
            ))),
            ImportInput::Invalid => None,
        };
        match &parsed {
            ImportInput::Single(_) => hint(ui, "One fic"),
            ImportInput::Ids(ids) => hint(ui, &format!("{} fics", ids.len())),
            ImportInput::List(list) => hint(ui, &format!("{}: adds every work in it", list.label)),
            ImportInput::Profile { user } => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{user}'s profile. Import:")).weak());
                    ui.radio_value(&mut state.profile_bookmarks, false, "Their works");
                    ui.radio_value(&mut state.profile_bookmarks, true, "Their bookmarks");
                });
            }
            ImportInput::Invalid if !trimmed.is_empty() => {
                ui.label(
                    RichText::new("Not an AO3 fic or list link").color(theme::error(ui.visuals())),
                );
            }
            ImportInput::Invalid => {}
        }
        if let Some(user) = username {
            ui.add_space(6.0);
            if ui.button("Import my subscriptions").clicked() {
                return ModalOutcome::Submit(AddRequest::Import(ImportSource::List(
                    subscriptions_list(user),
                )));
            }
        }
        ui.add_space(6.0);
        action_row(ui, "Add", request.is_some(), Some(&resp), || {
            request.expect("Add is only enabled with a valid request")
        })
    })
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).weak());
}
