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

use crate::domain::fanfiction::ReadingStatus;

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
