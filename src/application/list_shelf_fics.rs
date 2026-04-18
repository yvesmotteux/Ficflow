use crate::domain::fanfiction::Fanfiction;
use crate::domain::shelf::ShelfOps;
use crate::error::FicflowError;

pub fn list_shelf_fics(
    shelf_ops: &dyn ShelfOps,
    shelf_id: u64,
) -> Result<Vec<Fanfiction>, FicflowError> {
    shelf_ops.list_fics_in_shelf(shelf_id)
}
