//! Minimal autocomplete text field: a plain text edit that, while
//! typing, shows a filtered list of matching `options` below it.
//! Clicking a suggestion replaces the field's text. No keyboard
//! navigation — filter + list + click only.

use egui::{Popup, PopupCloseBehavior, Response, Ui};

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

    // Open/close state lives in egui's popup memory (keyed by `popup_id`),
    // not in the text field's own focus flag — the field surrenders
    // focus the instant a click lands anywhere outside its own rect
    // (including on a suggestion below it), *within the same frame* as
    // that click. Gating visibility on `resp.has_focus()` meant the
    // popup vanished before the click could ever reach a suggestion.
    let popup_id = ui.id().with(("autocomplete-popup", &id_salt));
    if matches.is_empty() {
        Popup::close_id(ui.ctx(), popup_id);
    } else if resp.gained_focus() || resp.changed() {
        Popup::open_id(ui.ctx(), popup_id);
    }

    Popup::from_response(&resp)
        .id(popup_id)
        .open_memory(None)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(resp.rect.width());
            for opt in &matches {
                if ui.selectable_label(false, opt.as_str()).clicked() {
                    *text = (*opt).clone();
                    Popup::close_id(ui.ctx(), popup_id);
                }
            }
        });

    resp
}
