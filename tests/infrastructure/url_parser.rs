use ficflow::interfaces::utils::url_parser::extract_ao3_id;

#[test]
fn test_extract_numeric_id() {
    assert_eq!(extract_ao3_id("62072974").unwrap(), 62072974);
}

#[test]
fn test_extract_from_full_url() {
    assert_eq!(
        extract_ao3_id("https://archiveofourown.org/works/62072974").unwrap(),
        62072974
    );
}

#[test]
fn test_extract_from_headless_url() {
    assert_eq!(
        extract_ao3_id("archiveofourown.org/works/62072974").unwrap(),
        62072974
    );
}

#[test]
fn test_extract_from_chapter_url() {
    assert_eq!(
        extract_ao3_id("https://archiveofourown.org/works/62072974/chapters/12345").unwrap(),
        62072974
    );
}

#[test]
fn test_extract_from_comment_url() {
    assert_eq!(
        extract_ao3_id("https://archiveofourown.org/works/62072974/comments/915048250").unwrap(),
        62072974
    );
}

#[test]
fn test_invalid_input() {
    assert!(extract_ao3_id("not-a-valid-input").is_err());
    assert!(extract_ao3_id("https://example.com").is_err());
}

mod import_input {
    use ficflow::interfaces::utils::url_parser::{
        Ao3List, ImportInput, parse_import_input, profile_list,
    };

    fn list(path: &str, label: &str) -> ImportInput {
        ImportInput::List(Ao3List {
            path: path.to_string(),
            label: label.to_string(),
        })
    }

    #[test]
    fn single_id_or_work_link_is_a_single_fic() {
        assert_eq!(
            parse_import_input(" 62072974 "),
            ImportInput::Single(62072974)
        );
        assert_eq!(
            parse_import_input("https://archiveofourown.org/works/62072974/chapters/1"),
            ImportInput::Single(62072974)
        );
    }

    #[test]
    fn several_ids_are_deduplicated() {
        assert_eq!(
            parse_import_input("1, 2,3 \n 2 https://archiveofourown.org/works/4"),
            ImportInput::Ids(vec![1, 2, 3, 4])
        );
    }

    #[test]
    fn list_links_map_to_their_listing_page() {
        let cases = [
            (
                "https://archiveofourown.org/collections/test_fest",
                list("/collections/test_fest/works", "Collection test_fest"),
            ),
            (
                "archiveofourown.org/series/123/",
                list("/series/123", "Series 123"),
            ),
            (
                "https://archiveofourown.org/users/writer/works",
                list("/users/writer/works", "Works by writer"),
            ),
            (
                "https://archiveofourown.org/users/writer/pseuds/Pen%20Name/works",
                list("/users/writer/pseuds/Pen%20Name/works", "Works by Pen Name"),
            ),
            (
                "https://archiveofourown.org/users/reader/bookmarks?page=2",
                list("/users/reader/bookmarks?page=2", "Bookmarks of reader"),
            ),
            (
                "https://archiveofourown.org/tags/Fluff%20*a*%20Angst/works",
                list("/tags/Fluff%20*a*%20Angst/works", "Tag Fluff & Angst"),
            ),
            (
                "https://archiveofourown.org/works?work_search%5Bcomplete%5D=T&tag_id=Fluff",
                list(
                    "/works?work_search%5Bcomplete%5D=T&tag_id=Fluff",
                    "Search results",
                ),
            ),
            (
                "https://archiveofourown.org/users/reader/subscriptions",
                list(
                    "/users/reader/subscriptions?type=works",
                    "Your subscriptions",
                ),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(parse_import_input(input), expected, "{input}");
        }
    }

    #[test]
    fn bare_profile_links_need_a_choice() {
        let expected = ImportInput::Profile {
            user: "writer".to_string(),
        };
        assert_eq!(
            parse_import_input("https://archiveofourown.org/users/writer"),
            expected
        );
        assert_eq!(
            parse_import_input("https://archiveofourown.org/users/writer/profile"),
            expected
        );
        assert_eq!(
            profile_list("writer", true),
            Ao3List {
                path: "/users/writer/bookmarks".to_string(),
                label: "Bookmarks of writer".to_string(),
            }
        );
    }

    #[test]
    fn anything_else_is_invalid() {
        for input in [
            "",
            "hello",
            "https://example.com/series/1",
            "https://archiveofourown.org/works",
            "https://archiveofourown.org/series/abc",
            "1, https://archiveofourown.org/series/2",
        ] {
            assert_eq!(parse_import_input(input), ImportInput::Invalid, "{input}");
        }
    }
}
