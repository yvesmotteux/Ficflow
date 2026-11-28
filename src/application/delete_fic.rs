use crate::domain::fanfiction::{EpubStore, FanfictionOps};
use crate::error::FicflowError;

pub fn delete_fic(
    fanfiction_ops: &dyn FanfictionOps,
    epubs: &dyn EpubStore,
    fic_id: u64,
) -> Result<(), FicflowError> {
    fanfiction_ops.delete_fanfiction(fic_id)?;
    epubs.remove_epub(fic_id)
}
