pub mod persistence;
pub mod external;

pub use persistence::database::establish_connection;
pub use persistence::database::sqlite_connection::Database;
pub use persistence::repository::fanfiction_repository::FanfictionRepository;
pub use external::ao3::Ao3Fetcher;