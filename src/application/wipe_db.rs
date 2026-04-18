use crate::domain::fanfiction::DatabaseOps;
use crate::error::FicflowError;

pub fn wipe_database(db_ops: &dyn DatabaseOps) -> Result<(), FicflowError> {
    db_ops.wipe_database()
}
