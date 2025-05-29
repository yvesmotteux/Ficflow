use crate::{
    application::{
        add_fic::add_fanfiction,
        delete_fic::delete_fic,
        get_fic::get_fanfiction,
        list_fics::list_fics,
        update_chapters::update_last_chapter_read,
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
                println!("Successfully updated last read chapter.");
                // Display the updated fanfiction details
                if let Ok(fic) = get_fanfiction(self.database, fic_id) {
                    let details = details_view::render_fanfiction_details(&fic);
                    println!("\n{}", details);
                }
            },
            Err(e) => {
                eprintln!("Error updating last read chapter: {}", e);
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
            CliCommand::List => self.execute_list(),
            CliCommand::Wipe => self.execute_wipe(),
        }
    }
}
