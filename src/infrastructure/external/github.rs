use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::ACCEPT;
use serde::Deserialize;

use crate::error::FicflowError;

pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/yvesmotteux/Ficflow/releases/latest";

#[derive(Debug, Clone, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
}

impl Release {
    pub fn is_newer_than(&self, current_version: &str) -> bool {
        match (
            parse_version(&self.tag_name),
            parse_version(current_version),
        ) {
            (Some(latest), Some(current)) => latest > current,
            _ => false,
        }
    }
}

pub fn fetch_latest_release(url: &str) -> Result<Release, FicflowError> {
    let release = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("ficflow")
        .build()?
        .get(url)
        .header(ACCEPT, "application/vnd.github+json")
        .send()?
        .error_for_status()?
        .json()?;
    Ok(release)
}

fn parse_version(version: &str) -> Option<Vec<u32>> {
    let core = version.trim_start_matches('v').split(['-', '+']).next()?;
    core.split('.').map(|part| part.parse().ok()).collect()
}
