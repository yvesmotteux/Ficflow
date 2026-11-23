use crate::domain::fanfiction::{Fanfiction, ReadingStatus};
use crate::interfaces::utils::formatter;
use term_table::row::Row;
use term_table::table_cell::{Alignment, TableCell};
use term_table::{Table, TableStyle};

pub fn render_fanfiction_list(fanfictions: &[Fanfiction]) -> String {
    if fanfictions.is_empty() {
        return "No fanfictions found in your library.".to_string();
    }

    let mut output = format!(
        "Found {} fanfictions in your library:\n\n",
        fanfictions.len()
    );

    // Create a table for better display
    let mut table = Table::new();
    table.style = TableStyle::thin();

    // Add header row with centered headers
    table.add_row(Row::new(vec![
        aligned("ID", Alignment::Center),
        aligned("Title", Alignment::Center),
        aligned("Author(s)", Alignment::Center),
        aligned("Fandom(s)", Alignment::Center),
        aligned("Words", Alignment::Center),
        aligned("Status", Alignment::Center),
    ]));

    // Add fanfiction rows
    for fic in fanfictions {
        let authors = fic.authors.join(", ");
        let fandoms = fic.fandoms.join(", ");

        // Format the status (with a symbol)
        let status = match fic.reading_status {
            ReadingStatus::PlanToRead => "📚 Plan to Read",
            ReadingStatus::InProgress => "📖 In Progress",
            ReadingStatus::Read => "✅ Read",
            ReadingStatus::Paused => "⏸️ Paused",
            ReadingStatus::Abandoned => "❌ Abandoned",
        };

        // Create a row with cells
        let row_cells = vec![
            TableCell::new(fic.id),
            TableCell::new(&fic.title),
            TableCell::new(authors),
            TableCell::new(fandoms),
            aligned(formatter::format_thousands(fic.words), Alignment::Right),
            TableCell::new(status),
        ];

        table.add_row(Row::new(row_cells));
    }

    output.push_str(&table.render());
    output
}

fn aligned<T: ToString>(data: T, alignment: Alignment) -> TableCell {
    TableCell::builder(data).alignment(alignment).build()
}
