use std::process::ExitCode;

use ficflow::infrastructure::external::ao3::fetcher::{
    ALT_AO3_URL, PRIMARY_AO3_URL, PROXY_AO3_URL,
};
use ficflow::infrastructure::{establish_connection, Ao3Fetcher, SqliteRepository};
use ficflow::interfaces::interface::InterfaceFactory;

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // An explicit AO3_BASE_URL pin (e.g. integration tests) disables fallback: only that URL is used.
    let (urls, max_cycles) = match std::env::var("AO3_BASE_URL") {
        Ok(url) => (vec![url], 3),
        Err(_) => (
            vec![
                PRIMARY_AO3_URL.to_string(),
                ALT_AO3_URL.to_string(),
                PROXY_AO3_URL.to_string(),
            ],
            2,
        ),
    };

    let fetcher = Ao3Fetcher::new(urls, max_cycles).expect("Failed to create Ao3Fetcher");
    let conn = establish_connection().expect("Failed to establish database connection");
    let repository = SqliteRepository::new(&conn);

    let factory = InterfaceFactory::new(&fetcher, &repository);
    let interface = factory.create_cli_interface();

    interface.run()
}
