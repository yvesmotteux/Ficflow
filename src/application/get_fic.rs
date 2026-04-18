use crate::domain::fanfiction::{Fanfiction, FanfictionOps};
use crate::error::FicflowError;

pub fn get_fanfiction(
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
) -> Result<Fanfiction, FicflowError> {
    fanfiction_ops.get_fanfiction_by_id(fic_id)
}
