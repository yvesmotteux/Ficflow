use super::sync_series::sync_fic_series;
use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher, FanfictionOps};
use crate::domain::series::SeriesOps;
use crate::error::FicflowError;

pub fn add_fanfiction(
    fetcher: &dyn FanfictionFetcher,
    fanfiction_ops: &dyn FanfictionOps,
    series_ops: &dyn SeriesOps,
    fic_id: u64,
) -> Result<Fanfiction, FicflowError> {
    match fanfiction_ops.get_fanfiction_by_id(fic_id) {
        Ok(_) => return Err(FicflowError::AlreadyExists { fic_id }),
        Err(FicflowError::NotFound { .. }) => {}
        Err(e) => return Err(e),
    }

    let fic = fetcher.fetch_fanfiction(fic_id)?;
    fanfiction_ops.save_fanfiction(&fic)?;
    sync_fic_series(fetcher, series_ops, &fic)?;
    Ok(fic)
}
