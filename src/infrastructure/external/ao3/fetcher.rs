use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher, ReadingStatus};
use crate::infrastructure::external::ao3::ao3_client::Ao3Client;
use crate::infrastructure::external::ao3::parser::Ao3Parser;
use chrono::Utc;
use scraper::Html;
use std::error::Error;

pub struct Ao3Fetcher {
    client: Ao3Client,
    parser: Ao3Parser,
}

impl Ao3Fetcher {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let client = Ao3Client::new()?;
        let parser = Ao3Parser;
        
        Ok(Self { client, parser })
    }
}

impl Default for Ao3Fetcher {
    fn default() -> Self {
        Self::new().expect("Failed to create Ao3Fetcher")
    }
}

impl FanfictionFetcher for Ao3Fetcher {
    fn fetch_fanfiction(&self, fic_id: u64, base_url: &str) -> Result<Fanfiction, Box<dyn Error>> {
        // Fetch the HTML content
        let response = self.client.fetch_work(fic_id, base_url)?;
        
        // Parse the HTML document
        let document = Html::parse_document(&response);
        
        // Check if the work is restricted
        let restricted = self.parser.extract_restricted(&document)?;
        
        // Parse all the information from the document
        let title = self.parser.extract_title(&document)?;
        let authors = self.parser.extract_authors(&document)?;
        let summary = self.parser.extract_summary(&document)?;
        let categories = self.parser.extract_categories(&document)?;
        let (chapters_published, chapters_total, complete) = self.parser.extract_chapters(&document)?;
        let fandoms = self.parser.extract_fandoms(&document)?;
        let (hits, kudos, words) = self.parser.extract_stats(&document)?;
        let language = self.parser.extract_language(&document)?;
        let rating = self.parser.extract_rating(&document)?;
        let warnings = self.parser.extract_warnings(&document)?;
        let relationships = self.parser.extract_relationships(&document)?;
        let characters = self.parser.extract_characters(&document)?;
        let tags = self.parser.extract_tags(&document)?;
        let (date_published, date_updated) = self.parser.extract_dates(&document)?;
        
        // Construct and return the Fanfiction object
        Ok(Fanfiction {
            id: fic_id,
            title,
            authors,
            categories,
            chapters_total,
            chapters_published,
            characters,
            complete,
            fandoms,
            hits,
            kudos,
            language,
            rating,
            relationships,
            restricted,
            summary,
            tags,
            warnings,
            words,
            date_published,
            date_updated,
            last_chapter_read: None,
            reading_status: ReadingStatus::PlanToRead,
            read_count: 0,
            user_rating: None,
            personal_note: None,
            last_checked_date: Utc::now(),
        })
    }
}
