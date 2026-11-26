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

const AO3_HOSTS: [&str; 5] = [
    "archiveofourown.org",
    "www.archiveofourown.org",
    "archiveofourown.gay",
    "xn--iao3-lw4b.ws",
    "ao3.org",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ao3List {
    pub path: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportInput {
    Single(u64),
    Ids(Vec<u64>),
    List(Ao3List),
    Profile { user: String },
    Invalid,
}

pub fn parse_import_input(input: &str) -> ImportInput {
    let tokens: Vec<&str> = input
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|token| !token.is_empty())
        .collect();
    let ids: Option<Vec<u64>> = tokens
        .iter()
        .map(|token| extract_ao3_id(token).ok())
        .collect();
    match (tokens.as_slice(), ids) {
        ([], _) => ImportInput::Invalid,
        (_, Some(ids)) => {
            let mut unique = Vec::new();
            for id in ids {
                if !unique.contains(&id) {
                    unique.push(id);
                }
            }
            match unique.as_slice() {
                [id] => ImportInput::Single(*id),
                _ => ImportInput::Ids(unique),
            }
        }
        ([url], None) => parse_list_url(url),
        _ => ImportInput::Invalid,
    }
}

pub fn profile_list(user: &str, bookmarks: bool) -> Ao3List {
    if bookmarks {
        list(
            format!("/users/{user}/bookmarks"),
            format!("Bookmarks of {user}"),
        )
    } else {
        list(format!("/users/{user}/works"), format!("Works by {user}"))
    }
}

pub fn subscriptions_list(user: &str) -> Ao3List {
    list(
        format!("/users/{user}/subscriptions?type=works"),
        "Your subscriptions".to_string(),
    )
}

fn list(path: String, label: String) -> Ao3List {
    Ao3List { path, label }
}

fn parse_list_url(url: &str) -> ImportInput {
    let url = url.split('#').next().unwrap_or_default();
    let without_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let Some((host, rest)) = without_scheme.split_once('/') else {
        return ImportInput::Invalid;
    };
    if !AO3_HOSTS.contains(&host.to_ascii_lowercase().as_str()) {
        return ImportInput::Invalid;
    }
    let (path, query) = match rest.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (rest, None),
    };
    let with_query = |path: String| match query {
        Some(query) if !query.is_empty() => format!("{path}?{query}"),
        _ => path,
    };
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let found = match segments.as_slice() {
        ["collections", name] | ["collections", name, "works"] => list(
            with_query(format!("/collections/{name}/works")),
            format!("Collection {}", decode(name)),
        ),
        ["series", id] if id.parse::<u64>().is_ok() => {
            list(format!("/series/{id}"), format!("Series {id}"))
        }
        ["users", user] | ["users", user, "profile"] => {
            return ImportInput::Profile {
                user: user.to_string(),
            };
        }
        ["users", user, "works"] => list(
            with_query(format!("/users/{user}/works")),
            format!("Works by {user}"),
        ),
        ["users", user, "pseuds", pseud] | ["users", user, "pseuds", pseud, "works"] => list(
            with_query(format!("/users/{user}/pseuds/{pseud}/works")),
            format!("Works by {}", decode(pseud)),
        ),
        ["users", user, "bookmarks"] => list(
            with_query(format!("/users/{user}/bookmarks")),
            format!("Bookmarks of {user}"),
        ),
        ["users", user, "pseuds", pseud, "bookmarks"] => list(
            with_query(format!("/users/{user}/pseuds/{pseud}/bookmarks")),
            format!("Bookmarks of {}", decode(pseud)),
        ),
        ["users", user, "gifts"] => list(
            with_query(format!("/users/{user}/gifts")),
            format!("Gifts for {user}"),
        ),
        ["users", user, "subscriptions"] => subscriptions_list(user),
        ["tags", tag] | ["tags", tag, "works"] => list(
            with_query(format!("/tags/{tag}/works")),
            format!("Tag {}", decode(tag)),
        ),
        ["works"] | ["works", "search"] if query.is_some() => list(
            with_query(format!("/{}", segments.join("/"))),
            "Search results".to_string(),
        ),
        _ => return ImportInput::Invalid,
    };
    ImportInput::List(found)
}

fn decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (b'+', _) => {
                out.push(b' ');
                i += 1;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out)
        .replace("*s*", "/")
        .replace("*a*", "&")
        .replace("*d*", ".")
        .replace("*q*", "?")
        .replace("*h*", "#")
}
