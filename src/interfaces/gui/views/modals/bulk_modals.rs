use egui::{Context, Ui};

use super::{ModalOutcome, action_row, show_modal};
use crate::domain::fanfiction::Fanfiction;

const MAX_TITLES_SHOWN: usize = 5;

pub enum RemoveOrDelete {
    RemoveFromShelf,
    DeleteEverywhere,
}

/// Lists the first few titles by way of "are you sure", summarises the
/// rest as "+ N more".
pub fn draw_delete_confirm(
    ctx: &Context,
    ids: &[u64],
    fics: &[Fanfiction],
    with_epubs: bool,
) -> ModalOutcome<()> {
    show_modal(ctx, "Delete fanfictions", |ui| {
        ui.label(format!("Delete {} fanfiction(s)?", ids.len()));
        title_preview(ui, ids, fics);
        epub_note(ui, with_epubs);
        ui.add_space(6.0);
        action_row(ui, "Delete", true, None, || ())
    })
}

pub fn draw_remove_or_delete(
    ctx: &Context,
    ids: &[u64],
    shelf_name: &str,
    fics: &[Fanfiction],
    with_epubs: bool,
) -> ModalOutcome<RemoveOrDelete> {
    show_modal(ctx, "Remove or delete", |ui| {
        ui.label(format!("{} fanfiction(s) selected", ids.len()));
        title_preview(ui, ids, fics);
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Removing keeps them in Ficflow; deleting removes them everywhere.",
            )
            .italics()
            .weak(),
        );
        epub_note(ui, with_epubs);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui
                .button(format!("Remove from \"{}\"", shelf_name))
                .clicked()
            {
                ModalOutcome::Submit(RemoveOrDelete::RemoveFromShelf)
            } else if ui.button("Delete from Ficflow").clicked() {
                ModalOutcome::Submit(RemoveOrDelete::DeleteEverywhere)
            } else if ui.button("Cancel").clicked() {
                ModalOutcome::Cancel
            } else {
                ModalOutcome::None
            }
        })
        .inner
    })
}

fn epub_note(ui: &mut Ui, with_epubs: bool) {
    if with_epubs {
        ui.label(
            egui::RichText::new("Their saved EPUBs are deleted too.")
                .italics()
                .weak(),
        );
    }
}

fn title_preview(ui: &mut Ui, ids: &[u64], fics: &[Fanfiction]) {
    ui.add_space(4.0);
    for id in ids.iter().take(MAX_TITLES_SHOWN) {
        let title = fics
            .iter()
            .find(|f| f.id == *id)
            .map_or("(unknown)", |f| f.title.as_str());
        ui.label(format!("• {}", title));
    }
    if ids.len() > MAX_TITLES_SHOWN {
        ui.label(
            egui::RichText::new(format!("+ {} more", ids.len() - MAX_TITLES_SHOWN))
                .italics()
                .weak(),
        );
    }
}
