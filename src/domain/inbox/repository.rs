use super::entity::{InboxMessage, MessageContent};
use crate::error::FicflowError;

pub trait InboxOps {
    fn post_message(&self, content: &MessageContent) -> Result<InboxMessage, FicflowError>;
    fn list_messages(&self) -> Result<Vec<InboxMessage>, FicflowError>;
    fn delete_message(&self, message_id: u64) -> Result<(), FicflowError>;
    fn clear_inbox(&self) -> Result<(), FicflowError>;
    fn mark_message_read(&self, message_id: u64) -> Result<(), FicflowError>;
    fn mark_all_read(&self) -> Result<(), FicflowError>;
}
