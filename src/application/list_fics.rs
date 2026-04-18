use crate::domain::fanfiction::{DatabaseOps, Fanfiction};
use crate::error::FicflowError;

pub fn list_fics(db_ops: &dyn DatabaseOps) -> Result<Vec<Fanfiction>, FicflowError> {
    db_ops.list_fanfictions()
}
