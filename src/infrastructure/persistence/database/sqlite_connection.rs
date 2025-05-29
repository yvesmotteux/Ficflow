use rusqlite::Connection;

pub struct Database<'a> {
    pub conn: &'a Connection,
}

impl<'a> Database<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}
