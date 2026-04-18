use crate::domain::fanfiction::{DatabaseOps, FanfictionFetcher};
use crate::error::FicflowError;

pub fn add_fanfiction(
    fetcher: &dyn FanfictionFetcher,
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    base_url: &str,
) -> Result<String, FicflowError> {
    let fic = fetcher.fetch_fanfiction(fic_id, base_url)?;
    db_ops.save_fanfiction(&fic)?;
    Ok(fic.title)
}
