use crate::error::FicflowError;
use reqwest::{
    blocking::Client,
    header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT},
};

pub struct Ao3Client {
    client: Client,
}

impl Ao3Client {
    pub fn new() -> Result<Self, FicflowError> {
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
            .timeout(std::time::Duration::from_secs(60))
            .default_headers(headers)
            .build()?;

        Ok(Self { client })
    }

    pub fn fetch_work(&self, fic_id: u64, base_url: &str) -> Result<String, FicflowError> {
        let url = format!("{}/works/{}", base_url, fic_id);
        let response = self.client.get(&url).send()?.error_for_status()?.text()?;
        Ok(response)
    }
}
