use crate::domain::fanfiction::Fanfiction;
use crate::domain::repository::Repository;
use crate::domain::shelf::ShelfKind;
use crate::error::FicflowError;

pub fn list_shelf_fics(
    repository: &dyn Repository,
    shelf_id: u64,
) -> Result<Vec<Fanfiction>, FicflowError> {
    match repository.get_shelf_by_id(shelf_id)?.kind {
        ShelfKind::Auto(criteria) => Ok(repository
            .list_fanfictions()?
            .into_iter()
            .filter(|fic| criteria.matches(fic))
            .collect()),
        ShelfKind::Normal => repository.list_fics_in_shelf(shelf_id),
    }
}
