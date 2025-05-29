use crate::domain::db::DatabaseOps;
use crate::domain::fic::Fanfiction;
use std::error::Error;

pub fn list_fics(db_ops: &dyn DatabaseOps) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
    db_ops.list_fanfictions()
}