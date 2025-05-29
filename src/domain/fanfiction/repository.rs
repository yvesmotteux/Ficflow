use super::entity::Fanfiction;

pub trait DatabaseOps {
    fn insert_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn std::error::Error>>;
    fn update_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn std::error::Error>>;
    fn delete_fanfiction(&self, fic_id: u64) -> Result<(), Box<dyn std::error::Error>>;
    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, Box<dyn std::error::Error>>;
    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, Box<dyn std::error::Error>>;
    fn wipe_database(&self) -> Result<(), Box<dyn std::error::Error>>;
}
