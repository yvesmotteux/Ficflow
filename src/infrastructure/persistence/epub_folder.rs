use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::domain::fanfiction::EpubStore;
use crate::error::FicflowError;

pub struct EpubFolder {
    dir: PathBuf,
}

impl EpubFolder {
    pub fn for_library(db_path: &Path) -> Self {
        let library_dir = db_path.parent().unwrap_or(Path::new("."));
        Self {
            dir: library_dir.join("epubs"),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, fic_id: u64) -> PathBuf {
        self.dir.join(format!("{fic_id}.epub"))
    }

    pub fn has(&self, fic_id: u64) -> bool {
        self.path(fic_id).is_file()
    }
}

impl EpubStore for EpubFolder {
    fn save_epub(&self, fic_id: u64, epub: &[u8]) -> Result<(), FicflowError> {
        std::fs::create_dir_all(&self.dir)?;
        let partial = self.dir.join(format!("{fic_id}.epub.part"));
        std::fs::write(&partial, epub)?;
        std::fs::rename(&partial, self.path(fic_id))?;
        Ok(())
    }

    fn remove_epub(&self, fic_id: u64) -> Result<(), FicflowError> {
        match std::fs::remove_file(self.path(fic_id)) {
            Err(err) if err.kind() != ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }
}
