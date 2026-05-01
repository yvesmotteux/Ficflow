use egui::{Color32, RichText, Stroke, Ui};

use crate::domain::fanfiction::ReadingStatus;
use crate::domain::shelf::Shelf;

use super::super::view::View;

pub struct SidebarState<'a> {
    pub current_view: &'a mut View,
    pub shelves: &'a [Shelf],
    pub create_shelf_request: &'a mut bool,
    pub delete_shelf_request: &'a mut Option<u64>,
    /// Set when a drag-and-drop drop lands on a shelf row: `(shelf_id, fic_ids)`.
    /// Caller bulk-adds the fics to the shelf and refreshes its caches.
    pub drop_on_shelf: &'a mut Option<(u64, Vec<u64>)>,
}

pub fn draw(ui: &mut Ui, state: SidebarState<'_>) {
    let SidebarState {
        current_view,
        shelves,
        create_shelf_request,
        delete_shelf_request,
        drop_on_shelf,
    } = state;

    // Pin Tasks/Settings to the bottom; Library + Shelves scroll above them.
    egui::TopBottomPanel::bottom("ficflow-sidebar-bottom")
        .resizable(false)
        .show_separator_line(true)
        .show_inside(ui, |ui| {
            ui.add_space(4.0);
            view_row(ui, current_view, View::Tasks, "Tasks");
            view_row(ui, current_view, View::Settings, "Settings");
            ui.add_space(4.0);
        });

    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show_inside(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("sidebar-scroll")
                .show(ui, |ui| {
                    ui.add_space(4.0);
                    section_label(ui, "LIBRARY");
                    view_row(ui, current_view, View::AllFics, "All Fanfictions");
                    view_row(
                        ui,
                        current_view,
                        View::ByStatus(ReadingStatus::InProgress),
                        "In Progress",
                    );
                    view_row(
                        ui,
                        current_view,
                        View::ByStatus(ReadingStatus::Read),
                        "Read",
                    );
                    view_row(
                        ui,
                        current_view,
                        View::ByStatus(ReadingStatus::PlanToRead),
                        "Plan to Read",
                    );
                    view_row(
                        ui,
                        current_view,
                        View::ByStatus(ReadingStatus::Paused),
                        "Paused",
                    );
                    view_row(
                        ui,
                        current_view,
                        View::ByStatus(ReadingStatus::Abandoned),
                        "Abandoned",
                    );

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        section_label(ui, "SHELVES");
                        if ui.small_button("+").on_hover_text("New shelf").clicked() {
                            *create_shelf_request = true;
                        }
                    });
                    if shelves.is_empty() {
                        ui.label(RichText::new("(none yet)").italics().weak());
                    } else {
                        for shelf in shelves {
                            let resp =
                                view_row(ui, current_view, View::Shelf(shelf.id), &shelf.name);

                            // Drop target: highlight while a row is hovering
                            // with a payload, commit the drop on release.
                            if resp.dnd_hover_payload::<Vec<u64>>().is_some() {
                                ui.painter().rect_stroke(
                                    resp.rect,
                                    4.0,
                                    Stroke::new(2.0, Color32::from_rgb(120, 200, 120)),
                                );
                            }
                            if let Some(payload) = resp.dnd_release_payload::<Vec<u64>>() {
                                *drop_on_shelf = Some((shelf.id, (*payload).clone()));
                            }

                            resp.context_menu(|ui| {
                                if ui.button("Delete shelf").clicked() {
                                    *delete_shelf_request = Some(shelf.id);
                                    ui.close_menu();
                                }
                            });
                        }
                    }
                });
        });
}

fn section_label(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).weak().size(11.0));
}

/// A clickable sidebar row that becomes selected when `current_view` matches
/// `target`. Returns the underlying response so callers can attach context
/// menus (used for shelf right-click → delete).
fn view_row(ui: &mut Ui, current_view: &mut View, target: View, label: &str) -> egui::Response {
    let selected = *current_view == target;
    let resp = ui.selectable_label(selected, label);
    if resp.clicked() && !selected {
        *current_view = target;
    }
    resp
}
