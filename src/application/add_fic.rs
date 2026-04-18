use crate::domain::fanfiction::{FanfictionFetcher, FanfictionOps};
use crate::error::FicflowError;

pub fn add_fanfiction(
    fetcher: &dyn FanfictionFetcher,
    fanfiction_ops: &dyn FanfictionOps,
    fic_id: u64,
    base_url: &str,
) -> Result<String, FicflowError> {
    match fanfiction_ops.get_fanfiction_by_id(fic_id) {
        Ok(_) => return Err(FicflowError::AlreadyExists { fic_id }),
        Err(FicflowError::NotFound { .. }) => {}
        Err(e) => return Err(e),
    }

    let fic = fetcher.fetch_fanfiction(fic_id, base_url)?;
    fanfiction_ops.save_fanfiction(&fic)?;
    Ok(fic.title)
}
