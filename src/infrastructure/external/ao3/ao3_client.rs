use crate::error::FicflowError;
use reqwest::{
    blocking::{Client, Response},
    header::{
        ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, COOKIE, HeaderMap, HeaderName,
        HeaderValue, UPGRADE_INSECURE_REQUESTS, USER_AGENT,
    },
};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Minimum gap between successive AO3 requests.
pub const DEFAULT_MIN_REQUEST_GAP: Duration = Duration::from_secs(4);

#[cfg(target_os = "windows")]
const FIREFOX_PLATFORM: &str = "Windows NT 10.0; Win64; x64";
#[cfg(target_os = "macos")]
const FIREFOX_PLATFORM: &str = "Macintosh; Intel Mac OS X 10.15";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const FIREFOX_PLATFORM: &str = "X11; Linux x86_64";

const FIREFOX_VERSION: &str = "153.0";

pub fn browser_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let user_agent = format!(
        "Mozilla/5.0 ({FIREFOX_PLATFORM}; rv:{FIREFOX_VERSION}) Gecko/20100101 Firefox/{FIREFOX_VERSION}"
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&user_agent).expect("valid user agent"),
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));
    headers.insert(
        ACCEPT_ENCODING,
        HeaderValue::from_static("gzip, deflate, br, zstd"),
    );
    headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
    headers.insert(UPGRADE_INSECURE_REQUESTS, HeaderValue::from_static("1"));
    for (name, value) in [
        ("sec-fetch-dest", "document"),
        ("sec-fetch-mode", "navigate"),
        ("sec-fetch-site", "none"),
        ("sec-fetch-user", "?1"),
        ("priority", "u=0, i"),
    ] {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    headers
}

pub fn read_page(response: Response) -> Result<String, FicflowError> {
    let challenged = response
        .headers()
        .get("cf-mitigated")
        .is_some_and(|value| value == "challenge");
    let failure = response.error_for_status_ref().err();
    let body = response.text()?;
    if challenged || (failure.is_some() && body.contains("/cdn-cgi/challenge-platform")) {
        return Err(FicflowError::CloudflareChallenge);
    }
    match failure {
        Some(err) => Err(err.into()),
        None => Ok(body),
    }
}

pub struct Ao3Client {
    client: Client,
    min_gap: Duration,
    last_request: Mutex<Option<Instant>>,
}

impl Ao3Client {
    /// Override the minimum inter-request gap. Pass `Duration::ZERO` in tests
    /// so the mock-backed suite doesn't wait between calls.
    pub fn with_min_gap(min_gap: Duration) -> Result<Self, FicflowError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .default_headers(browser_headers())
            .http1_only()
            .http1_title_case_headers()
            .build()?;

        Ok(Self {
            client,
            min_gap,
            last_request: Mutex::new(None),
        })
    }

    fn throttle(&self) {
        if self.min_gap.is_zero() {
            return;
        }
        let mut slot = self.last_request.lock().unwrap();
        if let Some(prev) = *slot {
            let elapsed = prev.elapsed();
            if elapsed < self.min_gap {
                std::thread::sleep(self.min_gap - elapsed);
            }
        }
        *slot = Some(Instant::now());
    }

    pub fn fetch_work(
        &self,
        fic_id: u64,
        base_url: &str,
        cookie: Option<&str>,
    ) -> Result<String, FicflowError> {
        self.throttle();
        let url = format!("{}/works/{}", base_url, fic_id);
        let mut request = self.client.get(&url);
        if let Some(cookie) = cookie {
            request = request.header(COOKIE, cookie);
        }
        let response = request.send()?;
        if response.url().path().starts_with("/users/login") {
            return Err(FicflowError::RestrictedWork { fic_id });
        }
        read_page(response)
    }
}
