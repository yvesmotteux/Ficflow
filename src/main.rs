use std::process::ExitCode;

use ficflow::domain::url_config;
use ficflow::infrastructure::{
    establish_connection, Ao3Fetcher, RetryingFetcher, SqliteRepository,
};
use ficflow::interfaces::interface::InterfaceFactory;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if let Ok(url) = std::env::var("AO3_BASE_URL") {
        url_config::set_ao3_base_url(&url);
    }

    let conn = establish_connection().expect("Failed to establish database connection");
    let repository = SqliteRepository::new(&conn);
    let ao3_fetcher = Ao3Fetcher::new().expect("Failed to create Ao3Fetcher");
    let fetcher = RetryingFetcher::new(ao3_fetcher, 3);

    let factory = InterfaceFactory::new(&fetcher, &repository);
    let interface = factory.create_cli_interface();

    interface.run()
}
