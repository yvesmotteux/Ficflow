use clap::{Command, Arg};

#[derive(Debug)]
pub enum CliCommand {
    Add { fic_id: u64 },
    Delete { fic_id: u64 },
    Get { fic_id: u64 },
    List,
    Wipe,
}

pub fn parse_cli_commands() -> CliCommand {
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
        CliCommand::Add { fic_id: fic_id.parse::<u64>().unwrap() }
    } else if let Some(matches) = matches.subcommand_matches("delete") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        CliCommand::Delete { fic_id: fic_id.parse::<u64>().unwrap() }
    } else if let Some(matches) = matches.subcommand_matches("get") {
        let fic_id = matches.get_one::<String>("fic-id").expect("fic-id is required");
        CliCommand::Get { fic_id: fic_id.parse::<u64>().unwrap() }
    } else if matches.subcommand_matches("list").is_some() {
        CliCommand::List
    } else if matches.subcommand_matches("wipe").is_some() {
        CliCommand::Wipe
    } else {
        // Default to list if no command provided
        CliCommand::List
    }
}
