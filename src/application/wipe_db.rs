use crate::domain::fanfiction::DatabaseOps;
use std::error::Error;

pub fn wipe_database(db_ops: &dyn DatabaseOps) -> Result<(), Box<dyn Error>> {
    db_ops.wipe_database()
}
