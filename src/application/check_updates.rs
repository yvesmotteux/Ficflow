use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher, FanfictionOps};
use crate::error::FicflowError;
use chrono::Utc;

pub fn check_fic_updates(
    fetcher: &dyn FanfictionFetcher,
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
) -> Result<(bool, Fanfiction), FicflowError> {
    let current = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
    let fetched = fetcher.fetch_fanfiction(fic_id)?;
    let has_new_chapters = fetched.chapters_published > current.chapters_published;

    // AO3 metadata comes from the fresh fetch; the user's own fields are kept.
    let updated = Fanfiction {
        last_chapter_read: current.last_chapter_read,
        reading_status: current.reading_status,
        read_count: current.read_count,
        user_rating: current.user_rating,
        personal_note: current.personal_note,
        last_checked_date: Utc::now(),
        ..fetched
    };
    fanfiction_ops.save_fanfiction(&updated)?;

    Ok((has_new_chapters, updated))
}
