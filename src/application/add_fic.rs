use crate::domain::fanfiction::FanfictionFetcher;
use crate::domain::fanfiction::DatabaseOps;
use std::time::Duration;
use std::thread;

pub enum AddOutcome {
    Added { title: String },
    AlreadyExists,
}

pub fn add_fanfiction(
    fetcher: &dyn FanfictionFetcher,
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    base_url: &str,
) -> Result<AddOutcome, Box<dyn std::error::Error>> {
    let max_retries = 3;
    let mut last_error = None;

    for attempt in 1..=max_retries {
        match fetcher.fetch_fanfiction(fic_id, base_url) {
            Ok(fic) => {
                if fic.title == "Unknown Title (Error Loading)"
                    || fic.title == "Unknown Title"
                    || (fic.words == 0 && fic.authors.is_empty() && fic.fandoms.is_empty())
                {
                    if attempt == max_retries {
                        log::error!(
                            "Could not fetch fanfiction data after {} attempts. Not adding to database.",
                            max_retries
                        );
                        return Err("Failed to fetch valid fanfiction data".into());
                    }

                    let wait_time = Duration::from_secs(attempt as u64 * 2);
                    log::info!(
                        "Incomplete fanfiction data received. Retrying in {} seconds (attempt {}/{})",
                        wait_time.as_secs(), attempt, max_retries
                    );
                    thread::sleep(wait_time);
                    continue;
                }

                match db_ops.save_fanfiction(&fic) {
                    Ok(_) => return Ok(AddOutcome::Added { title: fic.title }),
                    Err(e) => {
                        if e.to_string().contains("UNIQUE constraint failed") {
                            return Ok(AddOutcome::AlreadyExists);
                        }
                        return Err(e);
                    }
                }
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("SSL handshake failed") {
                    log::warn!("SSL handshake failed. This could be due to network issues or AO3 might be down.");
                } else if error_str.contains("operation timed out") {
                    log::warn!("Connection to AO3 timed out. The site might be busy or your internet connection may be slow.");
                } else if error_str.contains("404") || error_str.contains("Not Found") {
                    log::warn!("Fanfiction ID {} was not found on AO3. It may have been deleted or restricted.", fic_id);
                    return Err(e);
                } else {
                    log::warn!("Error fetching fanfiction: {}", e);
                }

                last_error = Some(e);

                if attempt < max_retries {
                    let wait_time = Duration::from_secs(attempt as u64 * 2);
                    log::info!("Retrying in {} seconds (attempt {}/{})", wait_time.as_secs(), attempt, max_retries);
                    thread::sleep(wait_time);
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "Failed to add fanfiction after multiple attempts".into()))
}
