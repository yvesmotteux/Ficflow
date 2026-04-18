use ficflow::infrastructure::{
    establish_connection, Ao3Fetcher, RetryingFetcher, SqliteRepository,
};
use ficflow::interfaces::interface::InterfaceFactory;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let conn = establish_connection().expect("Failed to establish database connection");
    let repository = SqliteRepository::new(&conn);
    let ao3_fetcher = Ao3Fetcher::new().expect("Failed to create Ao3Fetcher");
    let fetcher = RetryingFetcher::new(ao3_fetcher, 3);

    let factory = InterfaceFactory::new(&fetcher, &repository);
    let interface = factory.create_cli_interface();

    interface.run();
}
