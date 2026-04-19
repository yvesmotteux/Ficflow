use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher, FanfictionOps};
use crate::error::FicflowError;
use chrono::Utc;

pub fn check_fic_updates(
    fetcher: &dyn FanfictionFetcher,
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
) -> Result<(bool, Fanfiction), FicflowError> {
    let mut current_fic = fanfiction_ops.get_fanfiction_by_id(fic_id)?;
    let new_fic = fetcher.fetch_fanfiction(fic_id)?;

    let has_new_chapters = new_fic.chapters_published > current_fic.chapters_published;

    // Only update fields that can change over time, preserving user's custom fields
    current_fic.title = new_fic.title;
    current_fic.authors = new_fic.authors;
    current_fic.categories = new_fic.categories;
    current_fic.chapters_total = new_fic.chapters_total;
    current_fic.chapters_published = new_fic.chapters_published;
    current_fic.characters = new_fic.characters;
    current_fic.complete = new_fic.complete;
    current_fic.fandoms = new_fic.fandoms;
    current_fic.hits = new_fic.hits;
    current_fic.kudos = new_fic.kudos;
    current_fic.language = new_fic.language;
    current_fic.rating = new_fic.rating;
    current_fic.relationships = new_fic.relationships;
    current_fic.restricted = new_fic.restricted;
    current_fic.summary = new_fic.summary;
    current_fic.tags = new_fic.tags;
    current_fic.warnings = new_fic.warnings;
    current_fic.words = new_fic.words;
    current_fic.date_published = new_fic.date_published;
    current_fic.date_updated = new_fic.date_updated;

    current_fic.last_checked_date = Utc::now();

    fanfiction_ops.save_fanfiction(&current_fic)?;

    Ok((has_new_chapters, current_fic))
}
