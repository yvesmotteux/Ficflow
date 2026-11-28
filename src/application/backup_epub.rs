use crate::domain::fanfiction::{EpubStore, Fanfiction, FanfictionFetcher};
use crate::error::FicflowError;

pub fn backup_epub(
    fetcher: &dyn FanfictionFetcher,
    store: &dyn EpubStore,
    fic: &Fanfiction,
) -> Result<(), FicflowError> {
    let path = fic
        .epub_path
        .as_deref()
        .ok_or_else(|| FicflowError::Other("AO3 offers no EPUB for this fic".into()))?;
    let epub = fetcher.download_epub(path)?;
    store.save_epub(fic.id, &epub)
}
