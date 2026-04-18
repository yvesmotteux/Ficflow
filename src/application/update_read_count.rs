use crate::domain::fanfiction::{DatabaseOps, ReadingStatus};
use crate::error::FicflowError;

pub fn update_read_count(
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    new_read_count: u32,
) -> Result<(), FicflowError> {
    let mut fic = db_ops.get_fanfiction_by_id(fic_id)?;
    fic.read_count = new_read_count;

    if new_read_count == 0 && fic.reading_status == ReadingStatus::Read {
        log::info!("Read count set to 0. Changing status from Read to Plan To Read.");
        fic.reading_status = ReadingStatus::PlanToRead;
    }

    db_ops.save_fanfiction(&fic)?;
    Ok(())
}
