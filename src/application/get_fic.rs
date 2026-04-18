use crate::domain::fanfiction::{DatabaseOps, Fanfiction};
use crate::error::FicflowError;

pub fn get_fanfiction(database: &dyn DatabaseOps, fic_id: u64) -> Result<Fanfiction, FicflowError> {
    database.get_fanfiction_by_id(fic_id)
}
