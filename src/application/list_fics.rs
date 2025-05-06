use crate::domain::db::DatabaseOps;
use crate::domain::fic::ReadingStatus;
use std::error::Error;
use term_table::{Table, TableStyle};
use term_table::row::Row;
use term_table::table_cell::{Alignment, TableCell};

pub fn list_fics(db_ops: &dyn DatabaseOps) -> Result<(), Box<dyn Error>> {
    let fanfictions = db_ops.list_fanfictions()?;
    
    if fanfictions.is_empty() {
        println!("No fanfictions found in your library.");
        return Ok(());
    }
    
    println!("Found {} fanfictions in your library:\n", fanfictions.len());
    
    // Create a table for better display
    let mut table = Table::new();
    table.style = TableStyle::thin();
    
    // Add header row with centered headers
    // Use #[allow(deprecated)] to silence warnings about new_with_alignment
    #[allow(deprecated)]
    table.add_row(Row::new(vec![
        TableCell::new_with_alignment("ID", 1, Alignment::Center),
        TableCell::new_with_alignment("Title", 1, Alignment::Center),
        TableCell::new_with_alignment("Author(s)", 1, Alignment::Center),
        TableCell::new_with_alignment("Fandom(s)", 1, Alignment::Center),
        TableCell::new_with_alignment("Words", 1, Alignment::Center),
        TableCell::new_with_alignment("Status", 1, Alignment::Center),
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
        // Use #[allow(deprecated)] to silence warnings about new_with_alignment
        #[allow(deprecated)]
        let row_cells = vec![
            TableCell::new(fic.id),
            TableCell::new(fic.title),
            TableCell::new(authors),
            TableCell::new(fandoms),
            TableCell::new_with_alignment(format_word_count(fic.words), 1, Alignment::Right),
            TableCell::new(status),
        ];
        
        table.add_row(Row::new(row_cells));
    }
    
    println!("{}", table.render());
    Ok(())
}

// Helper function to format word count with comma separators
fn format_word_count(words: u32) -> String {
    let words_str = words.to_string();
    let mut result = String::new();
    let len = words_str.len();
    
    for (i, c) in words_str.chars().enumerate() {
        result.push(c);
        if (len - i - 1) % 3 == 0 && i < len - 1 {
            result.push(',');
        }
    }
    
    result
}