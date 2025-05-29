use std::error::Error;
use crate::domain::fanfiction::Fanfiction;
use crate::domain::fanfiction::DatabaseOps;
use crate::infrastructure::persistence::database::sqlite_connection::Database;
use crate::infrastructure::persistence::repository::operations::{
    insert_fanfiction, update_fanfiction, delete_fanfiction,
    get_all_fanfictions, get_fanfiction_by_id,
    wipe_database
};

pub struct FanfictionRepository<'a> {
    database: Database<'a>,
}

impl<'a> FanfictionRepository<'a> {
    pub fn new(database: Database<'a>) -> Self {
        Self { database }
    }
}

impl<'a> DatabaseOps for FanfictionRepository<'a> {
    fn insert_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
        insert_fanfiction(self.database.conn, fic)
    }
    
    fn update_fanfiction(&self, fic: &Fanfiction) -> Result<(), Box<dyn Error>> {
        update_fanfiction(self.database.conn, fic)
    }

    fn delete_fanfiction(&self, fic_id: u64) -> Result<(), Box<dyn Error>> {
        delete_fanfiction(self.database.conn, fic_id)
    }

    fn list_fanfictions(&self) -> Result<Vec<Fanfiction>, Box<dyn Error>> {
        get_all_fanfictions(self.database.conn)
    }
    
    fn get_fanfiction_by_id(&self, fic_id: u64) -> Result<Fanfiction, Box<dyn Error>> {
        get_fanfiction_by_id(self.database.conn, fic_id)
    }

    fn wipe_database(&self) -> Result<(), Box<dyn Error>> {
        wipe_database(self.database.conn)
    }
}
