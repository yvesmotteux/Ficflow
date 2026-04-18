use super::entity::Fanfiction;
use crate::error::FicflowError;

pub trait FanfictionOps {
    fn save_fanfiction(&self, fic: &Fanfiction) -> Result<(), FicflowError>;
    fn delete_fanfiction(&self, fic_id: u64) -> Result<(), FicflowError>;
    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, FicflowError>;
    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, FicflowError>;
    fn wipe_database(&self) -> Result<(), FicflowError>;
}
