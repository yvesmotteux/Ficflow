use std::collections::HashMap;

use super::entity::Shelf;
use crate::domain::fanfiction::Fanfiction;
use crate::error::FicflowError;

pub trait ShelfOps {
    fn create_shelf(&self, name: &str, parent_shelf_id: Option<u64>)
    -> Result<Shelf, FicflowError>;
    fn delete_shelf(&self, shelf_id: u64) -> Result<(), FicflowError>;
    fn move_shelf(&self, shelf_id: u64, new_parent: Option<u64>) -> Result<Shelf, FicflowError>;
    fn update_shelf_name(&self, shelf_id: u64, new_name: &str) -> Result<Shelf, FicflowError>;
    fn list_shelves(&self) -> Result<Vec<Shelf>, FicflowError>;
    fn get_shelf_by_id(&self, shelf_id: u64) -> Result<Shelf, FicflowError>;
    fn add_fic_to_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError>;
    fn remove_fic_from_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError>;
    fn list_fics_in_shelf(&self, shelf_id: u64) -> Result<Vec<Fanfiction>, FicflowError>;
    fn list_shelves_for_fic(&self, fic_id: u64) -> Result<Vec<Shelf>, FicflowError>;
    fn count_fics_in_shelf(&self, shelf_id: u64) -> Result<usize, FicflowError>;
    /// Bulk equivalent of `count_fics_in_shelf` — returns the distinct
    /// non-deleted-fic count (own fics plus descendant shelves') for
    /// every shelf that has at least one, in a single query. Shelves
    /// whose subtree holds zero non-deleted fics are absent from the
    /// map (callers default missing keys to 0).
    fn count_fics_per_shelf(&self) -> Result<HashMap<u64, usize>, FicflowError>;
}
