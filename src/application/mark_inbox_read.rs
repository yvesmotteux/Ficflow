use crate::domain::inbox::InboxOps;
use crate::error::FicflowError;

pub fn mark_message_read(inbox_ops: &dyn InboxOps, message_id: u64) -> Result<(), FicflowError> {
    inbox_ops.mark_message_read(message_id)
}

pub fn mark_all_read(inbox_ops: &dyn InboxOps) -> Result<(), FicflowError> {
    inbox_ops.mark_all_read()
}
