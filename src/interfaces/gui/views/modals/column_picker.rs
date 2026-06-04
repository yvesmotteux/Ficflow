use egui::{Context, Window};

use super::super::super::config::ColumnKey;

pub fn show(ctx: &Context, open: &mut bool, visible_columns: &mut Vec<ColumnKey>) -> bool {
    let mut changed = false;
    Window::new("Manage Columns")
        .open(open)
        .resizable(false)
        .collapsible(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.label("Choose which columns to display in the library table.");
            ui.add_space(6.0);
            for column in ColumnKey::ALL {
                let mut visible = visible_columns.contains(&column);
                if ui.checkbox(&mut visible, column.label()).changed() {
                    if visible {
                        visible_columns.push(column);
                    } else {
                        visible_columns.retain(|c| *c != column);
                    }
                    changed = true;
                }
            }
        });
    changed
}
