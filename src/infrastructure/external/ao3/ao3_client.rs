use reqwest::{blocking::Client, header::{HeaderMap, USER_AGENT}};
use std::error::Error;

pub struct Ao3Client {
    client: Client,
}

impl Ao3Client {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))  // Set a 60-second timeout
            .build()?;
        
        Ok(Self { client })
    }
    
    pub fn fetch_work(&self, fic_id: u64, base_url: &str) -> Result<String, Box<dyn Error>> {
        let url = format!("{}/works/{}", base_url, fic_id);
        
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36".parse().unwrap());
    
        let response = self.client.get(&url).headers(headers).send()?.text()?;
        
        Ok(response)
    }
}
