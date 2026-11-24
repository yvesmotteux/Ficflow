//! Display helpers shared by GUI views — wording is a UI choice
//! (the CLI may render the same domain enums with terser labels).

use chrono::{DateTime, Datelike, NaiveDate, Utc};

use crate::domain::fanfiction::ReadingStatus;

pub fn format_status(status: &ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "In Progress",
        ReadingStatus::Read => "Read",
        ReadingStatus::PlanToRead => "Plan to Read",
        ReadingStatus::Paused => "Paused",
        ReadingStatus::Abandoned => "Abandoned",
    }
}

const ERISIAN_SEASONS: [&str; 5] = [
    "Chaos",
    "Discord",
    "Confusion",
    "Bureaucracy",
    "The Aftermath",
];
const ERISIAN_WEEKDAYS: [&str; 5] = [
    "Sweetmorn",
    "Boomtime",
    "Pungenday",
    "Prickle-Prickle",
    "Setting Orange",
];

pub fn erisian_date(date: NaiveDate) -> String {
    let yold = date.year() + 1166;
    let mut ordinal = date.ordinal() as usize;

    if date.leap_year() {
        if ordinal == 60 {
            return format!("St. Tib's Day, YOLD {yold}");
        }
        if ordinal > 60 {
            ordinal -= 1; // skip Feb 29: the Erisian year is always 365 days
        }
    }

    let weekday = ERISIAN_WEEKDAYS[(ordinal - 1) % 5];
    let season = ERISIAN_SEASONS[(ordinal - 1) / 73];
    let day = (ordinal - 1) % 73 + 1;
    format!("{weekday}, {season} {day}, YOLD {yold}")
}

pub fn relative_time(when: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - when).num_seconds().max(0);
    let (value, unit) = match secs {
        0..5 => return "just now".into(),
        5..60 => (secs, "s"),
        60..3_600 => (secs / 60, "m"),
        3_600..86_400 => (secs / 3_600, "h"),
        86_400..2_592_000 => (secs / 86_400, "d"),
        2_592_000..31_104_000 => (secs / 2_592_000, "mo"),
        _ => (secs / 31_104_000, "y"),
    };
    format!("{}{} ago", value, unit)
}
