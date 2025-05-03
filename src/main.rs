use ficflow::infrastructure::db::{establish_connection, Database};
use ficflow::infrastructure::ao3::fetch_fic::Ao3Fetcher;
use ficflow::interfaces::cli::run_cli;

fn main() {
    let conn = establish_connection().expect("Failed to establish database connection");
    let database = Database { conn: &conn };
    let fetcher = Ao3Fetcher;

    run_cli(&fetcher, &database);
}