use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use regex::Regex;
use scraper::{Html, Selector};

use crate::domain::fanfiction::{ArchiveWarnings, Categories, Rating};
use crate::error::FicflowError;

pub struct Ao3Parser;

fn parse_selector(sel: &str) -> Selector {
    Selector::parse(sel).unwrap_or_else(|e| panic!("invalid CSS selector `{}`: {}", sel, e))
}

fn missing(field: &str) -> FicflowError {
    FicflowError::Parse {
        field: field.to_string(),
        reason: "element not found in HTML".to_string(),
    }
}

impl Ao3Parser {
    pub fn extract_title(&self, document: &Html) -> Result<String, FicflowError> {
        let selector = parse_selector("h2.title.heading");
        document
            .select(&selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .ok_or_else(|| missing("title"))
    }

    pub fn extract_authors(&self, document: &Html) -> Result<Vec<String>, FicflowError> {
        let selector = parse_selector("h3.byline.heading a[rel=\"author\"]");
        let authors = document
            .select(&selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if authors.is_empty() {
            return Err(missing("authors"));
        }

        Ok(authors)
    }

    pub fn extract_summary(&self, document: &Html) -> Result<String, FicflowError> {
        let summary_selector = parse_selector("div.summary.module blockquote.userstuff");

        if let Some(element) = document.select(&summary_selector).next() {
            let raw_summary = element.inner_html();
            let summary = self.strip_html_tags(&raw_summary);
            if !summary.is_empty() {
                return Ok(summary);
            }
        }

        let alt_summary_selector = parse_selector(".preface .summary");
        if let Some(element) = document.select(&alt_summary_selector).next() {
            let raw_summary = element.inner_html();
            let summary = self.strip_html_tags(&raw_summary);
            if !summary.is_empty() {
                return Ok(summary);
            }
        }

        Ok("No summary available".to_string())
    }

    fn strip_html_tags(&self, html: &str) -> String {
        let fragment = Html::parse_fragment(html);
        fragment
            .root_element()
            .text()
            .collect::<String>()
            .trim()
            .to_string()
    }

    pub fn extract_categories(
        &self,
        document: &Html,
    ) -> Result<Option<Vec<Categories>>, FicflowError> {
        let selector = parse_selector("dd.category.tags a.tag");
        let categories = document
            .select(&selector)
            .filter_map(|a| {
                let category_text = a.text().collect::<String>().trim().to_string();
                map_category(&category_text)
            })
            .collect::<Vec<Categories>>();

        if categories.is_empty() {
            Ok(None)
        } else {
            Ok(Some(categories))
        }
    }

    pub fn extract_chapters(
        &self,
        document: &Html,
    ) -> Result<(u32, Option<u32>, bool), FicflowError> {
        let selector = parse_selector("dd.chapters");
        let chapter_text = document
            .select(&selector)
            .next()
            .map(|element| element.text().collect::<String>())
            .unwrap_or_else(|| "0/0".to_string());

        let mut chapters_iter = chapter_text
            .split('/')
            .map(|s| s.parse::<u32>().unwrap_or(0));

        let chapters_published = chapters_iter.next().unwrap_or(0);
        let total_chapters = chapters_iter.next();

        let total_chapters = if total_chapters == Some(0) {
            None
        } else {
            total_chapters
        };

        let complete = match total_chapters {
            Some(total) => chapters_published > 0 && chapters_published == total,
            None => false,
        };

        Ok((chapters_published, total_chapters, complete))
    }

    pub fn extract_fandoms(&self, document: &Html) -> Result<Vec<String>, FicflowError> {
        let selector = parse_selector("dd.fandom.tags a.tag");
        let fandoms = document
            .select(&selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if fandoms.is_empty() {
            return Err(missing("fandoms"));
        }

        Ok(fandoms)
    }

    pub fn extract_stats(&self, document: &Html) -> Result<(u32, u32, u32), FicflowError> {
        let hits_selector = parse_selector("dd.hits");
        let hits = document
            .select(&hits_selector)
            .next()
            .map(|element| {
                let text = element.text().collect::<String>().trim().to_string();
                let cleaned_text = text.replace(",", "");
                cleaned_text.parse::<u32>().unwrap_or(0)
            })
            .unwrap_or(0);

        let kudos_selector = parse_selector("dd.kudos");
        let kudos = document
            .select(&kudos_selector)
            .next()
            .map(|element| {
                let text = element.text().collect::<String>().trim().to_string();
                let cleaned_text = text.replace(",", "");
                cleaned_text.parse::<u32>().unwrap_or(0)
            })
            .unwrap_or(0);

        let words_selector = parse_selector("dd.words");
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

    pub fn extract_language(&self, document: &Html) -> Result<String, FicflowError> {
        let selector = parse_selector("dd.language");
        let language = document
            .select(&selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "English".to_string());

        Ok(language)
    }

    pub fn extract_rating(&self, document: &Html) -> Result<Rating, FicflowError> {
        let selector = parse_selector("dd.rating.tags a.tag");
        let rating_text = document
            .select(&selector)
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

    pub fn extract_warnings(&self, document: &Html) -> Result<Vec<ArchiveWarnings>, FicflowError> {
        let selector = parse_selector("dd.warning.tags a.tag");
        let warnings = document
            .select(&selector)
            .filter_map(|element| {
                let warning_text = element.text().collect::<String>().trim().to_string();
                map_warning(&warning_text)
            })
            .collect::<Vec<ArchiveWarnings>>();

        if warnings.is_empty() {
            Ok(vec![ArchiveWarnings::NoArchiveWarningsApply])
        } else {
            Ok(warnings)
        }
    }

    pub fn extract_relationships(
        &self,
        document: &Html,
    ) -> Result<Option<Vec<String>>, FicflowError> {
        let selector = parse_selector("dd.relationship.tags a.tag");
        let relationships = document
            .select(&selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if relationships.is_empty() {
            Ok(None)
        } else {
            Ok(Some(relationships))
        }
    }

    pub fn extract_characters(&self, document: &Html) -> Result<Option<Vec<String>>, FicflowError> {
        let selector = parse_selector("dd.character.tags a.tag");
        let characters = document
            .select(&selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if characters.is_empty() {
            Ok(None)
        } else {
            Ok(Some(characters))
        }
    }

    pub fn extract_tags(&self, document: &Html) -> Result<Option<Vec<String>>, FicflowError> {
        let selector = parse_selector("dd.freeform.tags a.tag");
        let tags = document
            .select(&selector)
            .map(|element| element.text().collect::<String>().trim().to_string())
            .collect::<Vec<String>>();

        if tags.is_empty() {
            Ok(None)
        } else {
            Ok(Some(tags))
        }
    }

    pub fn extract_dates(
        &self,
        document: &Html,
    ) -> Result<(DateTime<Utc>, DateTime<Utc>), FicflowError> {
        let published_selector = parse_selector("dd.published");
        let published_text = document
            .select(&published_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let updated_selector = parse_selector("dd.status");
        let updated_text = document
            .select(&updated_selector)
            .next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .unwrap_or_else(|| published_text.clone());

        let published_date = parse_date(&published_text)?;
        let updated_date = parse_date(&updated_text)?;

        Ok((published_date, updated_date))
    }

    pub fn extract_restricted(&self, document: &Html) -> Result<bool, FicflowError> {
        let selector = parse_selector("p.notice");
        let restricted = document.select(&selector).any(|element| {
            let text = element.text().collect::<String>().to_lowercase();
            text.contains("only available to registered users")
                || text.contains("restricted to archive users")
        });

        Ok(restricted)
    }
}

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

fn parse_date(date_string: &str) -> Result<DateTime<Utc>, FicflowError> {
    let date_regex = Regex::new(r"(\d{4}-\d{2}-\d{2})").expect("invalid date regex");
    let date_str = match date_regex.captures(date_string) {
        Some(cap) => cap.get(1).map_or("", |m| m.as_str()),
        None => {
            return Err(FicflowError::Parse {
                field: "date".to_string(),
                reason: format!("no YYYY-MM-DD substring in `{}`", date_string),
            });
        }
    };

    let naive_date =
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| FicflowError::Parse {
            field: "date".to_string(),
            reason: format!("`{}`: {}", date_str, e),
        })?;

    let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap_or_default();
    Ok(Utc.from_utc_datetime(&datetime))
}
