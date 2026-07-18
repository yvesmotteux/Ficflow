//! Minimal autocomplete text field: a plain text edit that, while
//! focused with non-empty text, shows a filtered list of matching
//! `options` below it. Clicking a suggestion replaces the field's text.
//! No keyboard navigation — filter + list + click only.

use egui::{Area, Frame, Id, Order, Response, Ui};

pub fn draw(
    ui: &mut Ui,
    id_salt: impl std::hash::Hash,
    text: &mut String,
    options: &[String],
) -> Response {
    let resp = ui.text_edit_singleline(text);
    let query = text.trim().to_lowercase();
    let matches: Vec<&String> = if query.is_empty() {
        Vec::new()
    } else {
        options
            .iter()
            .filter(|o| o.to_lowercase().contains(&query))
            .take(8)
            .collect()
    };

    if resp.has_focus() && !matches.is_empty() {
        Area::new(Id::new(("autocomplete-popup", &id_salt)))
            .order(Order::Foreground)
            .fixed_pos(resp.rect.left_bottom())
            .show(ui.ctx(), |ui| {
                Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(resp.rect.width());
                    for opt in &matches {
                        if ui.selectable_label(false, opt.as_str()).clicked() {
                            *text = (*opt).clone();
                        }
                    }
                });
            });
    }

    resp
}
