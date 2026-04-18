use crate::domain::fanfiction::DatabaseOps;
use crate::error::FicflowError;

pub fn delete_fic(db_ops: &dyn DatabaseOps, fic_id: u64) -> Result<(), FicflowError> {
    db_ops.delete_fanfiction(fic_id)
}
