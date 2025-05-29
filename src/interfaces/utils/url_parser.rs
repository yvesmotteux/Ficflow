use regex::Regex;

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
    
    // Regular expression to match AO3 work IDs in URLs
    // This matches patterns like:
    // - archiveofourown.org/works/12345678
    // - archiveofourown.org/works/12345678/chapters/123456
    // - archiveofourown.org/works/12345678/comments/123456
    let re = Regex::new(r"(?:archiveofourown\.org/|//)works/(\d+)(?:/|$)").unwrap();
    
    if let Some(captures) = re.captures(input) {
        if let Some(id_match) = captures.get(1) {
            let id_str = id_match.as_str();
            return id_str.parse::<u64>()
                .map_err(|_| format!("Failed to parse extracted ID '{}' as a number", id_str));
        }
    }
    
    Err(format!("Could not extract AO3 ID from '{}'", input))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extract_numeric_id() {
        assert_eq!(extract_ao3_id("62072974").unwrap(), 62072974);
    }
    
    #[test]
    fn test_extract_from_full_url() {
        assert_eq!(extract_ao3_id("https://archiveofourown.org/works/62072974").unwrap(), 62072974);
    }
    
    #[test]
    fn test_extract_from_headless_url() {
        assert_eq!(extract_ao3_id("archiveofourown.org/works/62072974").unwrap(), 62072974);
    }
    
    #[test]
    fn test_extract_from_chapter_url() {
        assert_eq!(extract_ao3_id("https://archiveofourown.org/works/62072974/chapters/12345").unwrap(), 62072974);
    }
    
    #[test]
    fn test_extract_from_comment_url() {
        assert_eq!(extract_ao3_id("https://archiveofourown.org/works/62072974/comments/915048250").unwrap(), 62072974);
    }
    
    #[test]
    fn test_invalid_input() {
        assert!(extract_ao3_id("not-a-valid-input").is_err());
        assert!(extract_ao3_id("https://example.com").is_err());
    }
}
