use crate::domain::shelf::{Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn move_shelf(
    shelf_ops: &dyn ShelfOps,
    shelf_id: u64,
    new_parent: Option<u64>,
) -> Result<Shelf, FicflowError> {
    shelf_ops.move_shelf(shelf_id, new_parent)
}
