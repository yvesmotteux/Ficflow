use crate::domain::shelf::{Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn unpin_shelf(shelf_ops: &dyn ShelfOps, shelf_id: u64) -> Result<Shelf, FicflowError> {
    shelf_ops.set_shelf_pinned(shelf_id, false)
}
