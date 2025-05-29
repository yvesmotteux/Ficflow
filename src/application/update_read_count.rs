use crate::domain::fanfiction::{DatabaseOps, ReadingStatus};
use std::error::Error;

pub fn update_read_count(
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    new_read_count: u32
) -> Result<(), Box<dyn Error>> {
    // Get the current fanfiction
    let mut fic = db_ops.get_fanfiction_by_id(fic_id)?;
    
    // Update the read count
    fic.read_count = new_read_count;
    
    // If read count is set to 0 and status was Read, change to PlanToRead
    if new_read_count == 0 && fic.reading_status == ReadingStatus::Read {
        println!("Read count set to 0. Changing status from Read to Plan To Read.");
        fic.reading_status = ReadingStatus::PlanToRead;
    }
    
    // Update the fanfiction in the database
    db_ops.update_fanfiction(&fic)?;
    
    Ok(())
}
