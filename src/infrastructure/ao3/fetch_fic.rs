use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use reqwest::{blocking::Client, header::{HeaderMap, USER_AGENT}};
use scraper::{Html, Selector};
use std::error::Error;
use crate::domain::fic::{ArchiveWarnings, Categories, Fanfiction, FanfictionFetcher, Rating, ReadingStatus};

pub struct Ao3Fetcher;

impl FanfictionFetcher for Ao3Fetcher {
    fn fetch_fanfiction(&self, fic_id: u64, base_url: &str) -> Result<Fanfiction, Box<dyn Error>> {
        let url = format!("{}/works/{}", base_url, fic_id);
        
        // Create a client with a longer timeout
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))  // Set a 60-second timeout
            .build()?;
            
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36".parse().unwrap());
    
        let response = client.get(&url).headers(headers).send()?.text()?;

        let document = Html::parse_document(&response);
        
        // Title
        let title = self.extract_title(&document)?;
    
        // Authors
        let author_selector = Selector::parse("h3.byline.heading")?;
        let authors = document
            .select(&author_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();
    
        // Summary
        let summary = self.extract_summary(&document)?;
    
        // Categories
        let categories_selector = Selector::parse("dd.category.tags")?;
        let categories = document
            .select(&categories_selector)
            .flat_map(|element| {
                element.select(&Selector::parse("li a.tag").unwrap())
                    .filter_map(|a| {
                        let category_text = a.text().collect::<String>().trim().to_string();
                        map_category(&category_text) // Map the category string to Categories enum
                    })
                    .collect::<Vec<Categories>>()
            })
            .collect::<Vec<Categories>>();
    
        // Chapters
        let chapters_selector = Selector::parse("dd.chapters")?;
        let chapter_text = document
            .select(&chapters_selector)
            .next()
            .map(|element| element.text().collect::<String>())
            .unwrap_or_else(|| "0/0".to_string());
        let mut chapters_iter = chapter_text.split('/').map(|s| s.parse::<u32>().unwrap_or(0));
    
        let chapters_published = chapters_iter.next().unwrap_or(0);
        let total_chapters = chapters_iter.next().unwrap_or(0);
    
        // Complete (whether the fic is completed or not)
        let complete = chapters_published > 0 && chapters_published == total_chapters;
    
        // Fandoms
        let fandom_selector = Selector::parse("dd.fandom.tags a.tag")?;
        let fandoms = document
            .select(&fandom_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();
    
        // Hits, Kudos & Words
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
    
        // Language
        let language_selector = Selector::parse("dd.language")?;
        let language = document
            .select(&language_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or("Unknown".to_string());
    
        // Rating
        let rating_selector = Selector::parse("dd.rating.tags")?;
        let rating = document
            .select(&rating_selector)
            .next()
            .map(|element| {
                let rating_text = element.text().collect::<String>().trim().to_string();
                map_rating(&rating_text) // Map the rating string to Rating enum
            })
            .unwrap_or(Rating::NotRated);
    
        // Warnings, Tags, Characters, Relationships
        let warnings_selector = Selector::parse("dd.warning.tags li")?;
        let warnings = document
            .select(&warnings_selector)
            .map(|element| {
                let warning_text = element.text().collect::<String>().trim().to_string();
                map_warning(&warning_text).unwrap_or(ArchiveWarnings::ChooseNotToUse)
            })
            .collect::<Vec<ArchiveWarnings>>();
    
        let tags_selector = Selector::parse("dd.freeform.tags a.tag")?;
        let tags = document
            .select(&tags_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();
    
        let characters_selector = Selector::parse("dd.character.tags a.tag")?;
        let characters = document
            .select(&characters_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();
    
        let relationships_selector = Selector::parse("dd.relationship.tags a.tag")?;
        let relationships = document
            .select(&relationships_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();
    
        // Date Published and Date Updated
        let date_selector = Selector::parse("dd.published")?;
        let date_published = document
            .select(&date_selector)
            .next()
            .and_then(|element| {
                let date_str = element.text().collect::<String>().trim().to_string();
                NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .ok()
                .and_then(|naive_date| {
                    naive_date.and_hms_opt(0, 0, 0)
                })
                .map(|naive_date_time| {
                    DateTime::<Utc>::from_naive_utc_and_offset(naive_date_time, Utc)
                })
            })
            .unwrap_or_else(|| Utc.timestamp_opt(0, 0).single().unwrap());
    
        let date_updated_selector = Selector::parse("dd.status")?;
        let date_updated = document
            .select(&date_updated_selector)
            .next()
            .and_then(|element| {
                let date_str = element.text().collect::<String>().trim().to_string();
                let split: Vec<&str> = date_str.split('-').collect();
                if split.len() == 3 {
                    NaiveDate::from_ymd_opt(
                        split[0].parse::<i32>().ok()?,
                        split[1].parse::<u32>().ok()?,
                        split[2].parse::<u32>().ok()?,
                    )
                    .and_then(|naive_date| naive_date.and_hms_opt(0, 0, 0)) 
                    .map(|naive_date| DateTime::<Utc>::from_naive_utc_and_offset(naive_date, Utc))
                } else {
                    None
                }
            })
            .unwrap_or(date_published);
    
        let fanfiction = Fanfiction {
            id: fic_id,
            title,
            authors,
            categories: Some(categories),
            chapters_total: if total_chapters == 0 { None } else { Some(total_chapters) },
            chapters_published,
            characters: Some(characters),
            complete,
            fandoms,
            hits,
            kudos,
            language,
            rating,
            relationships: Some(relationships),
            restricted: false, // TODO: Implement this
            summary,
            tags: Some(tags),
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
        };
    
        Ok(fanfiction)
    }
}

impl Ao3Fetcher {
    fn extract_title(&self, document: &Html) -> Result<String, Box<dyn Error>> {
        // Try primary selector: h2.title.heading (standard AO3 title format)
        let title_selector = Selector::parse("h2.title.heading").unwrap_or_else(|_| Selector::parse("h2").unwrap());
        if let Some(element) = document.select(&title_selector).next() {
            let title = element.text().collect::<String>().trim().to_string();
            if !title.is_empty() {
                return Ok(self.clean_title(title));
            }
        }

        // Try secondary selector: head > title (use page title as fallback)
        let head_title_selector = Selector::parse("head > title").unwrap_or_else(|_| Selector::parse("title").unwrap());
        if let Some(element) = document.select(&head_title_selector).next() {
            let full_title = element.text().collect::<String>().trim().to_string();
            
            // AO3 page titles typically follow the format: 
            // "Title - Author - Fandom [Archive of Our Own]"
            if let Some(dash_pos) = full_title.find(" - ") {
                return Ok(self.clean_title(full_title[..dash_pos].trim().to_string()));
            }
            
            // If we can't parse it properly, just return the cleaned full title
            if !full_title.is_empty() {
                return Ok(self.clean_title(full_title));
            }
        }

        // Try tertiary selector: .preface .title (alternative title location)
        let preface_title_selector = Selector::parse(".preface .title").unwrap_or_else(|_| Selector::parse(".title").unwrap());
        if let Some(element) = document.select(&preface_title_selector).next() {
            let title = element.text().collect::<String>().trim().to_string();
            if !title.is_empty() {
                return Ok(self.clean_title(title));
            }
        }

        // If all selectors fail, return a generic title rather than failing
        Ok("Unknown Title".to_string())
    }
    
    // New method to clean up titles that might contain error messages or unintended text
    fn clean_title(&self, title: String) -> String {
        // Remove common prefixes that indicate errors
        let title = title.trim();
        
        // Check for known error patterns
        if title.starts_with("archiveofourown.org") || 
           title.contains("SSL handshake failed") ||
           title.contains("404") ||
           title.contains("Not Found") ||
           title.contains("Error") {
            return "Unknown Title (Error Loading)".to_string();
        }
        
        // Remove HTML artifacts
        let title = title.replace("&nbsp;", " ")
                         .replace("&amp;", "&")
                         .replace("&lt;", "<")
                         .replace("&gt;", ">")
                         .replace("&#39;", "'")
                         .replace("&quot;", "\"");
        
        // Remove AO3 suffix if present (e.g., "| Archive of Our Own")
        if let Some(index) = title.find("|") {
            let potential_suffix = title[index..].to_lowercase();
            if potential_suffix.contains("archive") || potential_suffix.contains("our own") {
                return title[..index].trim().to_string();
            }
        }
        
        title.to_string()
    }
    
    fn extract_summary(&self, document: &Html) -> Result<String, Box<dyn Error>> {
        // Try primary selector: standard AO3 summary format
        let summary_selector = Selector::parse("div.summary.module blockquote.userstuff").unwrap_or_else(|_| {
            Selector::parse("div.summary blockquote").unwrap()
        });
        
        if let Some(element) = document.select(&summary_selector).next() {
            let summary = element.text().collect::<String>().trim().to_string();
            if !summary.is_empty() {
                return Ok(summary);
            }
        }
        
        // Try alternative selector: sometimes the summary is in a different location
        let alt_summary_selector = Selector::parse(".preface .summary").unwrap_or_else(|_| {
            Selector::parse(".summary").unwrap()
        });
        
        if let Some(element) = document.select(&alt_summary_selector).next() {
            let summary = element.text().collect::<String>().trim().to_string();
            if !summary.is_empty() {
                return Ok(summary);
            }
        }
        
        // If no summary is found, return a default one rather than failing
        Ok("No summary available".to_string())
    }
}

fn map_category(category: &str) -> Option<Categories> {
    match category {
        "F/F" => Some(Categories::FF),
        "F/M" => Some(Categories::FM),
        "M/M" => Some(Categories::MM),
        "Gen" => Some(Categories::Gen),
        "Other" => Some(Categories::Other),
        "Multi" => Some(Categories::Multi),
        _ => None, // If it's not a recognized category, return None
    }
}

fn map_rating(rating: &str) -> Rating {
    match rating {
        "Not Rated" => Rating::NotRated,
        "General Audiences" => Rating::General,
        "General Audience" => Rating::General, // Keep for backward compatibility
        "Teen And Up Audiences" => Rating::TeenAndUp,
        "Mature" => Rating::Mature,
        "Explicit" => Rating::Explicit,
        _ => {
            eprintln!("Unrecognized rating: '{}' - defaulting to NotRated", rating);
            Rating::NotRated
        }
    }
}

fn map_warning(warning: &str) -> Option<ArchiveWarnings> {
    match warning {
        "Choose Not To Use" => Some(ArchiveWarnings::ChooseNotToUse),
        "Graphic Depictions Of Violence" => Some(ArchiveWarnings::GraphicDepictionsOfViolence),
        "Major Character Death" => Some(ArchiveWarnings::MajorCharacterDeath),
        "No Archive Warnings Apply" => Some(ArchiveWarnings::NoArchiveWarningsApply),
        "Rape/Non-Con" => Some(ArchiveWarnings::RapeNonCon),
        "Underage" => Some(ArchiveWarnings::Underage),
        _ => None, // If the warning isn't recognized, return None
    }
}