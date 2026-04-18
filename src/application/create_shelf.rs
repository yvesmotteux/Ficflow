use crate::domain::shelf::{Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn create_shelf(shelf_ops: &dyn ShelfOps, name: &str) -> Result<Shelf, FicflowError> {
    shelf_ops.create_shelf(name)
}
