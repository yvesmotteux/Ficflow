use std::thread;
use std::time::Duration;

use reqwest::StatusCode;

use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher};
use crate::error::FicflowError;

pub struct RetryingFetcher<F: FanfictionFetcher> {
    inner: F,
    max_retries: u32,
    backoff_base: Duration,
}

impl<F: FanfictionFetcher> RetryingFetcher<F> {
    pub fn new(inner: F, max_retries: u32) -> Self {
        Self { inner, max_retries, backoff_base: Duration::from_secs(2) }
    }

    /// Constructor for tests — lets callers shrink the backoff so retries don't dominate test time.
    pub fn with_backoff(inner: F, max_retries: u32, backoff_base: Duration) -> Self {
        Self { inner, max_retries, backoff_base }
    }
}

fn is_retryable(err: &FicflowError) -> bool {
    match err {
        // HTTP 404 is deterministic — the fic doesn't exist on AO3. No point retrying.
        FicflowError::Network(e) => e.status() != Some(StatusCode::NOT_FOUND),
        // Parse failures may be transient when AO3 serves a degraded page under load.
        FicflowError::Parse { .. } => true,
        _ => false,
    }
}

impl<F: FanfictionFetcher> FanfictionFetcher for RetryingFetcher<F> {
    fn fetch_fanfiction(&self, fic_id: u64, base_url: &str) -> Result<Fanfiction, FicflowError> {
        let mut last_err: Option<FicflowError> = None;

        for attempt in 1..=self.max_retries {
            match self.inner.fetch_fanfiction(fic_id, base_url) {
                Ok(fic) => return Ok(fic),
                Err(e) => {
                    if !is_retryable(&e) || attempt == self.max_retries {
                        return Err(e);
                    }
                    let wait = self.backoff_base * attempt;
                    log::info!(
                        "Fetch attempt {}/{} failed ({}). Retrying in {} seconds...",
                        attempt, self.max_retries, e, wait.as_secs()
                    );
                    last_err = Some(e);
                    thread::sleep(wait);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            FicflowError::Other("Fetch failed after retries".into())
        }))
    }
}

