use super::entity::Series;
use crate::domain::fanfiction::Fanfiction;
use crate::error::FicflowError;

pub trait SeriesOps {
    fn save_series(&self, series: &Series) -> Result<(), FicflowError>;
    fn link_fic_to_series(&self, fic: &Fanfiction) -> Result<(), FicflowError>;
    fn get_series(&self, series_id: u64) -> Result<Series, FicflowError>;
    fn list_series(&self) -> Result<Vec<Series>, FicflowError>;
}
