use crate::error::FicflowError;
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT},
};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Minimum gap between successive AO3 requests.
const DEFAULT_MIN_REQUEST_GAP: Duration = Duration::from_secs(4);

pub struct Ao3Client {
    client: Client,
    min_gap: Duration,
    last_request: Mutex<Option<Instant>>,
}

impl Ao3Client {
    pub fn new() -> Result<Self, FicflowError> {
        Self::with_min_gap(DEFAULT_MIN_REQUEST_GAP)
    }

    /// Override the minimum inter-request gap. Pass `Duration::ZERO` in tests
    /// so the mock-backed suite doesn't wait between calls.
    pub fn with_min_gap(min_gap: Duration) -> Result<Self, FicflowError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0",
            ),
        );
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.5"));

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .default_headers(headers)
            .http1_only()
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

    pub fn fetch_work(&self, fic_id: u64, base_url: &str) -> Result<String, FicflowError> {
        self.throttle();
        let url = format!("{}/works/{}", base_url, fic_id);
        let response = self.client.get(&url).send()?.error_for_status()?.text()?;
        Ok(response)
    }
}
