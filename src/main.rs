use ficflow::infrastructure::{establish_connection, SqliteRepository, Ao3Fetcher};
use ficflow::interfaces::interface::InterfaceFactory;

fn main() {
    let conn = establish_connection().expect("Failed to establish database connection");
    let repository = SqliteRepository::new(&conn);
    let fetcher = Ao3Fetcher::new().expect("Failed to create Ao3Fetcher");

    let factory = InterfaceFactory::new(&fetcher, &repository);
    let interface = factory.create_cli_interface();

    interface.run();
}
