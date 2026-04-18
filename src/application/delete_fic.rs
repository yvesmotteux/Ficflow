use crate::domain::fanfiction::FanfictionOps;
use crate::error::FicflowError;

pub fn delete_fic(fanfiction_ops: &dyn FanfictionOps, fic_id: u64) -> Result<(), FicflowError> {
    fanfiction_ops.delete_fanfiction(fic_id)
}
