use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use reqwest::{blocking::Client, header::{HeaderMap, USER_AGENT}};
use scraper::{Html, Selector};
use std::error::Error;
use crate::domain::fic::{ArchiveWarnings, Categories, Fanfiction, FanfictionFetcher, Rating, ReadingStatus};

pub struct Ao3Fetcher;

impl FanfictionFetcher for Ao3Fetcher {
    fn fetch_fanfiction(&self, fic_id: u64, base_url: &str) -> Result<Fanfiction, Box<dyn Error>> {
        let url = format!("{}/works/{}", base_url, fic_id);
        let client = Client::new();
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36".parse().unwrap());
    
        let response = client.get(&url).headers(headers).send()?.text()?;
    
        let document = Html::parse_document(&response);
    
        // Title
        let title_selector = Selector::parse("h2.title.heading")?;
        let title = document
            .select(&title_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .ok_or("Title not found")?;
    
        // Authors
        let author_selector = Selector::parse("h3.byline.heading")?;
        let authors = document
            .select(&author_selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();
    
        // Summary
        let summary_selector = Selector::parse("div.summary.module blockquote.userstuff")?;
        let summary = document
            .select(&summary_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .ok_or("Summary not found")?;
    
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
                println!("Date string: {}", date_str);
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
        "General Audience" => Rating::General,
        "Teen And Up Audiences" => Rating::TeenAndUp,
        "Mature" => Rating::Mature,
        "Explicit" => Rating::Explicit,
        _ => Rating::NotRated, // Default to NotRated if it's not a recognized rating
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