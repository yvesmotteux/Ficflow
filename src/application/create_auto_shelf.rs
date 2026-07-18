use crate::domain::shelf::{AutoShelfCriteria, Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn create_auto_shelf(
    shelf_ops: &dyn ShelfOps,
    name: &str,
    parent_shelf_id: Option<u64>,
    criteria: AutoShelfCriteria,
) -> Result<Shelf, FicflowError> {
    shelf_ops.create_auto_shelf(name, parent_shelf_id, criteria)
}
