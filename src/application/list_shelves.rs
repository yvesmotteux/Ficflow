use crate::domain::shelf::{Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn list_shelves(shelf_ops: &dyn ShelfOps) -> Result<Vec<Shelf>, FicflowError> {
    shelf_ops.list_shelves()
}
