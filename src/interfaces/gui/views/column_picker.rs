use egui::{Context, Window};

use crate::infrastructure::config::ColumnKey;

/// Renders the column-picker window when `open` is true. Returns true if the
/// user toggled any column (caller persists the new `visible_columns`).
///
/// `visible_columns` is rewritten in canonical `ColumnKey::ALL` order so that
/// removing and re-adding a column doesn't mess up its position.
pub fn show(ctx: &Context, open: &mut bool, visible_columns: &mut Vec<ColumnKey>) -> bool {
    let mut changed = false;
    Window::new("Manage Columns")
        .open(open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.label("Choose which columns to display in the library table.");
            ui.add_space(6.0);
            for column in ColumnKey::ALL {
                let mut visible = visible_columns.contains(&column);
                if ui.checkbox(&mut visible, column.label()).changed() {
                    if visible {
                        // Insert preserving canonical order.
                        *visible_columns = ColumnKey::ALL
                            .iter()
                            .copied()
                            .filter(|c| *c == column || visible_columns.contains(c))
                            .collect();
                    } else {
                        visible_columns.retain(|c| *c != column);
                    }
                    changed = true;
                }
            }
        });
    changed
}
