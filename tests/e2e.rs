//! CLI end-to-end tests. Most user flows that used to live here moved
//! to the headless GUI suite (`tests/gui/`) once Phase 11 made the
//! GUI's control surface drivable from tests. The few that remain
//! exercise paths the GUI doesn't expose at all:
//!
//!  * `wipe` is CLI-only (no destructive button in the UI).
//!  * Adding a *soft-deleted* fic to a shelf is something the GUI
//!    can't even attempt, since the dropdown only lists live shelves
//!    and the Add Fic dialog re-fetches the fic (which moves it out
//!    of the soft-deleted state). The CLI exposes the raw `shelf add
//!    <fic-id> <shelf-id>` path, which is where the not-found
//!    rejection lives.

use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "common/mod.rs"]
mod common;
use common::{assertions, fixtures};

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::TempDir;

    // Helper struct to keep the tempdir alive during the test
    struct TestDatabase {
        conn: Connection,
        db_path: PathBuf,
        _temp_dir: TempDir,
    }

    fn setup_test_db() -> TestDatabase {
        let (conn, db_path, temp_dir) = fixtures::given_test_database();
        TestDatabase {
            conn,
            db_path,
            _temp_dir: temp_dir,
        }
    }

    fn run_cli_command(
        args: &[&str],
        db_path: &Path,
        mock_url: &str,
        additional_env: Option<(&str, &str)>,
    ) -> (String, String, i32) {
        let binary_path = env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join("ficflow");

        assert!(
            binary_path.exists(),
            "Binary not found at {:?}. Run `cargo build` first.",
            binary_path
        );

        let mut cmd = Command::new(binary_path);
        cmd.env("FICFLOW_DB_PATH", db_path.to_str().unwrap())
            .env("AO3_BASE_URL", mock_url);
        if let Some((key, value)) = additional_env {
            cmd.env(key, value);
        }
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd.output().expect("Failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        (stdout, stderr, output.status.code().unwrap_or(0))
    }

    /// Adding a soft-deleted fic to a shelf must be rejected at the
    /// repo layer. The GUI hides soft-deleted fics from every UI
    /// surface so it can't naturally trigger this; only the CLI's
    /// `shelf add <fic-id> <shelf-id>` lets a user (or a script) name
    /// the id directly.
    #[test]
    fn test_add_to_shelf_rejects_soft_deleted_fic() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();
        let db_path = &test_db.db_path;

        // Given: fic added then soft-deleted; a shelf exists
        let (_, add_err, add_status) =
            run_cli_command(&["add", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(add_status, &add_err, None, None);
        let (_, del_err, del_status) =
            run_cli_command(&["delete", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(del_status, &del_err, None, None);
        let (_, sc_err, sc_status) =
            run_cli_command(&["shelf", "create", "Favorites"], db_path, &base, None);
        assertions::then_command_succeeded(sc_status, &sc_err, None, None);

        // When/Then: shelf add rejects the soft-deleted fic
        let (_, sa_err, sa_status) = run_cli_command(
            &["shelf", "add", &fic_id.to_string(), "1"],
            db_path,
            &base,
            None,
        );
        assert_ne!(
            sa_status, 0,
            "expected shelf add to fail for soft-deleted fic"
        );
        assert!(
            sa_err.contains("not found"),
            "expected 'not found' in stderr, got: {}",
            sa_err
        );

        Ok(())
    }

    /// `wipe` only clears fanfictions; user-curated shelves should
    /// survive. Wipe has no GUI counterpart so the test stays here.
    #[test]
    fn test_wipe_leaves_shelves_alone() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();
        let db_path = &test_db.db_path;

        // Given: fic added and a shelf exists
        let (_, add_err, add_status) =
            run_cli_command(&["add", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(add_status, &add_err, None, None);
        let (_, sc_err, sc_status) =
            run_cli_command(&["shelf", "create", "Favorites"], db_path, &base, None);
        assertions::then_command_succeeded(sc_status, &sc_err, None, None);

        // When: wipe
        let (_, wipe_err, wipe_status) = run_cli_command(
            &["wipe"],
            db_path,
            &base,
            Some(("FICFLOW_NON_INTERACTIVE", "1")),
        );
        assertions::then_command_succeeded(wipe_status, &wipe_err, None, None);

        // Then: no fanfictions
        assertions::then_database_was_wiped(&test_db.conn)?;

        // And: shelves untouched
        let (list_out, list_err, list_status) =
            run_cli_command(&["shelf", "list"], db_path, &base, None);
        assertions::then_command_succeeded(
            list_status,
            &list_err,
            Some(&["Favorites"]),
            Some(&list_out),
        );

        Ok(())
    }

    /// Add → get → wipe round-trip via the CLI binary. Covers the
    /// `get` command's pretty-print output and the `wipe` happy path
    /// — neither has a GUI counterpart.
    #[test]
    fn test_add_get_wipe_fanfiction() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();

        // Add
        let (_add_stdout, add_stderr, add_status) = run_cli_command(
            &["add", &fic_id.to_string()],
            &test_db.db_path,
            &mock_server.base_url(),
            None,
        );
        assertions::then_command_succeeded(add_status, &add_stderr, None, None);

        // Get — pretty-prints summary fields
        let (get_stdout, get_stderr, get_status) = run_cli_command(
            &["get", &fic_id.to_string()],
            &test_db.db_path,
            &mock_server.base_url(),
            None,
        );
        let expected_strings = &["Featherlight", "Gummy_bean", "Hazbin Hotel"];
        assertions::then_command_succeeded(
            get_status,
            &get_stderr,
            Some(expected_strings),
            Some(&get_stdout),
        );

        // Wipe
        let (_wipe_stdout, wipe_stderr, wipe_status) = run_cli_command(
            &["wipe"],
            &test_db.db_path,
            &mock_server.base_url(),
            Some(("FICFLOW_NON_INTERACTIVE", "1")),
        );
        assertions::then_command_succeeded(wipe_status, &wipe_stderr, None, None);
        assertions::then_database_was_wiped(&test_db.conn)?;

        Ok(())
    }
}
