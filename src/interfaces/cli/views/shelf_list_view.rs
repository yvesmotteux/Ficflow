use std::collections::HashMap;

use crate::domain::shelf::Shelf;
use term_table::row::Row;
use term_table::table_cell::{Alignment, TableCell};
use term_table::{Table, TableStyle};

pub fn render_shelf_list(shelves: &[Shelf]) -> String {
    if shelves.is_empty() {
        return "No shelves found. Create one with `ficflow shelf create <name>`.".to_string();
    }

    let mut output = format!("Found {} shelves:\n\n", shelves.len());

    let mut table = Table::new();
    table.style = TableStyle::thin();

    #[allow(deprecated)]
    table.add_row(Row::new(vec![
        TableCell::new_with_alignment("ID", 1, Alignment::Center),
        TableCell::new_with_alignment("Name", 1, Alignment::Center),
        TableCell::new_with_alignment("Created", 1, Alignment::Center),
    ]));

    let mut children: HashMap<Option<u64>, Vec<&Shelf>> = HashMap::new();
    for shelf in shelves {
        children
            .entry(shelf.parent_shelf_id)
            .or_default()
            .push(shelf);
    }
    add_rows(&mut table, &children, None, 0);

    output.push_str(&table.render());
    output
}

fn add_rows(
    table: &mut Table,
    children: &HashMap<Option<u64>, Vec<&Shelf>>,
    parent: Option<u64>,
    depth: usize,
) {
    let Some(siblings) = children.get(&parent) else {
        return;
    };
    for shelf in siblings {
        #[allow(deprecated)]
        let row_cells = vec![
            TableCell::new_with_alignment(shelf.id, 1, Alignment::Right),
            TableCell::new(format!("{}{}", "  ".repeat(depth), shelf.name)),
            TableCell::new(shelf.created_at.format("%Y-%m-%d").to_string()),
        ];
        table.add_row(Row::new(row_cells));
        add_rows(table, children, Some(shelf.id), depth + 1);
    }
}
