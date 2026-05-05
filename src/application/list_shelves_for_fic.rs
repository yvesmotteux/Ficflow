use crate::domain::shelf::{Shelf, ShelfOps};
use crate::error::FicflowError;

pub fn list_shelves_for_fic(
    shelf_ops: &dyn ShelfOps,
    fic_id: u64,
) -> Result<Vec<Shelf>, FicflowError> {
    shelf_ops.list_shelves_for_fic(fic_id)
}
