use std::collections::HashMap;

use crate::domain::shelf::ShelfOps;
use crate::error::FicflowError;

pub fn count_fics_per_shelf(shelf_ops: &dyn ShelfOps) -> Result<HashMap<u64, usize>, FicflowError> {
    shelf_ops.count_fics_per_shelf()
}
