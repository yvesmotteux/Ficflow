use crate::domain::fanfiction::DatabaseOps;
use crate::domain::fanfiction::Fanfiction;
use std::error::Error;

pub fn list_fics(db_ops: &dyn DatabaseOps) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
    db_ops.list_fanfictions()
}