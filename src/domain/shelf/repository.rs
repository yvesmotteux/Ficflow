use super::entity::Shelf;
use crate::domain::fanfiction::Fanfiction;
use crate::error::FicflowError;

pub trait ShelfOps {
    fn create_shelf(&self, name: &str) -> Result<Shelf, FicflowError>;
    fn delete_shelf(&self, shelf_id: u64) -> Result<(), FicflowError>;
    fn list_shelves(&self) -> Result<Vec<Shelf>, FicflowError>;
    fn get_shelf_by_id(&self, shelf_id: u64) -> Result<Shelf, FicflowError>;
    fn add_fic_to_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError>;
    fn remove_fic_from_shelf(&self, fic_id: u64, shelf_id: u64) -> Result<(), FicflowError>;
    fn list_fics_in_shelf(&self, shelf_id: u64) -> Result<Vec<Fanfiction>, FicflowError>;
    fn list_shelves_for_fic(&self, fic_id: u64) -> Result<Vec<Shelf>, FicflowError>;
    fn count_fics_in_shelf(&self, shelf_id: u64) -> Result<usize, FicflowError>;
}
