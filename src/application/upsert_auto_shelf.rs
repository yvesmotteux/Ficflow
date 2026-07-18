use crate::domain::shelf::{AutoShelfCriteria, Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn upsert_auto_shelf(
    shelf_ops: &dyn ShelfOps,
    shelf_id: Option<u64>,
    name: &str,
    parent_shelf_id: Option<u64>,
    criteria: AutoShelfCriteria,
) -> Result<Shelf, FicflowError> {
    shelf_ops.upsert_auto_shelf(shelf_id, name, parent_shelf_id, criteria)
}
