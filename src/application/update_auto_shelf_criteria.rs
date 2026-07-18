use crate::domain::shelf::{AutoShelfCriteria, Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn update_auto_shelf_criteria(
    shelf_ops: &dyn ShelfOps,
    shelf_id: u64,
    criteria: AutoShelfCriteria,
) -> Result<Shelf, FicflowError> {
    shelf_ops.update_auto_shelf_criteria(shelf_id, criteria)
}
