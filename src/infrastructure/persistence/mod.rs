pub mod database;
pub mod repository;

pub use database::connection::establish_connection;
pub use database::sqlite_connection::Database;
pub use repository::fanfiction_repository::FanfictionRepository;
