//! Display helpers shared by GUI views — wording is a UI choice
//! (the CLI may render the same domain enums with terser labels).

use chrono::{Datelike, NaiveDate};

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

/// `12345 → "12,345"`.
pub fn format_thousands(n: u32) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}
