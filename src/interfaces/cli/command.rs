use clap::{Parser, Subcommand};

use crate::application::update_rating::parse_user_rating;
use crate::application::update_status::parse_reading_status;
use crate::domain::fanfiction::{ReadingStatus, UserRating};
use crate::interfaces::utils::url_parser::extract_ao3_id;

#[derive(Parser, Debug)]
#[command(name = "FicFlow")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Clone, Copy, Debug)]
pub struct RatingArg(pub Option<UserRating>);

fn parse_rating_arg(input: &str) -> Result<RatingArg, crate::error::FicflowError> {
    parse_user_rating(input).map(RatingArg)
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Add a fanfiction to the database
    Add {
        /// The ID or URL of the fanfiction (e.g. 12345678, https://archiveofourown.org/works/12345678)
        #[arg(value_name = "fic-id", value_parser = extract_ao3_id)]
        fic_id: u64,
    },
    /// Delete a fanfiction from the database
    Delete {
        /// The ID of the fanfiction
        #[arg(value_name = "fic-id")]
        fic_id: u64,
    },
    /// Get detailed information about a specific fanfiction
    Get {
        /// The ID of the fanfiction
        #[arg(value_name = "fic-id")]
        fic_id: u64,
    },
    /// Update the last chapter read for a fanfiction
    #[command(name = "chapter")]
    UpdateChapter {
        /// The ID of the fanfiction
        #[arg(value_name = "fic-id")]
        fic_id: u64,
        /// The chapter number you've read up to
        #[arg(value_name = "chapter")]
        chapter: u32,
    },
    /// Update the reading status of a fanfiction
    #[command(name = "status")]
    UpdateStatus {
        /// The ID of the fanfiction
        #[arg(value_name = "fic-id")]
        fic_id: u64,
        /// The new reading status (inprogress, read, plantoread, paused, abandoned)
        #[arg(value_name = "status", value_parser = parse_reading_status)]
        status: ReadingStatus,
    },
    /// Update the read count of a fanfiction
    #[command(name = "reads")]
    UpdateReadCount {
        /// The ID of the fanfiction
        #[arg(value_name = "fic-id")]
        fic_id: u64,
        /// The new read count
        #[arg(value_name = "count")]
        read_count: u32,
    },
    /// Update the user rating of a fanfiction
    #[command(name = "rating")]
    UpdateRating {
        /// The ID of the fanfiction
        #[arg(value_name = "fic-id")]
        fic_id: u64,
        /// The new rating (1-5, or 'one' through 'five', or 'none' to remove)
        #[arg(value_name = "rating", value_parser = parse_rating_arg)]
        rating: RatingArg,
    },
    /// Add or remove a personal note for a fanfiction
    #[command(name = "note")]
    UpdateNote {
        /// The ID of the fanfiction
        #[arg(value_name = "fic-id")]
        fic_id: u64,
        /// The personal note text (omit to remove note)
        #[arg(value_name = "note")]
        note: Option<String>,
    },
    /// List all stored fanfictions
    List,
    /// Wipe the database (removes all fanfictions)
    Wipe,
    /// Manage shelves (custom groupings of fanfictions)
    #[command(subcommand)]
    Shelf(ShelfCommand),
}

#[derive(Subcommand, Debug)]
pub enum ShelfCommand {
    /// Create a new shelf
    Create {
        /// Shelf name
        #[arg(value_name = "name")]
        name: String,
        /// Parent shelf ID to nest the new shelf under
        #[arg(short, long, value_name = "parent")]
        parent: Option<u64>,
    },
    /// Delete a shelf
    Delete {
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
    },
    /// Rename an existing shelf
    Rename {
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
        /// New shelf name
        #[arg(value_name = "new-name")]
        new_name: String,
    },
    /// Move a shelf under another shelf (omit parent-id to move to top level)
    Move {
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
        /// New parent shelf ID (omit for top level)
        #[arg(value_name = "parent-id")]
        parent: Option<u64>,
    },
    /// Pin a shelf to the top of its siblings
    Pin {
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
    },
    /// Unpin a shelf
    Unpin {
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
    },
    /// List all shelves
    List,
    /// Add a fanfiction to a shelf
    Add {
        /// Fanfiction ID
        #[arg(value_name = "fic-id")]
        fic_id: u64,
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
    },
    /// Remove a fanfiction from a shelf
    Remove {
        /// Fanfiction ID
        #[arg(value_name = "fic-id")]
        fic_id: u64,
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
    },
    /// List the fanfictions in a shelf
    Show {
        /// Shelf ID
        #[arg(value_name = "shelf-id")]
        shelf_id: u64,
    },
}

pub fn parse_cli_commands() -> CliCommand {
    Cli::parse().command
}
