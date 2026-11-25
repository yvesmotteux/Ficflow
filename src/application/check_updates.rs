use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher, FanfictionOps};
use crate::domain::inbox::{InboxMessage, InboxOps, MessageContent};
use crate::error::FicflowError;
use chrono::Utc;

pub fn check_fic_updates(
    fetcher: &dyn FanfictionFetcher,
    fanfiction_ops: &dyn FanfictionOps,
    inbox_ops: &dyn InboxOps,
    fic_id: u64,
) -> Result<(Fanfiction, Option<InboxMessage>), FicflowError> {
    let current = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
    let fetched = fetcher.fetch_fanfiction(fic_id)?;

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

    let message = if updated.chapters_published > current.chapters_published {
        Some(
            inbox_ops.post_message(&MessageContent::NewChapters {
                fic_id,
                title: updated.title.clone(),
                authors: updated.authors.clone(),
                fandoms: updated.fandoms.clone(),
                previous_count: current.chapters_published,
                new_count: updated.chapters_published,
                first_new_chapter_id: updated
                    .chapter_ids
                    .get(current.chapters_published as usize)
                    .copied(),
            })?,
        )
    } else {
        None
    };

    Ok((updated, message))
}
