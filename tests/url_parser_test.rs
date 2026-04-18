use ficflow::interfaces::utils::url_parser::extract_ao3_id;

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
