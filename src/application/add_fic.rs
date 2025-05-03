use crate::domain::fic::FanfictionFetcher;
use crate::domain::db::DatabaseOps;

pub fn add_fanfiction(
    fetcher: &dyn FanfictionFetcher,
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let fic = fetcher.fetch_fanfiction(fic_id, "https://archiveofourown.org")?;
    db_ops.insert_fanfiction(&fic)?;
    Ok(())
}