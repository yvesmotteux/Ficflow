use clap::{Command, Arg};
use crate::interfaces::utils::url_parser;

#[derive(Debug)]
pub enum CliCommand {
    Add { fic_id: u64 },
    Delete { fic_id: u64 },
    Get { fic_id: u64 },
    List,
    Wipe,
    UpdateChapter { fic_id: u64, chapter: u32 },
    UpdateStatus { fic_id: u64, status: String },
    UpdateReadCount { fic_id: u64, read_count: u32 },
    UpdateRating { fic_id: u64, rating: String },
    UpdateNote { fic_id: u64, note: Option<String> },
}

pub fn parse_cli_commands() -> CliCommand {
    let matches = Command::new("FicFlow")
        .subcommand(
            Command::new("add")
                .about("Add a fanfiction to the database")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID or URL of the fanfiction (e.g. 12345678, https://archiveofourown.org/works/12345678)")),
        )
        .subcommand(
            Command::new("delete")
                .about("Delete a fanfiction from the database")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction")),
        )
        .subcommand(
            Command::new("get")
                .about("Get detailed information about a specific fanfiction")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction")),
        )
        .subcommand(
            Command::new("chapter")
                .about("Update the last chapter read for a fanfiction")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction"))
                .arg(Arg::new("chapter").required(true).index(2).help("The chapter number you've read up to")),
        )
        .subcommand(
            Command::new("status")
                .about("Update the reading status of a fanfiction")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction"))
                .arg(Arg::new("status").required(true).index(2).help("The new reading status (inprogress, read, plantoread, paused, abandoned)")),
        )
        .subcommand(
            Command::new("reads")
                .about("Update the read count of a fanfiction")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction"))
                .arg(Arg::new("count").required(true).index(2).help("The new read count")),
        )
        .subcommand(
            Command::new("rating")
                .about("Update the user rating of a fanfiction")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction"))
                .arg(Arg::new("rating").required(true).index(2).help("The new rating (1-5, or 'one' through 'five', or 'none' to remove)")),
        )
        .subcommand(
            Command::new("note")
                .about("Add or remove a personal note for a fanfiction")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction"))
                .arg(Arg::new("note").required(false).index(2).help("The personal note text (omit to remove note)")),
        )
        .subcommand(Command::new("list").about("List all stored fanfictions"))
        .subcommand(Command::new("wipe").about("Wipe the database (removes all fanfictions)"))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("add") {
        let fic_id_input = matches.get_one::<String>("fic-id").expect("fic-id or url is required");
        
        // Extract AO3 ID from input (could be a direct ID or a URL in various formats)
        match url_parser::extract_ao3_id(fic_id_input) {
            Ok(id) => CliCommand::Add { fic_id: id },
            Err(e) => {
                eprintln!("Error: {}. Please provide a valid AO3 ID or URL.", e);
                std::process::exit(1);
            }
        }
    } else if let Some(matches) = matches.subcommand_matches("delete") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        CliCommand::Delete { fic_id: fic_id.parse::<u64>().unwrap() }
    } else if let Some(matches) = matches.subcommand_matches("get") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        CliCommand::Get { fic_id: fic_id.parse::<u64>().unwrap() }
    } else if let Some(matches) = matches.subcommand_matches("chapter") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        let chapter = matches.get_one::<String>("chapter").expect("chapter number is required");
        CliCommand::UpdateChapter { 
            fic_id: fic_id.parse::<u64>().unwrap(),
            chapter: chapter.parse::<u32>().unwrap()
        }
    } else if let Some(matches) = matches.subcommand_matches("status") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        let status = matches.get_one::<String>("status").expect("status is required");
        CliCommand::UpdateStatus { 
            fic_id: fic_id.parse::<u64>().unwrap(),
            status: status.to_string()
        }
    } else if let Some(matches) = matches.subcommand_matches("reads") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        let count = matches.get_one::<String>("count").expect("read count is required");
        CliCommand::UpdateReadCount { 
            fic_id: fic_id.parse::<u64>().unwrap(),
            read_count: count.parse::<u32>().unwrap()
        }
    } else if let Some(matches) = matches.subcommand_matches("rating") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        let rating = matches.get_one::<String>("rating").expect("rating is required");
        CliCommand::UpdateRating { 
            fic_id: fic_id.parse::<u64>().unwrap(),
            rating: rating.to_string()
        }
    } else if let Some(matches) = matches.subcommand_matches("note") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        let note = matches.get_one::<String>("note").map(|s| s.to_string());
        CliCommand::UpdateNote { 
            fic_id: fic_id.parse::<u64>().unwrap(),
            note
        }
    } else if matches.subcommand_matches("list").is_some() {
        CliCommand::List
    } else if matches.subcommand_matches("wipe").is_some() {
        CliCommand::Wipe
    } else {
        // Default to list if no command provided
        CliCommand::List
    }
}
