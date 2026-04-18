use crate::domain::fanfiction::FanfictionOps;
use crate::error::FicflowError;

pub fn wipe_database(fanfiction_ops: &dyn FanfictionOps) -> Result<(), FicflowError> {
    fanfiction_ops.wipe_database()
}
