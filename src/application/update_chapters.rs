use crate::domain::fanfiction::{Fanfiction, FanfictionOps, ReadingStatus};
use crate::error::FicflowError;

pub fn update_last_chapter_read(
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
    new_chapter_count: u32,
) -> Result<Fanfiction, FicflowError> {
    let mut fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;

    let chapter = match fic.chapters_total {
        Some(total) if new_chapter_count > total => {
            log::warn!(
                "Requested chapter {} exceeds total chapters {}. Setting to maximum.",
                new_chapter_count,
                total
            );
            total
        }
        _ => new_chapter_count,
    };
    fic.last_chapter_read = Some(chapter);

    if fic.chapters_total.is_some_and(|total| chapter >= total) {
        fic.read_count += 1;
        fic.reading_status = ReadingStatus::Read;
    } else if matches!(
        fic.reading_status,
        ReadingStatus::PlanToRead | ReadingStatus::Paused
    ) {
        fic.reading_status = ReadingStatus::InProgress;
    }

    fanfiction_ops.save_fanfiction(&fic)?;

    Ok(fic)
}
