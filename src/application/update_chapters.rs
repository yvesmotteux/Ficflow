use crate::domain::fanfiction::{DatabaseOps, ReadingStatus};
use std::error::Error;

pub fn update_last_chapter_read(
    db_ops: &dyn DatabaseOps, 
    fic_id: u64, 
    new_chapter_count: u32
) -> Result<(), Box<dyn Error>> {
    let mut fic = db_ops.get_fanfiction_by_id(fic_id)?;
    
    // Make sure new_chapter_count doesn't exceed total_chapters if total is known
    let adjusted_chapter_count = if let Some(total_chapters) = fic.chapters_total {
        if new_chapter_count > total_chapters {
            println!("Warning: Requested chapter {} exceeds total chapters {}. Setting to maximum.", 
                     new_chapter_count, total_chapters);
            total_chapters
        } else {
            new_chapter_count
        }
    } else {
        new_chapter_count
    };
    
    fic.last_chapter_read = Some(adjusted_chapter_count);
    
    // Check if this is the final chapter
    let is_final_chapter = if let Some(total_chapters) = fic.chapters_total {
        adjusted_chapter_count >= total_chapters
    } else {
        false
    };
    
    if is_final_chapter {
        fic.read_count += 1;
        fic.reading_status = ReadingStatus::Read;
    } else {
        match fic.reading_status {
            ReadingStatus::PlanToRead => fic.reading_status = ReadingStatus::InProgress,
            ReadingStatus::Paused => fic.reading_status = ReadingStatus::InProgress,
            _ => {}
        }
    }
    
    // Update the fanfiction in the database
    db_ops.update_fanfiction(&fic)?;
    
    Ok(())
}
