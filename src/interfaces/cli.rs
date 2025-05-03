use clap::{Command, Arg};
use crate::{
    application::{add_fic::add_fanfiction, delete_fic::delete_fic, list_fics::list_fics}, 
    domain::{db::DatabaseOps, fic::FanfictionFetcher},
};

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
        .subcommand(Command::new("list").about("List all stored fanfictions"))
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("add") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        println!("Adding fanfiction with ID: {}", fic_id);
        if let Err(e) = add_fanfiction(fetcher, database, fic_id.parse::<u64>().unwrap()) {
            eprintln!("Error adding fanfiction: {}", e);
        }
    } else if let Some(matches) = matches.subcommand_matches("delete") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        println!("Deleting fanfiction with ID: {}", fic_id);
        if let Err(e) = delete_fic(database, fic_id.parse::<u64>().unwrap()) {
            eprintln!("Error deleting fanfiction: {}", e);
        }
    } else if matches.subcommand_matches("list").is_some() {
        println!("Listing all fanfictions");
        if let Err(e) = list_fics(database) {
            eprintln!("Error listing fanfictions: {}", e);
        }
    }
}