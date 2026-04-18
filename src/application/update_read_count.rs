use crate::domain::fanfiction::{Fanfiction, FanfictionOps, ReadingStatus};
use crate::error::FicflowError;

pub fn update_read_count(
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
    new_read_count: u32,
) -> Result<Fanfiction, FicflowError> {
    let mut fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
    fic.read_count = new_read_count;

    if new_read_count == 0 && fic.reading_status == ReadingStatus::Read {
        log::info!("Read count set to 0. Changing status from Read to Plan To Read.");
        fic.reading_status = ReadingStatus::PlanToRead;
    }

    fanfiction_ops.save_fanfiction(&fic)?;
    Ok(fic)
}
