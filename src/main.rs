use ficflow::infrastructure::db::{establish_connection, Database};
use ficflow::infrastructure::ao3::fetch_fic::Ao3Fetcher;
use ficflow::interfaces::interface::InterfaceFactory;

fn main() {
    // Initialize core dependencies
    let conn = establish_connection().expect("Failed to establish database connection");
    let database = Database { conn: &conn };
    let fetcher = Ao3Fetcher;

    let factory = InterfaceFactory::new(&fetcher, &database);
    
    let interface = factory.create_cli_interface();
    
    interface.run();
}