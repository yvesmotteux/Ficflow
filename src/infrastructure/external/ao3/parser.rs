use std::sync::LazyLock;

use chrono::{DateTime, NaiveDate, Utc};
use regex::Regex;
use scraper::{ElementRef, Html, Selector};

use crate::domain::fanfiction::{ArchiveWarnings, Categories, Fanfiction, Rating, ReadingStatus};
use crate::error::FicflowError;

macro_rules! selector {
    ($css:literal) => {{
        static SELECTOR: LazyLock<Selector> =
            LazyLock::new(|| Selector::parse($css).expect("valid CSS selector"));
        &*SELECTOR
    }};
}

static DATE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d{4}-\d{2}-\d{2})").expect("valid date regex"));

pub fn parse_work(fic_id: u64, html: &str) -> Result<Fanfiction, FicflowError> {
    let doc = Html::parse_document(html);
    let (chapters_published, chapters_total, complete) = chapters(&doc);
    let published_text = first_text(&doc, selector!("dd.published"));
    let updated_text = first_text(&doc, selector!("dd.status"));
    let date_published = parse_date(published_text.as_deref().unwrap_or("Unknown"))?;
    let date_updated = match updated_text {
        Some(text) => parse_date(&text)?,
        None => date_published,
    };

    Ok(Fanfiction {
        id: fic_id,
        title: first_text(&doc, selector!("h2.title.heading")).ok_or_else(|| missing("title"))?,
        authors: authors(&doc)?,
        categories: non_empty(
            all_texts(&doc, selector!("dd.category.tags a.tag"))
                .iter()
                .filter_map(|t| map_category(t))
                .collect(),
        ),
        chapters_total,
        chapters_published,
        characters: non_empty(all_texts(&doc, selector!("dd.character.tags a.tag"))),
        complete,
        fandoms: non_empty(all_texts(&doc, selector!("dd.fandom.tags a.tag")))
            .ok_or_else(|| missing("fandoms"))?,
        hits: count(&doc, selector!("dd.hits")),
        kudos: count(&doc, selector!("dd.kudos")),
        language: first_text(&doc, selector!("dd.language"))
            .unwrap_or_else(|| "English".to_string()),
        rating: first_text(&doc, selector!("dd.rating.tags a.tag"))
            .map_or(Rating::NotRated, |t| map_rating(&t)),
        relationships: non_empty(all_texts(&doc, selector!("dd.relationship.tags a.tag"))),
        restricted: restricted(&doc),
        summary: summary(&doc),
        tags: non_empty(all_texts(&doc, selector!("dd.freeform.tags a.tag"))),
        warnings: non_empty(
            all_texts(&doc, selector!("dd.warning.tags a.tag"))
                .iter()
                .filter_map(|t| map_warning(t))
                .collect(),
        )
        .unwrap_or_else(|| vec![ArchiveWarnings::NoArchiveWarningsApply]),
        words: count(&doc, selector!("dd.words")),
        date_published,
        date_updated,
        last_chapter_read: None,
        reading_status: ReadingStatus::PlanToRead,
        read_count: 0,
        user_rating: None,
        personal_note: None,
        last_checked_date: Utc::now(),
        chapter_ids: chapter_ids(&doc),
    })
}

fn chapter_ids(doc: &Html) -> Vec<u64> {
    doc.select(selector!("select#selected_id option"))
        .filter_map(|option| option.value().attr("value")?.parse().ok())
        .collect()
}

fn text_of(element: ElementRef) -> String {
    element.text().collect::<String>().trim().to_string()
}

fn first_text(doc: &Html, selector: &Selector) -> Option<String> {
    doc.select(selector).next().map(text_of)
}

fn all_texts(doc: &Html, selector: &Selector) -> Vec<String> {
    doc.select(selector).map(text_of).collect()
}

fn non_empty<T>(values: Vec<T>) -> Option<Vec<T>> {
    (!values.is_empty()).then_some(values)
}

fn count(doc: &Html, selector: &Selector) -> u32 {
    first_text(doc, selector)
        .and_then(|t| t.replace(',', "").parse().ok())
        .unwrap_or(0)
}

fn missing(field: &str) -> FicflowError {
    FicflowError::Parse {
        field: field.to_string(),
        reason: "element not found in HTML".to_string(),
    }
}

fn authors(doc: &Html) -> Result<Vec<String>, FicflowError> {
    if let Some(authors) = non_empty(all_texts(
        doc,
        selector!("h3.byline.heading a[rel=\"author\"]"),
    )) {
        return Ok(authors);
    }
    match first_text(doc, selector!("h3.byline.heading")) {
        Some(byline) if byline == "Anonymous" => Ok(vec![byline]),
        _ => Err(missing("authors")),
    }
}

fn summary(doc: &Html) -> String {
    [
        selector!("div.summary.module blockquote.userstuff"),
        selector!(".preface .summary"),
    ]
    .into_iter()
    .filter_map(|sel| first_text(doc, sel))
    .find(|s| !s.is_empty())
    .unwrap_or_else(|| "No summary available".to_string())
}

fn chapters(doc: &Html) -> (u32, Option<u32>, bool) {
    let text = first_text(doc, selector!("dd.chapters")).unwrap_or_else(|| "0/0".to_string());
    let mut parts = text.split('/').map(|s| s.parse::<u32>().unwrap_or(0));
    let published = parts.next().unwrap_or(0);
    let total = parts.next().filter(|&t| t != 0);
    let complete = total.is_some_and(|t| published > 0 && published == t);
    (published, total, complete)
}

fn restricted(doc: &Html) -> bool {
    if doc
        .select(selector!("img[alt=\"(Restricted)\"]"))
        .next()
        .is_some()
    {
        return true;
    }
    doc.select(selector!("p.notice")).any(|element| {
        let text = element.text().collect::<String>().to_lowercase();
        text.contains("only available to registered users")
            || text.contains("restricted to archive users")
    })
}

fn map_rating(text: &str) -> Rating {
    match text {
        "General Audiences" => Rating::General,
        "Teen And Up Audiences" => Rating::TeenAndUp,
        "Mature" => Rating::Mature,
        "Explicit" => Rating::Explicit,
        _ => Rating::NotRated,
    }
}

fn map_category(text: &str) -> Option<Categories> {
    match text {
        "F/F" => Some(Categories::FF),
        "F/M" => Some(Categories::FM),
        "M/M" => Some(Categories::MM),
        "Gen" => Some(Categories::Gen),
        "Multi" => Some(Categories::Multi),
        "Other" => Some(Categories::Other),
        _ => None,
    }
}

fn map_warning(text: &str) -> Option<ArchiveWarnings> {
    match text {
        "Creator Chose Not To Use Archive Warnings" => Some(ArchiveWarnings::ChooseNotToUse),
        "Graphic Depictions Of Violence" => Some(ArchiveWarnings::GraphicDepictionsOfViolence),
        "Major Character Death" => Some(ArchiveWarnings::MajorCharacterDeath),
        "No Archive Warnings Apply" => Some(ArchiveWarnings::NoArchiveWarningsApply),
        "Rape/Non-Con" => Some(ArchiveWarnings::RapeNonCon),
        "Underage" => Some(ArchiveWarnings::Underage),
        _ => None,
    }
}

fn parse_date(text: &str) -> Result<DateTime<Utc>, FicflowError> {
    let date_str = DATE_REGEX
        .captures(text)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str())
        .ok_or_else(|| FicflowError::Parse {
            field: "date".to_string(),
            reason: format!("no YYYY-MM-DD substring in `{}`", text),
        })?;
    let date =
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map_err(|e| FicflowError::Parse {
            field: "date".to_string(),
            reason: format!("`{}`: {}", date_str, e),
        })?;
    Ok(date.and_hms_opt(0, 0, 0).unwrap_or_default().and_utc())
}
