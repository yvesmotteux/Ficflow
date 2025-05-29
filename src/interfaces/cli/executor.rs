use crate::{
    application::{
        add_fic::add_fanfiction,
        delete_fic::delete_fic,
        get_fic::get_fanfiction,
        list_fics::list_fics,
        update_chapters::update_last_chapter_read,
        update_rating::update_user_rating,
        update_read_count::update_read_count,
        update_status::update_reading_status,
        wipe_db::wipe_database,
    },
    domain::{
        fanfiction::DatabaseOps,
        fanfiction::FanfictionFetcher,
        url_config
    },
};
use super::command::CliCommand;
use super::views::{details_view, list_view};

pub trait CommandExecutor {
    fn execute_command(&self, command: CliCommand);
}

pub struct CliCommandExecutor<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    database: &'a dyn DatabaseOps,
}

impl<'a> CliCommandExecutor<'a> {
    pub fn new(fetcher: &'a dyn FanfictionFetcher, database: &'a dyn DatabaseOps) -> Self {
        Self { fetcher, database }
    }

    fn execute_add(&self, fic_id: u64) {
        println!("Adding fanfiction with ID: {}", fic_id);
        
        let base_url = url_config::get_ao3_base_url();
        
        if let Err(e) = add_fanfiction(self.fetcher, self.database, fic_id, &base_url) {
            eprintln!("Error adding fanfiction: {}", e);
        }
    }

    fn execute_delete(&self, fic_id: u64) {
        println!("Deleting fanfiction with ID: {}", fic_id);
        if let Err(e) = delete_fic(self.database, fic_id) {
            eprintln!("Error deleting fanfiction: {}", e);
        }
    }

    fn execute_get(&self, fic_id: u64) {
        println!("Getting fanfiction with ID: {}", fic_id);
        match get_fanfiction(self.database, fic_id) {
            Ok(fic) => {
                let details = details_view::render_fanfiction_details(&fic);
                println!("\n{}", details);
            },
            Err(e) => {
                eprintln!("Error getting fanfiction: {}", e);
            }
        }
    }

    fn execute_list(&self) {
        println!("Listing all fanfictions");
        match list_fics(self.database) {
            Ok(fanfictions) => {
                println!("{}", list_view::render_fanfiction_list(&fanfictions));
            },
            Err(e) => {
                eprintln!("Error listing fanfictions: {}", e);
            }
        }
    }

    fn execute_wipe(&self) {
        println!("Preparing to wipe database...");
        
        if let Err(e) = wipe_database(self.database) {
            eprintln!("Error wiping database: {}", e);
        }
    }
    
    fn execute_update_chapter(&self, fic_id: u64, chapter: u32) {
        println!("Updating last read chapter for fanfiction ID: {} to chapter {}", fic_id, chapter);
        match update_last_chapter_read(self.database, fic_id, chapter) {
            Ok(_) => {
                // Display a brief summary of the updated fanfiction
                if let Ok(fic) = get_fanfiction(self.database, fic_id) {
                    println!("Successfully updated \"{}\" (ID: {}) to chapter {}.", 
                             fic.title, fic_id, chapter);
                    println!("Reading Status: {}", fic.reading_status);
                    println!("Read Count: {}", fic.read_count);
                } else {
                    println!("Successfully updated last read chapter.");
                }
            },
            Err(e) => {
                eprintln!("Error updating last read chapter: {}", e);
            }
        }
    }

    fn execute_update_status(&self, fic_id: u64, status: &str) {
        println!("Updating reading status for fanfiction ID: {} to '{}'", fic_id, status);
        match update_reading_status(self.database, fic_id, status) {
            Ok(_) => {
                // Display a brief summary of the updated fanfiction
                if let Ok(fic) = get_fanfiction(self.database, fic_id) {
                    println!("Successfully updated \"{}\" (ID: {}) to status: {}.", 
                             fic.title, fic_id, fic.reading_status);
                } else {
                    println!("Successfully updated reading status.");
                }
            },
            Err(e) => {
                eprintln!("Error updating reading status: {}", e);
            }
        }
    }

    fn execute_update_read_count(&self, fic_id: u64, read_count: u32) {
        println!("Updating read count for fanfiction ID: {} to {}", fic_id, read_count);
        match update_read_count(self.database, fic_id, read_count) {
            Ok(_) => {
                // Display a brief summary of the updated fanfiction
                if let Ok(fic) = get_fanfiction(self.database, fic_id) {
                    println!("Successfully updated \"{}\" (ID: {}) to read count: {}.", 
                             fic.title, fic_id, fic.read_count);
                    println!("Reading Status: {}", fic.reading_status);
                } else {
                    println!("Successfully updated read count.");
                }
            },
            Err(e) => {
                eprintln!("Error updating read count: {}", e);
            }
        }
    }

    fn execute_update_rating(&self, fic_id: u64, rating: &str) {
        println!("Updating user rating for fanfiction ID: {} to '{}'", fic_id, rating);
        match update_user_rating(self.database, fic_id, rating) {
            Ok(_) => {
                // Display a brief summary of the updated fanfiction
                if let Ok(fic) = get_fanfiction(self.database, fic_id) {
                    let rating_display = match &fic.user_rating {
                        Some(rating) => format!("{}", rating),
                        None => "None".to_string(),
                    };
                    println!("Successfully updated \"{}\" (ID: {}) to rating: {}.", 
                             fic.title, fic_id, rating_display);
                } else {
                    println!("Successfully updated user rating.");
                }
            },
            Err(e) => {
                eprintln!("Error updating user rating: {}", e);
            }
        }
    }
}

impl<'a> CommandExecutor for CliCommandExecutor<'a> {
    fn execute_command(&self, command: CliCommand) {
        match command {
            CliCommand::Add { fic_id } => self.execute_add(fic_id),
            CliCommand::Delete { fic_id } => self.execute_delete(fic_id),
            CliCommand::Get { fic_id } => self.execute_get(fic_id),
            CliCommand::UpdateChapter { fic_id, chapter } => self.execute_update_chapter(fic_id, chapter),
            CliCommand::UpdateStatus { fic_id, status } => self.execute_update_status(fic_id, &status),
            CliCommand::UpdateReadCount { fic_id, read_count } => self.execute_update_read_count(fic_id, read_count),
            CliCommand::UpdateRating { fic_id, rating } => self.execute_update_rating(fic_id, &rating),
            CliCommand::List => self.execute_list(),
            CliCommand::Wipe => self.execute_wipe(),
        }
    }
}
