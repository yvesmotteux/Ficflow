use thiserror::Error;

#[derive(Debug, Error)]
pub enum FicflowError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("fanfiction with ID {fic_id} not found")]
    NotFound { fic_id: u64 },

    #[error("fanfiction with ID {fic_id} already exists in your library")]
    AlreadyExists { fic_id: u64 },

    #[error("failed to parse {field}: {reason}")]
    Parse { field: String, reason: String },

    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("database migration error: {0}")]
    Migration(#[from] rusqlite_migration::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("{0}")]
    Other(String),
}
