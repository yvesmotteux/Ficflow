//! Multi-select dropdown for picking which shelves a fic belongs to.
//! Closed state shows the selected shelves as bubble chips with an
//! `×` to remove and a caret on the right; clicking opens a popup that
//! lists every shelf with checkboxes for toggling membership.

use std::collections::HashSet;

use egui::{Align2, Color32, Id, RichText, Sense, Stroke, Ui, Vec2};

use crate::domain::shelf::Shelf;

/// Returned per frame so the caller can run the appropriate side-effect
/// (DB add/remove). Only one toggle fires per frame at most — clicking a
/// chip's `×` or a checkbox in the popup.
#[derive(Clone, Copy)]
pub enum DropdownOutcome {
    None,
    Toggled { shelf_id: u64, now_selected: bool },
}

pub fn shelves_dropdown(
    ui: &mut Ui,
    id_salt: &str,
    all_shelves: &[Shelf],
    selected: &HashSet<u64>,
) -> DropdownOutcome {
    let mut outcome = DropdownOutcome::None;
    let popup_id = Id::new(("shelves-dropdown", id_salt));

    let visuals = &ui.visuals().widgets.inactive;
    let stroke = visuals.bg_stroke;
    let fill = ui.visuals().extreme_bg_color;

    // Outer field (the "closed" state). Allocate a row of full available
    // width and ~26 px tall, paint a TextEdit-style frame, then lay out
    // chips + caret inside.
    let avail = ui.available_width();
    let height = 28.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(avail, height), Sense::click());
    ui.painter().rect(rect, 4.0, fill, stroke);

    // Caret cell on the right (16 px). Click toggles the popup.
    let caret_w = 20.0;
    let caret_rect = egui::Rect::from_min_size(
        egui::pos2(rect.right() - caret_w, rect.top()),
        Vec2::new(caret_w, height),
    );
    let popup_open = ui.memory(|m| m.is_popup_open(popup_id));
    let caret = if popup_open { "\u{25B2}" } else { "\u{25BC}" };
    ui.painter().text(
        caret_rect.center(),
        Align2::CENTER_CENTER,
        caret,
        egui::FontId::proportional(11.0),
        ui.visuals().weak_text_color(),
    );

    // Toggle popup on click anywhere in the field.
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    // Chips for each currently-selected shelf, laid out left-to-right
    // inside the field. Painted manually so we control sizing and the
    // per-chip × hit area.
    let pad_x = 8.0;
    let chip_h = 20.0;
    let chip_y = rect.top() + (height - chip_h) / 2.0;
    let mut x = rect.left() + pad_x;
    let chip_font = egui::FontId::proportional(12.0);
    let label_color = ui.visuals().text_color();
    let chip_fill = ui.visuals().widgets.inactive.weak_bg_fill;
    let max_x = caret_rect.left() - 4.0;

    // Iterate in canonical shelf order so the chip order is stable.
    for shelf in all_shelves {
        if !selected.contains(&shelf.id) {
            continue;
        }
        // Measure the chip width.
        let label_w = ui
            .painter()
            .layout_no_wrap(shelf.name.clone(), chip_font.clone(), label_color)
            .size()
            .x;
        let close_w = 14.0;
        let chip_w = label_w + 8.0 + close_w + 8.0;
        if x + chip_w > max_x {
            // No more horizontal room — last chip is followed by an
            // ellipsis-style indicator. Skip silently for simplicity:
            // user can scroll the popup to see the full list.
            break;
        }

        let chip_rect = egui::Rect::from_min_size(egui::pos2(x, chip_y), Vec2::new(chip_w, chip_h));
        ui.painter().rect(chip_rect, 4.0, chip_fill, Stroke::NONE);
        ui.painter().text(
            egui::pos2(chip_rect.left() + 6.0, chip_rect.center().y),
            Align2::LEFT_CENTER,
            &shelf.name,
            chip_font.clone(),
            label_color,
        );

        // × glyph on the right of the chip; clicking removes the shelf.
        let close_rect = egui::Rect::from_min_size(
            egui::pos2(chip_rect.right() - close_w - 4.0, chip_y),
            Vec2::new(close_w, chip_h),
        );
        let close_resp = ui.interact(
            close_rect,
            ui.id().with(("chip-close", id_salt, shelf.id)),
            Sense::click(),
        );
        let close_color = if close_resp.hovered() {
            Color32::from_rgb(220, 90, 90)
        } else {
            ui.visuals().weak_text_color()
        };
        ui.painter().text(
            close_rect.center(),
            Align2::CENTER_CENTER,
            "\u{2715}",
            egui::FontId::proportional(11.0),
            close_color,
        );
        if close_resp.clicked() {
            outcome = DropdownOutcome::Toggled {
                shelf_id: shelf.id,
                now_selected: false,
            };
        }

        x += chip_w + 4.0;
    }

    if selected.is_empty() {
        ui.painter().text(
            egui::pos2(rect.left() + pad_x, rect.center().y),
            Align2::LEFT_CENTER,
            "No shelves",
            chip_font.clone(),
            ui.visuals().weak_text_color(),
        );
    }

    // Popup with one checkbox per shelf. Anchored below the field.
    egui::popup_below_widget(
        ui,
        popup_id,
        &resp,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(rect.width());
            ui.set_max_height(220.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                if all_shelves.is_empty() {
                    ui.label(RichText::new("No shelves yet.").italics().weak());
                    return;
                }
                for shelf in all_shelves {
                    let mut on = selected.contains(&shelf.id);
                    if ui.checkbox(&mut on, &shelf.name).changed() {
                        outcome = DropdownOutcome::Toggled {
                            shelf_id: shelf.id,
                            now_selected: on,
                        };
                    }
                }
            });
        },
    );

    outcome
}
