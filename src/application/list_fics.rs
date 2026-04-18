use crate::domain::fanfiction::{Fanfiction, FanfictionOps};
use crate::error::FicflowError;

pub fn list_fics(fanfiction_ops: &dyn FanfictionOps) -> Result<Vec<Fanfiction>, FicflowError> {
    fanfiction_ops.list_fanfictions()
}
