use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_SHELF_DEPTH: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shelf {
    pub id: u64,
    pub name: String,
    pub parent_shelf_id: Option<u64>,
    pub pinned: bool,
    pub created_at: DateTime<Utc>,
}
