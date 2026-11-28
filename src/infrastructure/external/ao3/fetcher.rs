use std::sync::Arc;
use std::thread;
use std::time::Duration;

use reqwest::StatusCode;

use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher, WorkList};
use crate::domain::series::Series;
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

    fn get(
        &self,
        path: &str,
        base_url: &str,
        needs_login: FicflowError,
    ) -> Result<String, FicflowError> {
        self.with_session_cookie(base_url, needs_login, |cookie| {
            self.client.fetch_page(path, base_url, cookie)
        })
    }

    fn with_session_cookie<T>(
        &self,
        base_url: &str,
        needs_login: FicflowError,
        fetch: impl FnOnce(Option<&str>) -> Result<Option<T>, FicflowError>,
    ) -> Result<T, FicflowError> {
        let access = self
            .session
            .as_ref()
            .filter(|access| access.site == base_url);
        let session = access.and_then(|access| access.store.load());
        let cookie = session.as_ref().map(|session| session.cookie.as_str());
        match fetch(cookie)? {
            Some(body) => Ok(body),
            None if session.is_some() => {
                if let Some(access) = access {
                    access.store.clear()?;
                }
                Err(FicflowError::SessionExpired)
            }
            None => Err(needs_login),
        }
    }

    fn with_retries<T>(
        &self,
        mut attempt: impl FnMut(&str) -> Result<T, FicflowError>,
    ) -> Result<T, FicflowError> {
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
                match attempt(url) {
                    Ok(value) => return Ok(value),
                    Err(e) => {
                        let mirror_lacks_session = matches!(
                            e,
                            FicflowError::RestrictedWork { .. } | FicflowError::LoginRequired
                        ) && self.logged_in();
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
        let path = format!("/works/{fic_id}");
        self.with_retries(|base_url| {
            let html = self.get(&path, base_url, FicflowError::RestrictedWork { fic_id })?;
            parser::parse_work(fic_id, &html)
        })
    }

    fn fetch_work_ids(&self, list_path: &str, limit: usize) -> Result<WorkList, FicflowError> {
        let mut list = WorkList::default();
        let mut next = Some(list_path.to_string());
        while let Some(path) = next {
            let page = self.with_retries(|base_url| {
                let html = self.get(&path, base_url, FicflowError::LoginRequired)?;
                Ok(parser::parse_work_list(&html))
            })?;
            list.total = list.total.or(page.total);
            for id in page.ids {
                if !list.ids.contains(&id) {
                    list.ids.push(id);
                }
            }
            if list.total.unwrap_or(list.ids.len()) > limit || list.ids.len() > limit {
                break;
            }
            next = page.next_path;
        }
        Ok(list)
    }

    fn fetch_series(&self, series_id: u64) -> Result<Series, FicflowError> {
        let mut series: Option<Series> = None;
        let mut next = Some(format!("/series/{series_id}"));
        while let Some(path) = next {
            let page = self.with_retries(|base_url| {
                let html = self.get(&path, base_url, FicflowError::LoginRequired)?;
                parser::parse_series_page(series_id, &html)
            })?;
            next = page.next_path;
            match series.as_mut() {
                Some(series) => series.works.extend(page.series.works),
                None => series = Some(page.series),
            }
        }
        let mut series = series.ok_or(FicflowError::SeriesNotFound { series_id })?;
        series.works.sort_by_key(|work| work.part);
        series.works.dedup_by_key(|work| work.work_id);
        Ok(series)
    }

    fn download_epub(&self, path: &str) -> Result<Vec<u8>, FicflowError> {
        self.with_retries(|base_url| {
            self.with_session_cookie(base_url, FicflowError::LoginRequired, |cookie| {
                self.client.fetch_bytes(path, base_url, cookie)
            })
        })
    }
}
