use crate::domain::db::DatabaseOps;
use std::error::Error;

pub fn list_fics(db_ops: &dyn DatabaseOps) -> Result<(), Box<dyn Error>> {
    let fanfictions = db_ops.list_fanfictions()?;
    for fic in fanfictions {
        println!(
            "ID: {}, Title: {}, Authors: {:?}, Fandoms: {:?}, Words: {}",
            fic.id, fic.title, fic.authors, fic.fandoms, fic.words
        );
    }
    Ok(())
}