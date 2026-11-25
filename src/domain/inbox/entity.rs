use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MessageContent {
    NewChapters {
        fic_id: u64,
        title: String,
        authors: Vec<String>,
        fandoms: Vec<String>,
        previous_count: u32,
        new_count: u32,
        first_new_chapter_id: Option<u64>,
    },
}

impl MessageContent {
    pub fn fic_id(&self) -> Option<u64> {
        match self {
            MessageContent::NewChapters { fic_id, .. } => Some(*fic_id),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InboxMessage {
    pub id: u64,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub content: MessageContent,
}

impl InboxMessage {
    pub fn is_unread(&self) -> bool {
        self.read_at.is_none()
    }
}
