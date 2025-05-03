use crate::domain::db::DatabaseOps;
use std::error::Error;

pub fn delete_fic(db_ops: &dyn DatabaseOps, fic_id: u64) -> Result<(), Box<dyn Error>> {
    db_ops.delete_fanfiction(fic_id)?;
    Ok(())
}