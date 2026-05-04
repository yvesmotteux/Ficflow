//! Shared display helpers for the GUI layer.
//!
//! These map domain types to user-facing strings. They live here
//! (rather than as `Display` impls on the domain enums) on purpose:
//! the wording is a GUI choice, not a property of the domain. The
//! CLI for example may want different (or terser) labels.
//!
//! The convention: when two views need the *same* string for the same
//! input, the helper goes here. When they need *different* strings
//! (e.g. table-narrow "General" vs panel-long "General Audiences"),
//! each view keeps its own local helper.

use crate::domain::fanfiction::{ReadingStatus, UserRating};

/// Reading-status label used everywhere the user sees a status —
/// table cell, sidebar count row, status combo, status-change menu.
pub fn format_status(status: &ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "In Progress",
        ReadingStatus::Read => "Read",
        ReadingStatus::PlanToRead => "Plan to Read",
        ReadingStatus::Paused => "Paused",
        ReadingStatus::Abandoned => "Abandoned",
    }
}

/// Decimal number with thousands separators: 12345 → "12,345".
/// Used by the Words / Kudos / Hits cells in the library table and
/// by the same fields in the AO3 metadata panel.
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

/// Canonical CLI/application-layer payload string for a status. The
/// application functions accept a string for parity with the CLI and
/// JSON config; this helper keeps the conversion in one place.
pub fn status_payload(status: ReadingStatus) -> &'static str {
    match status {
        ReadingStatus::InProgress => "inprogress",
        ReadingStatus::Read => "read",
        ReadingStatus::PlanToRead => "plantoread",
        ReadingStatus::Paused => "paused",
        ReadingStatus::Abandoned => "abandoned",
    }
}

/// Canonical CLI/application-layer payload string for a user rating.
/// `None` is the "no rating" sentinel.
pub fn rating_payload(rating: Option<UserRating>) -> &'static str {
    match rating {
        Some(UserRating::One) => "1",
        Some(UserRating::Two) => "2",
        Some(UserRating::Three) => "3",
        Some(UserRating::Four) => "4",
        Some(UserRating::Five) => "5",
        None => "none",
    }
}
