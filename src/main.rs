use ficflow::infrastructure::{establish_connection, FanfictionRepository, Ao3Fetcher};
use ficflow::interfaces::interface::InterfaceFactory;

fn main() {
    // Initialize core dependencies
    let conn = establish_connection().expect("Failed to establish database connection");
    let db_instance = ficflow::infrastructure::persistence::database::sqlite_connection::Database::new(&conn);
    let fanfiction_repo = FanfictionRepository::new(db_instance);
    let fetcher = Ao3Fetcher::new().expect("Failed to create Ao3Fetcher");

    let factory = InterfaceFactory::new(&fetcher, &fanfiction_repo);
    
    let interface = factory.create_cli_interface();
    
    interface.run();
}