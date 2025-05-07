use clap::{Command, Arg};
use crate::{
    application::{add_fic::add_fanfiction, delete_fic::delete_fic, list_fics::list_fics, 
                  wipe_db::wipe_database, get_fic::{get_fanfiction, display_fanfiction_details}}, 
    domain::{db::DatabaseOps, fic::FanfictionFetcher, config},
};
use crate::infrastructure::db;

pub fn run_cli(fetcher: &dyn FanfictionFetcher, database: &dyn DatabaseOps) {
    let matches = Command::new("FicFlow")
        .subcommand(
            Command::new("add")
                .about("Add a fanfiction to the database")
                .arg(Arg::new("fic-id").required(true).index(1).help("The ID of the fanfiction")),
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
        .subcommand(Command::new("list").about("List all stored fanfictions"))
        .subcommand(Command::new("wipe").about("Wipe the database (removes all fanfictions)"))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("add") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        println!("Adding fanfiction with ID: {}", fic_id);
        
        let base_url = config::get_ao3_base_url();
        
        if let Err(e) = add_fanfiction(fetcher, database, fic_id.parse::<u64>().unwrap(), &base_url) {
            eprintln!("Error adding fanfiction: {}", e);
        }
    } else if let Some(matches) = matches.subcommand_matches("delete") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        println!("Deleting fanfiction with ID: {}", fic_id);
        if let Err(e) = delete_fic(database, fic_id.parse::<u64>().unwrap()) {
            eprintln!("Error deleting fanfiction: {}", e);
        }
    } else if let Some(matches) = matches.subcommand_matches("get") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        println!("Getting fanfiction with ID: {}", fic_id);
        match get_fanfiction(database, fic_id.parse::<u64>().unwrap()) {
            Ok(fic) => {
                let details = display_fanfiction_details(&fic);
                println!("\n{}", details);
            },
            Err(e) => {
                eprintln!("Error getting fanfiction: {}", e);
            }
        }
    } else if matches.subcommand_matches("list").is_some() {
        println!("Listing all fanfictions");
        if let Err(e) = list_fics(database) {
            eprintln!("Error listing fanfictions: {}", e);
        }
    } else if matches.subcommand_matches("wipe").is_some() {
        println!("Preparing to wipe database...");
        
        // For the wipe command, establish a direct connection to the database
        match db::establish_connection() {
            Ok(mut conn) => {
                if let Err(e) = wipe_database(database, &mut conn) {
                    eprintln!("Error wiping database: {}", e);
                }
            },
            Err(e) => {
                eprintln!("Error connecting to database: {}", e);
            }
        }
    }
}