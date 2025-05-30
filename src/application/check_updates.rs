use crate::domain::fanfiction::{DatabaseOps, FanfictionFetcher, Fanfiction};
use chrono::Utc;

pub fn check_fic_updates(
    fetcher: &dyn FanfictionFetcher,
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    base_url: &str,
) -> Result<(bool, Fanfiction), Box<dyn std::error::Error>> {
    let mut current_fic = match db_ops.get_fanfiction_by_id(fic_id) {
        Ok(fic) => fic,
        Err(e) => return Err(format!("Failed to get fanfiction from database: {}", e).into()),
    };
    
    let new_fic = match fetcher.fetch_fanfiction(fic_id, base_url) {
        Ok(fic) => fic,
        Err(e) => return Err(format!("Failed to fetch updated fanfiction data: {}", e).into()),
    };
    
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
    
    // Update last checked date
    current_fic.last_checked_date = Utc::now();
    
    match db_ops.update_fanfiction(&current_fic) {
        Ok(_) => {},
        Err(e) => return Err(format!("Failed to update fanfiction in database: {}", e).into()),
    }
    
    // Return whether there are new chapters and the updated fanfiction object
    Ok((has_new_chapters, current_fic))
}
