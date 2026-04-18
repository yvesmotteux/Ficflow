use crate::domain::shelf::ShelfOps;
use crate::error::FicflowError;

pub fn delete_shelf(shelf_ops: &dyn ShelfOps, shelf_id: u64) -> Result<(), FicflowError> {
    shelf_ops.delete_shelf(shelf_id)
}
