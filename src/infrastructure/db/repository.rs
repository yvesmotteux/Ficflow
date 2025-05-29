use std::error::Error;
use crate::domain::fanfiction::Fanfiction;
use crate::domain::fanfiction::DatabaseOps;
use crate::infrastructure::db::operations::{
    insert_fanfiction, delete_fanfiction, 
    get_all_fanfictions, get_fanfiction_by_id,
    wipe_database
};
use crate::infrastructure::db::connection::Database;

impl<'a> DatabaseOps for Database<'a> {
    fn insert_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
        insert_fanfiction(self.conn, fic)
    }

    fn delete_fanfiction(&self, fic_id: u64) -> Result<(), Box<dyn Error>> {
        let result = delete_fanfiction(self.conn, fic_id);
        Ok(result?)
    }

    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
        get_all_fanfictions(self.conn)
    }
    
    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
        get_fanfiction_by_id(self.conn, fic_id)
    }

    fn wipe_database(&self) -> Result<(), Box<dyn Error>> {
        let result = wipe_database(self.conn);
        Ok(result?)
    }
}
