use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shelf {
    pub id: u64,
    pub name: String,
    pub created_at: DateTime<Utc>,
}
