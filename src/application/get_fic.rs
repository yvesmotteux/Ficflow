use crate::domain::fanfiction::DatabaseOps;
use crate::domain::fanfiction::Fanfiction;
use std::error::Error;

pub fn get_fanfiction(database: &dyn DatabaseOps, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
    database.get_fanfiction_by_id(fic_id)
}