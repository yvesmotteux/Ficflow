use std::sync::Arc;
use std::thread;
use std::time::Duration;

use reqwest::StatusCode;

use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher};
use crate::error::FicflowError;
use crate::infrastructure::external::ao3::ao3_client::{Ao3Client, DEFAULT_MIN_REQUEST_GAP};
use crate::infrastructure::external::ao3::parser;
use crate::infrastructure::external::ao3::session::SessionStore;

pub const PRIMARY_AO3_URL: &str = "https://archiveofourown.org";
pub const ALT_AO3_URL: &str = "https://archiveofourown.gay";
pub const PROXY_AO3_URL: &str = "https://xn--iao3-lw4b.ws";

/// AO3 URL list + retry-cycle count derived from the environment.
///
/// `AO3_BASE_URL` pins to a single URL (used by integration tests):
/// only that URL is tried, with extra cycles as a small concession to
/// flaky test mocks. Otherwise the production round-robin is used:
/// primary → alt → proxy, two cycles.
///
/// Single source of truth for both `main.rs` (CLI) and
/// `FicflowConfig::default()` (GUI).
pub fn ao3_urls_from_env() -> (Vec<String>, u32) {
    match std::env::var("AO3_BASE_URL") {
        Ok(url) => (vec![url], 3),
        Err(_) => (
            vec![
                PRIMARY_AO3_URL.to_string(),
                ALT_AO3_URL.to_string(),
                PROXY_AO3_URL.to_string(),
            ],
            2,
        ),
    }
}

pub fn session_site_from_env() -> String {
    std::env::var("AO3_BASE_URL").unwrap_or_else(|_| PRIMARY_AO3_URL.to_string())
}

struct SessionAccess {
    store: Arc<dyn SessionStore>,
    site: String,
}

pub struct Ao3Fetcher {
    client: Ao3Client,
    urls: Vec<String>,
    max_cycles: u32,
    backoff_base: Duration,
    session: Option<SessionAccess>,
}

impl Ao3Fetcher {
    pub fn new(urls: Vec<String>, max_cycles: u32) -> Result<Self, FicflowError> {
        Self::with_min_gap(
            urls,
            max_cycles,
            DEFAULT_MIN_REQUEST_GAP,
            Duration::from_secs(2),
        )
    }

    /// Test constructor: collapses the AO3 throttle and inter-cycle backoff so retries
    /// don't dominate test runtime.
    pub fn with_min_gap(
        urls: Vec<String>,
        max_cycles: u32,
        min_gap: Duration,
        backoff_base: Duration,
    ) -> Result<Self, FicflowError> {
        assert!(!urls.is_empty(), "Ao3Fetcher requires at least one URL");
        Ok(Self {
            client: Ao3Client::with_min_gap(min_gap)?,
            urls,
            max_cycles,
            backoff_base,
            session: None,
        })
    }

    pub fn with_session(mut self, store: Arc<dyn SessionStore>, site: String) -> Self {
        self.session = Some(SessionAccess { store, site });
        self
    }

    fn logged_in(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|access| access.store.load().is_some())
    }

    fn scrape(&self, fic_id: u64, base_url: &str) -> Result<Fanfiction, FicflowError> {
        let access = self
            .session
            .as_ref()
            .filter(|access| access.site == base_url);
        let session = access.and_then(|access| access.store.load());
        let cookie = session.as_ref().map(|session| session.cookie.as_str());
        match self.client.fetch_work(fic_id, base_url, cookie) {
            Ok(response) => parser::parse_work(fic_id, &response),
            Err(FicflowError::RestrictedWork { .. }) if session.is_some() => {
                if let Some(access) = access {
                    access.store.clear()?;
                }
                Err(FicflowError::SessionExpired)
            }
            Err(err) => Err(err),
        }
    }
}

fn retryable(err: &FicflowError) -> bool {
    match err {
        // HTTP 404 is authoritative — the fic doesn't exist. Stop regardless of source URL.
        FicflowError::Network(e) => e.status() != Some(StatusCode::NOT_FOUND),
        // Parse failures may be transient when AO3 serves a degraded page under load.
        FicflowError::Parse { .. } => true,
        FicflowError::CloudflareChallenge => true,
        _ => false,
    }
}

impl FanfictionFetcher for Ao3Fetcher {
    fn fetch_fanfiction(&self, fic_id: u64) -> Result<Fanfiction, FicflowError> {
        let mut last_err: Option<FicflowError> = None;
        let mut challenged = false;

        for cycle in 1..=self.max_cycles {
            if cycle > 1 {
                let wait = self.backoff_base * (cycle - 1);
                log::info!(
                    "All URLs failed in cycle {}/{}. Waiting {}s before retrying.",
                    cycle - 1,
                    self.max_cycles,
                    wait.as_secs()
                );
                thread::sleep(wait);
            }

            for url in &self.urls {
                match self.scrape(fic_id, url) {
                    Ok(fic) => return Ok(fic),
                    Err(e) => {
                        let mirror_lacks_session =
                            matches!(e, FicflowError::RestrictedWork { .. }) && self.logged_in();
                        if !retryable(&e) && !mirror_lacks_session {
                            return Err(e);
                        }
                        log::info!(
                            "Fetch failed at {} (cycle {}/{}): {}",
                            url,
                            cycle,
                            self.max_cycles,
                            e
                        );
                        challenged |= matches!(e, FicflowError::CloudflareChallenge);
                        last_err = Some(e);
                    }
                }
            }
        }

        if challenged {
            return Err(FicflowError::CloudflareChallenge);
        }
        Err(last_err.unwrap_or_else(|| FicflowError::Other("Fetch failed after retries".into())))
    }
}
