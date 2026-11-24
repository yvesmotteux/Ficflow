use super::*;

impl FicflowApp {
    /// Fold the write-ahead log back into the main database file on exit so a
    /// cleanly-closed library is a single self-contained `.db` — this empties
    /// the `-wal` sidecar and makes "copy the file to back it up" reliable.
    /// Best-effort: the worker thread may still hold the WAL open, in which
    /// case SQLite still flushes committed frames into the `.db` even if it
    /// can't fully truncate the log.
    pub(super) fn checkpoint_wal(&self) {
        if let Err(err) = self
            .connection
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        {
            log::warn!("Failed to checkpoint the database on exit: {}", err);
        }
    }

    /// Opens the native folder/file picker for a Library action, parented to
    /// our own window so the portal dialog stacks above Ficflow instead of
    /// behind it. Called from `ui()`, the only place with a window handle.
    pub(super) fn open_library_picker(
        &mut self,
        request: settings_view::LibraryRequest,
        frame: &eframe::Frame,
    ) {
        let start_dir = self.current_db_path.parent().map(|p| p.to_path_buf());
        match request {
            settings_view::LibraryRequest::ChangeLocation => {
                let mut dialog = rfd::FileDialog::new().set_parent(frame);
                if let Some(dir) = &start_dir {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(folder) = dialog.pick_folder() {
                    self.change_library_location(folder);
                }
            }
            settings_view::LibraryRequest::Restore => {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter("SQLite database", &["db"])
                    .set_parent(frame);
                if let Some(dir) = &start_dir {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(backup) = dialog.pick_file() {
                    self.active_modal = ActiveModal::ConfirmRestore(backup);
                }
            }
        }
    }

    /// Takes effect on the next restart.
    pub(super) fn change_library_location(&mut self, folder: PathBuf) {
        match move_library_into(&self.current_db_path, &folder) {
            Ok(target) if target == self.current_db_path => {}
            Ok(target) => {
                self.config.shared.library_path = Some(target.clone());
                self.current_db_path = target;
                self.save_config();
                self.toasts
                    .info("Library location updated — restart Ficflow to load it.");
            }
            Err(err) => {
                self.toasts
                    .error(format!("Couldn't move the library: {}", err));
            }
        }
    }

    /// Copy a backup over the current library, leaving the configured
    /// location unchanged. Takes effect on the next restart.
    pub(super) fn restore_library_backup(&mut self, backup: PathBuf) {
        match restore_backup(&backup, &self.current_db_path) {
            Ok(()) => {
                self.toasts
                    .info("Backup restored — restart Ficflow to load it.");
            }
            Err(err) => {
                self.toasts
                    .error(format!("Couldn't restore the backup: {}", err));
            }
        }
    }
}
