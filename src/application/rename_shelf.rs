use crate::domain::shelf::{Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn rename_shelf(
    shelf_ops: &dyn ShelfOps,
    shelf_id: u64,
    new_name: &str,
) -> Result<Shelf, FicflowError> {
    shelf_ops.update_shelf_name(shelf_id, new_name)
}
