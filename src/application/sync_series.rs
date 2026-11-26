use crate::domain::fanfiction::{Fanfiction, FanfictionFetcher};
use crate::domain::series::{Series, SeriesOps};
use crate::error::FicflowError;

pub fn sync_series(
    fetcher: &dyn FanfictionFetcher,
    series_ops: &dyn SeriesOps,
    series_id: u64,
) -> Result<Series, FicflowError> {
    let series = fetcher.fetch_series(series_id)?;
    series_ops.save_series(&series)?;
    series_ops.get_series(series_id)
}

pub fn sync_fic_series(
    fetcher: &dyn FanfictionFetcher,
    series_ops: &dyn SeriesOps,
    fic: &Fanfiction,
) -> Result<(), FicflowError> {
    for series in &fic.series {
        match series_ops.get_series(series.id) {
            Err(FicflowError::SeriesNotFound { .. }) => {
                if let Err(e) = sync_series(fetcher, series_ops, series.id) {
                    log::warn!("Couldn't fetch series {}: {e}", series.id);
                }
            }
            Err(e) => return Err(e),
            Ok(_) => {}
        }
    }
    series_ops.link_fic_to_series(fic)
}
