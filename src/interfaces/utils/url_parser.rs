use std::sync::LazyLock;

use regex::Regex;

static WORK_URL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:archiveofourown\.org/|//)works/(\d+)(?:/|$)").expect("valid work URL regex")
});

/// Extracts a fanfiction ID from various input formats:
/// - Plain numeric ID: "12345678"
/// - Full URL: "https://archiveofourown.org/works/12345678"
/// - Headless URL: "archiveofourown.org/works/12345678"
/// - Chapter URL: "https://archiveofourown.org/works/12345678/chapters/123456"
/// - Comment URL: "https://archiveofourown.org/works/12345678/comments/123456"
///
/// Returns the extracted ID as u64 or an error if the input doesn't contain a valid ID.
pub fn extract_ao3_id(input: &str) -> Result<u64, String> {
    // Try direct numeric parsing first
    if let Ok(id) = input.parse::<u64>() {
        return Ok(id);
    }

    if let Some(captures) = WORK_URL.captures(input)
        && let Some(id_match) = captures.get(1)
    {
        let id_str = id_match.as_str();
        return id_str
            .parse::<u64>()
            .map_err(|_| format!("Failed to parse extracted ID '{}' as a number", id_str));
    }

    Err(format!("Could not extract AO3 ID from '{}'", input))
}
