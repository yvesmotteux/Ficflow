use std::error::Error;
use std::fs;
use tempfile::TempDir;

use crate::common::{assertions, fixtures};

#[cfg(test)]
mod tests {
    use ficflow::infrastructure::persistence::database::connection::{
        open_configured_db, relocate_library, restore_backup,
    };

    use super::*;

    #[test]
    fn relocate_moves_database_and_sidecars_and_preserves_data() -> Result<(), Box<dyn Error>> {
        let src_dir = TempDir::new()?;
        let from = src_dir.path().join("fanfictions.db");
        let conn = open_configured_db(&from)?;
        let fic = fixtures::given_sample_fanfiction(1, "Relocated Fic");
        fixtures::when_fanfiction_added_to_db(&conn, &fic)?;
        drop(conn);
        fs::write(src_dir.path().join("fanfictions.db-wal"), b"wal")?;
        fs::write(src_dir.path().join("fanfictions.db-shm"), b"shm")?;

        let dest_dir = TempDir::new()?;
        let to = dest_dir.path().join("moved.db");
        relocate_library(&from, &to)?;

        assert!(!from.exists());
        assert!(to.exists());
        assert!(dest_dir.path().join("moved.db-wal").exists());
        assert!(dest_dir.path().join("moved.db-shm").exists());

        let moved = open_configured_db(&to)?;
        assertions::then_fanfiction_was_added(&moved, &fic)?;
        Ok(())
    }

    #[test]
    fn relocate_creates_missing_destination_directory() -> Result<(), Box<dyn Error>> {
        let src_dir = TempDir::new()?;
        let from = src_dir.path().join("fanfictions.db");
        drop(open_configured_db(&from)?);

        let dest_dir = TempDir::new()?;
        let to = dest_dir.path().join("nested").join("deeper").join("lib.db");
        relocate_library(&from, &to)?;

        assert!(to.exists());
        Ok(())
    }

    #[test]
    fn restore_overwrites_current_with_backup_and_clears_sidecars() -> Result<(), Box<dyn Error>> {
        let backup_dir = TempDir::new()?;
        let backup = backup_dir.path().join("backup.db");
        let backup_conn = open_configured_db(&backup)?;
        let fic = fixtures::given_sample_fanfiction(42, "Backed-up Fic");
        fixtures::when_fanfiction_added_to_db(&backup_conn, &fic)?;
        drop(backup_conn);

        let current_dir = TempDir::new()?;
        let current = current_dir.path().join("fanfictions.db");
        let current_conn = open_configured_db(&current)?;
        let other = fixtures::given_sample_fanfiction(7, "Current Fic");
        fixtures::when_fanfiction_added_to_db(&current_conn, &other)?;
        drop(current_conn);
        fs::write(current_dir.path().join("fanfictions.db-wal"), b"stale")?;

        restore_backup(&backup, &current)?;

        assert!(!current_dir.path().join("fanfictions.db-wal").exists());
        let restored = open_configured_db(&current)?;
        assertions::then_fanfiction_was_added(&restored, &fic)?;
        assertions::then_fanfiction_was_deleted(&restored, other.id)?;
        Ok(())
    }
}
