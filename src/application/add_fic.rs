use crate::domain::fanfiction::{DatabaseOps, FanfictionFetcher};
use crate::error::FicflowError;

pub fn add_fanfiction(
    fetcher: &dyn FanfictionFetcher,
    db_ops: &dyn DatabaseOps,
    fic_id: u64,
    base_url: &str,
) -> Result<String, FicflowError> {
    match db_ops.get_fanfiction_by_id(fic_id) {
        Ok(_) => return Err(FicflowError::AlreadyExists { fic_id }),
        Err(FicflowError::NotFound { .. }) => {}
        Err(e) => return Err(e),
    }

    let fic = fetcher.fetch_fanfiction(fic_id, base_url)?;
    db_ops.save_fanfiction(&fic)?;
    Ok(fic.title)
}
