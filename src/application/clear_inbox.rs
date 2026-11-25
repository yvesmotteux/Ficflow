use crate::domain::inbox::InboxOps;
use crate::error::FicflowError;

pub fn clear_inbox(inbox_ops: &dyn InboxOps) -> Result<(), FicflowError> {
    inbox_ops.clear_inbox()
}
