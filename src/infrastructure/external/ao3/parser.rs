use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use regex::Regex;
use scraper::{Html, Selector};
use std::error::Error;

use crate::domain::fanfiction::{ArchiveWarnings, Categories, Rating};

pub struct Ao3Parser;

impl Ao3Parser {
    pub fn extract_title(&self, document: &Html) -> Result<String, Box<dyn Error>> {
        let title_selector = Selector::parse("h2.title.heading")?;
        let title = document
            .select(&title_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .ok_or_else(|| "Title not found".to_string())?;

        Ok(title)
    }

    pub fn extract_authors(&self, document: &Html) -> Result<Vec<String>, Box<dyn Error>> {
        let author_selector = Selector::parse("h3.byline.heading a[rel=\"author\"]")?;
        let authors = document
            .select(&author_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if authors.is_empty() {
            return Err("Authors not found".into());
        }

        Ok(authors)
    }

    pub fn extract_summary(&self, document: &Html) -> Result<String, Box<dyn Error>> {
        // Try primary selector: standard AO3 summary format
        let summary_selector = Selector::parse("div.summary.module blockquote.userstuff")?;
        
        if let Some(element) = document.select(&summary_selector).next() {
            // Use inner_html to get content with tags, then strip them
            let raw_summary = element.inner_html();
            let summary = self.strip_html_tags(&raw_summary);
            if !summary.is_empty() {
                return Ok(summary);
            }
        }
        
        // Try alternative selector: sometimes the summary is in a different location
        let alt_summary_selector = Selector::parse(".preface .summary")?;
        if let Some(element) = document.select(&alt_summary_selector).next() {
            let raw_summary = element.inner_html();
            let summary = self.strip_html_tags(&raw_summary);
            if !summary.is_empty() {
                return Ok(summary);
            }
        }
        
        // If no summary is found, return a default one rather than failing
        Ok("No summary available".to_string())
    }

    // Helper function to strip HTML tags from a string
    fn strip_html_tags(&self, html: &str) -> String {
        let fragment = Html::parse_fragment(html);
        fragment.root_element().text().collect::<String>().trim().to_string()
    }

    pub fn extract_categories(&self, document: &Html) -> Result<Option<Vec<Categories>>, Box<dyn Error>> {
        let categories_selector = Selector::parse("dd.category.tags a.tag")?;
        let categories = document
            .select(&categories_selector)
            .filter_map(|a| {
                let category_text = a.text().collect::<String>().trim().to_string();
                map_category(&category_text) // Map the category string to Categories enum
            })
            .collect::<Vec<Categories>>();

        if categories.is_empty() {
            Ok(None)
        } else {
            Ok(Some(categories))
        }
    }

    pub fn extract_chapters(&self, document: &Html) -> Result<(u32, Option<u32>, bool), Box<dyn Error>> {
        let chapters_selector = Selector::parse("dd.chapters")?;
        let chapter_text = document
            .select(&chapters_selector)
            .next()
            .map(|element| element.text().collect::<String>())
            .unwrap_or_else(|| "0/0".to_string());

        let mut chapters_iter = chapter_text.split('/').map(|s| s.parse::<u32>().unwrap_or(0));

        let chapters_published = chapters_iter.next().unwrap_or(0);
        let total_chapters = chapters_iter.next();

        // If total chapters is 0 or ? (parsed as 0), then it's None
        let total_chapters = if total_chapters == Some(0) { None } else { total_chapters };

        // Complete (whether the fic is completed or not)
        let complete = match total_chapters {
            Some(total) => chapters_published > 0 && chapters_published == total,
            None => false, // If total chapters is unknown, it's likely not complete
        };

        Ok((chapters_published, total_chapters, complete))
    }

    pub fn extract_fandoms(&self, document: &Html) -> Result<Vec<String>, Box<dyn Error>> {
        let fandom_selector = Selector::parse("dd.fandom.tags a.tag")?;
        let fandoms = document
            .select(&fandom_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if fandoms.is_empty() {
            return Err("Fandoms not found".into());
        }

        Ok(fandoms)
    }

    pub fn extract_stats(&self, document: &Html) -> Result<(u32, u32, u32), Box<dyn Error>> {
        // Hits
        let hits_selector = Selector::parse("dd.hits")?;
        let hits = document
            .select(&hits_selector)
            .next()
            .map(|element| {
                let text = element.text().collect::<String>().trim().to_string();
                let cleaned_text = text.replace(",", "");
                cleaned_text.parse::<u32>().unwrap_or(0)
            })
            .unwrap_or(0);

        // Kudos
        let kudos_selector = Selector::parse("dd.kudos")?;
        let kudos = document
            .select(&kudos_selector)
            .next()
            .map(|element| {
                let text = element.text().collect::<String>().trim().to_string();
                let cleaned_text = text.replace(",", "");
                cleaned_text.parse::<u32>().unwrap_or(0)
            })
            .unwrap_or(0);

        // Words
        let words_selector = Selector::parse("dd.words")?;
        let words = document
            .select(&words_selector)
            .next()
            .map(|element| {
                let text = element.text().collect::<String>().trim().to_string();
                let cleaned_text = text.replace(",", "");
                cleaned_text.parse::<u32>().unwrap_or(0)
            })
            .unwrap_or(0);

        Ok((hits, kudos, words))
    }

    pub fn extract_language(&self, document: &Html) -> Result<String, Box<dyn Error>> {
        let language_selector = Selector::parse("dd.language")?;
        let language = document
            .select(&language_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "English".to_string());

        Ok(language)
    }

    pub fn extract_rating(&self, document: &Html) -> Result<Rating, Box<dyn Error>> {
        let rating_selector = Selector::parse("dd.rating.tags a.tag")?;
        let rating_text = document
            .select(&rating_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "Not Rated".to_string());

        let rating = match rating_text.as_str() {
            "General Audiences" => Rating::General,
            "Teen And Up Audiences" => Rating::TeenAndUp,
            "Mature" => Rating::Mature,
            "Explicit" => Rating::Explicit,
            _ => Rating::NotRated,
        };

        Ok(rating)
    }

    pub fn extract_warnings(&self, document: &Html) -> Result<Vec<ArchiveWarnings>, Box<dyn Error>> {
        let warnings_selector = Selector::parse("dd.warning.tags a.tag")?;
        let warnings = document
            .select(&warnings_selector)
            .filter_map(|element| {
                let warning_text = element.text().collect::<String>().trim().to_string();
                map_warning(&warning_text)
            })
            .collect::<Vec<ArchiveWarnings>>();

        // Default to "No Archive Warnings Apply" if no warnings are found
        if warnings.is_empty() {
            Ok(vec![ArchiveWarnings::NoArchiveWarningsApply])
        } else {
            Ok(warnings)
        }
    }

    pub fn extract_relationships(&self, document: &Html) -> Result<Option<Vec<String>>, Box<dyn Error>> {
        let relationship_selector = Selector::parse("dd.relationship.tags a.tag")?;
        let relationships = document
            .select(&relationship_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if relationships.is_empty() {
            Ok(None)
        } else {
            Ok(Some(relationships))
        }
    }

    pub fn extract_characters(&self, document: &Html) -> Result<Option<Vec<String>>, Box<dyn Error>> {
        let character_selector = Selector::parse("dd.character.tags a.tag")?;
        let characters = document
            .select(&character_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if characters.is_empty() {
            Ok(None)
        } else {
            Ok(Some(characters))
        }
    }

    pub fn extract_tags(&self, document: &Html) -> Result<Option<Vec<String>>, Box<dyn Error>> {
        let tag_selector = Selector::parse("dd.freeform.tags a.tag")?;
        let tags = document
            .select(&tag_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if tags.is_empty() {
            Ok(None)
        } else {
            Ok(Some(tags))
        }
    }

    pub fn extract_dates(&self, document: &Html) -> Result<(DateTime<Utc>, DateTime<Utc>), Box<dyn Error>> {
        let published_selector = Selector::parse("dd.published")?;
        let published_text = document
            .select(&published_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let updated_selector = Selector::parse("dd.status")?;
        let updated_text = document
            .select(&updated_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| published_text.clone()); // Use published date if updated not found

        let published_date = parse_date(&published_text)?;
        let updated_date = parse_date(&updated_text)?;

        Ok((published_date, updated_date))
    }

    pub fn extract_restricted(&self, document: &Html) -> Result<bool, Box<dyn Error>> {
        // Check if there's a "This work is only available to registered users of the Archive" message
        let restricted_selector = Selector::parse("p.notice")?;
        let restricted = document
            .select(&restricted_selector)
            .any(|element| {
                let text = element.text().collect::<String>().to_lowercase();
                text.contains("only available to registered users") || 
                text.contains("restricted to archive users")
            });

        Ok(restricted)
    }
}

// Helper function to map category text to Categories enum
fn map_category(category_text: &str) -> Option<Categories> {
    match category_text {
        "F/F" => Some(Categories::FF),
        "F/M" => Some(Categories::FM),
        "M/M" => Some(Categories::MM),
        "Gen" => Some(Categories::Gen),
        "Multi" => Some(Categories::Multi),
        "Other" => Some(Categories::Other),
        _ => None,
    }
}

// Helper function to map warning text to ArchiveWarnings enum
fn map_warning(warning_text: &str) -> Option<ArchiveWarnings> {
    match warning_text {
        "Creator Chose Not To Use Archive Warnings" => Some(ArchiveWarnings::ChooseNotToUse),
        "Graphic Depictions Of Violence" => Some(ArchiveWarnings::GraphicDepictionsOfViolence),
        "Major Character Death" => Some(ArchiveWarnings::MajorCharacterDeath),
        "No Archive Warnings Apply" => Some(ArchiveWarnings::NoArchiveWarningsApply),
        "Rape/Non-Con" => Some(ArchiveWarnings::RapeNonCon),
        "Underage" => Some(ArchiveWarnings::Underage),
        _ => None,
    }
}

// Helper function to parse date strings from AO3
fn parse_date(date_string: &str) -> Result<DateTime<Utc>, Box<dyn Error>> {
    // AO3 date format is like: "2020-10-31" or sometimes with additional text

    // Extract just the date part (YYYY-MM-DD)
    let date_regex = Regex::new(r"(\d{4}-\d{2}-\d{2})")?;
    let date_str = match date_regex.captures(date_string) {
        Some(cap) => cap.get(1).map_or("", |m| m.as_str()),
        None => return Err(format!("Could not parse date: {}", date_string).into()),
    };

    // Parse the date
    let naive_date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")?;

    // Convert to DateTime<Utc> (using midnight UTC as the time)
    // let datetime = Utc.from_utc_datetime(&naive_date.and_hms(0, 0, 0));
    let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap_or_default();
    Ok(Utc.from_utc_datetime(&datetime))
}
