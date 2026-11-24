use egui::Context;

use super::modal_window;

use super::super::super::config::ColumnKey;

pub fn show(ctx: &Context, open: &mut bool, visible_columns: &mut Vec<ColumnKey>) -> bool {
    let mut changed = false;
    modal_window(ctx, "Manage Columns")
        .open(open)
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
