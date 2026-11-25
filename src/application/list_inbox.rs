use crate::domain::inbox::{InboxMessage, InboxOps};
use crate::error::FicflowError;

pub fn list_inbox(inbox_ops: &dyn InboxOps) -> Result<Vec<InboxMessage>, FicflowError> {
    inbox_ops.list_messages()
}
