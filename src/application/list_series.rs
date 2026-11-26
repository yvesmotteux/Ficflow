use crate::domain::series::{Series, SeriesOps};
use crate::error::FicflowError;

pub fn list_series(series_ops: &dyn SeriesOps) -> Result<Vec<Series>, FicflowError> {
    series_ops.list_series()
}
