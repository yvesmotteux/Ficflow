use std::env;
use std::io::{self, Write};

use crate::{
    application::{
        add_fic::add_fanfiction,
        delete_fic::delete_fic,
        get_fic::get_fanfiction,
        list_fics::list_fics,
        update_chapters::update_last_chapter_read,
        update_note::update_personal_note,
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
    error::FicflowError,
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

        match add_fanfiction(self.fetcher, self.database, fic_id, &base_url) {
            Ok(title) => println!("Successfully added: {}", title),
            Err(e) => report_error("adding fanfiction", &e),
        }
    }

    fn execute_delete(&self, fic_id: u64) {
        println!("Deleting fanfiction with ID: {}", fic_id);
        if let Err(e) = delete_fic(self.database, fic_id) {
            report_error("deleting fanfiction", &e);
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
                report_error("getting fanfiction", &e);
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
                report_error("listing fanfictions", &e);
            }
        }
    }

    fn execute_wipe(&self) {
        println!("Preparing to wipe database...");

        if !confirm_wipe() {
            println!("Operation cancelled.");
            return;
        }

        match wipe_database(self.database) {
            Ok(()) => println!("Database wiped successfully."),
            Err(e) => report_error("wiping database", &e),
        }
    }
    
    fn execute_update_chapter(&self, fic_id: u64, chapter: u32) {
        println!("Updating last read chapter for fanfiction ID: {} to chapter {}", fic_id, chapter);
        match update_last_chapter_read(self.database, fic_id, chapter) {
            Ok(fic) => {
                println!("Successfully updated \"{}\" (ID: {}) to chapter {}.", fic.title, fic_id, chapter);
                println!("Reading Status: {}", fic.reading_status);
                println!("Read Count: {}", fic.read_count);
            }
            Err(e) => report_error("updating last read chapter", &e),
        }
    }

    fn execute_update_status(&self, fic_id: u64, status: &str) {
        println!("Updating reading status for fanfiction ID: {} to '{}'", fic_id, status);
        match update_reading_status(self.database, fic_id, status) {
            Ok(fic) => {
                println!("Successfully updated \"{}\" (ID: {}) to status: {}.", fic.title, fic_id, fic.reading_status);
            }
            Err(e) => report_error("updating reading status", &e),
        }
    }

    fn execute_update_read_count(&self, fic_id: u64, read_count: u32) {
        println!("Updating read count for fanfiction ID: {} to {}", fic_id, read_count);
        match update_read_count(self.database, fic_id, read_count) {
            Ok(fic) => {
                println!("Successfully updated \"{}\" (ID: {}) to read count: {}.", fic.title, fic_id, fic.read_count);
                println!("Reading Status: {}", fic.reading_status);
            }
            Err(e) => report_error("updating read count", &e),
        }
    }

    fn execute_update_rating(&self, fic_id: u64, rating: &str) {
        println!("Updating user rating for fanfiction ID: {} to '{}'", fic_id, rating);
        match update_user_rating(self.database, fic_id, rating) {
            Ok(fic) => {
                let rating_display = match &fic.user_rating {
                    Some(r) => format!("{}", r),
                    None => "None".to_string(),
                };
                println!("Successfully updated \"{}\" (ID: {}) to rating: {}.", fic.title, fic_id, rating_display);
            }
            Err(e) => report_error("updating user rating", &e),
        }
    }

    fn execute_update_note(&self, fic_id: u64, note: Option<&str>) {
        // If removing a note, show the current one first so the user sees what's being dropped.
        if note.is_none() {
            if let Ok(fic) = get_fanfiction(self.database, fic_id) {
                if let Some(current_note) = &fic.personal_note {
                    println!("Current personal note for \"{}\" (ID: {}): {}", fic.title, fic_id, current_note);
                    println!("Removing personal note...");
                }
            }
        }

        match update_personal_note(self.database, fic_id, note) {
            Ok(fic) => match &fic.personal_note {
                Some(note_text) => {
                    println!("Successfully added note to \"{}\" (ID: {}).", fic.title, fic_id);
                    println!("Note: {}", note_text);
                }
                None => {
                    println!("Successfully removed note from \"{}\" (ID: {}).", fic.title, fic_id);
                }
            },
            Err(e) => report_error("updating personal note", &e),
        }
    }
}

fn report_error(verb: &str, err: &FicflowError) {
    match err {
        FicflowError::NotFound { fic_id } => {
            eprintln!("Fanfiction ID {} not found in your library", fic_id);
        }
        FicflowError::InvalidInput(msg) => {
            eprintln!("{}", msg);
        }
        other => {
            eprintln!("Error {}: {}", verb, other);
        }
    }
}

fn confirm_wipe() -> bool {
    if env::var("FICFLOW_NON_INTERACTIVE").is_ok() {
        return true;
    }

    print!(
        "WARNING: This action will delete ALL fanfictions from the database. \
         This process CANNOT be reversed!\nAre you sure you want to continue? (y/N): "
    );
    if io::stdout().flush().is_err() {
        return false;
    }

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    input.trim().eq_ignore_ascii_case("y")
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
            CliCommand::UpdateNote { fic_id, note } => self.execute_update_note(fic_id, note.as_deref()),
            CliCommand::List => self.execute_list(),
            CliCommand::Wipe => self.execute_wipe(),
        }
    }
}
