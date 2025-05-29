use crate::domain::fanfiction::FanfictionFetcher;
use crate::domain::fanfiction::DatabaseOps;
use std::time::Duration;
use std::thread;

pub fn add_fanfiction(
    fetcher: &dyn FanfictionFetcher,
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    base_url: &str,
) -> Result<(), Box<dyn std::error::Error>> {    
    // Try up to 3 times with increasing timeouts
    let max_retries = 3;
    let mut last_error = None;
    
    for attempt in 1..=max_retries {
        match fetcher.fetch_fanfiction(fic_id, base_url) {
            Ok(fic) => {
                // Check if the title indicates an error or unsuccessful fetch
                if fic.title == "Unknown Title (Error Loading)" || 
                   fic.title == "Unknown Title" ||
                   (fic.words == 0 && fic.authors.is_empty() && fic.fandoms.is_empty()) {
                    
                    // If this was the last attempt, return an error
                    if attempt == max_retries {
                        println!("Could not fetch fanfiction data after {} attempts. Not adding to database.", max_retries);
                        return Err("Failed to fetch valid fanfiction data".into());
                    }
                    
                    // Otherwise, retry
                    let wait_time = Duration::from_secs(attempt as u64 * 2);
                    println!("Incomplete fanfiction data received. Retrying in {} seconds (attempt {}/{})", 
                             wait_time.as_secs(), attempt, max_retries);
                    thread::sleep(wait_time);
                    continue;
                }
                
                // Successfully fetched with valid data, now insert into database
                match db_ops.insert_fanfiction(&fic) {
                    Ok(_) => {
                        println!("Successfully added: {}", fic.title);
                        return Ok(());
                    },
                    Err(e) => {
                        // Database error - if it's a unique constraint, the fic already exists
                        if e.to_string().contains("UNIQUE constraint failed") {
                            println!("Fanfiction already exists in your library");
                            return Ok(());
                        }
                        return Err(e);
                    }
                }
            },
            Err(e) => {
                // Specific error handling based on error type
                let error_str = e.to_string();
                if error_str.contains("SSL handshake failed") {
                    println!("SSL handshake failed. This could be due to network issues or AO3 might be down.");
                } else if error_str.contains("operation timed out") {
                    println!("Connection to AO3 timed out. The site might be busy or your internet connection may be slow.");
                } else if error_str.contains("404") || error_str.contains("Not Found") {
                    println!("Fanfiction ID {} was not found on AO3. It may have been deleted or restricted.", fic_id);
                    return Err(e);  // Don't retry for 404 errors
                } else {
                    println!("Error fetching fanfiction: {}", e);
                }
                
                // Save the error for possible rethrow
                last_error = Some(e);
                
                // If we have more attempts, wait and retry
                if attempt < max_retries {
                    let wait_time = Duration::from_secs(attempt as u64 * 2);  // 2, 4, 6 seconds
                    println!("Retrying in {} seconds (attempt {}/{})", wait_time.as_secs(), attempt, max_retries);
                    thread::sleep(wait_time);
                }
            }
        }
    }
    
    // If we get here, all attempts failed
    Err(last_error.unwrap_or_else(|| "Failed to add fanfiction after multiple attempts".into()))
}