use crate::domain::inbox::InboxOps;
use crate::error::FicflowError;

pub fn delete_inbox_message(inbox_ops: &dyn InboxOps, message_id: u64) -> Result<(), FicflowError> {
    inbox_ops.delete_message(message_id)
}
