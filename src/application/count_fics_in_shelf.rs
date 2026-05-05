use crate::domain::shelf::ShelfOps;
use crate::error::FicflowError;

pub fn count_fics_in_shelf(shelf_ops: &dyn ShelfOps, shelf_id: u64) -> Result<usize, FicflowError> {
    shelf_ops.count_fics_in_shelf(shelf_id)
}
