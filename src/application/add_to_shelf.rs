use crate::domain::shelf::ShelfOps;
use crate::error::FicflowError;

pub fn add_to_shelf(
    shelf_ops: &dyn ShelfOps,
    fic_id: u64,
    shelf_id: u64,
) -> Result<(), FicflowError> {
    shelf_ops.add_fic_to_shelf(fic_id, shelf_id)
}
