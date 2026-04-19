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
        _temp_dir: TempDir, // Prefixed with _ to indicate it's kept only for its lifetime
    }

    // Helper function to set up a test database
    fn setup_test_db() -> TestDatabase {
        // Create a temporary directory for the database
        let (conn, db_path, temp_dir) = fixtures::given_test_database();

        TestDatabase {
            conn,
            db_path,
            _temp_dir: temp_dir,
        }
    }

    // Helper function to run CLI command
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

        // Make sure the binary exists
        assert!(
            binary_path.exists(),
            "Binary not found at {:?}. Run `cargo build` first.",
            binary_path
        );

        let mut cmd = Command::new(binary_path);

        // Set environment variables for the test
        cmd.env("FICFLOW_DB_PATH", db_path.to_str().unwrap())
            .env("AO3_BASE_URL", mock_url);

        // Add additional environment variable if provided
        if let Some((key, value)) = additional_env {
            cmd.env(key, value);
        }

        // Add arguments
        for arg in args {
            cmd.arg(arg);
        }

        // Run the command and capture output
        let output = cmd.output().expect("Failed to execute command");

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        println!("Command stdout: {}", stdout);
        println!("Command stderr: {}", stderr);

        (stdout, stderr, output.status.code().unwrap_or(0))
    }

    #[test]
    fn test_add_list_remove_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();

        // When - Add fanfiction via CLI
        let (_add_stdout, add_stderr, add_status) = run_cli_command(
            &["add", &fic_id.to_string()],
            &test_db.db_path,
            &mock_server.base_url(),
            None,
        );

        // Then - Check if add was successful
        assertions::then_command_succeeded(add_status, &add_stderr, None, None);

        // When - List fanfictions via CLI
        let (list_stdout, list_stderr, list_status) =
            run_cli_command(&["list"], &test_db.db_path, &mock_server.base_url(), None);

        // Then - Check if list shows our fanfiction
        let expected_strings = &["Featherlight", &fic_id.to_string()];
        assertions::then_command_succeeded(
            list_status,
            &list_stderr,
            Some(expected_strings),
            Some(&list_stdout),
        );

        // When - Delete fanfiction via CLI
        let (_delete_stdout, delete_stderr, delete_status) = run_cli_command(
            &["delete", &fic_id.to_string()],
            &test_db.db_path,
            &mock_server.base_url(),
            None,
        );

        // Then - Check if delete was successful
        assertions::then_command_succeeded(delete_status, &delete_stderr, None, None);

        // Then - Verify database is empty using direct access
        assertions::then_fanfiction_was_deleted(&test_db.conn, fic_id)?;

        Ok(())
    }

    #[test]
    fn test_shelves_full_workflow() -> Result<(), Box<dyn Error>> {
        use ficflow::domain::shelf::ShelfOps;
        use ficflow::infrastructure::persistence::repository::SqliteRepository;

        // Given: a fic in the library
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();
        let db_path = &test_db.db_path;

        let (_, add_err, add_status) =
            run_cli_command(&["add", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(add_status, &add_err, None, None);

        // When: create a shelf
        let (create_out, create_err, create_status) =
            run_cli_command(&["shelf", "create", "Comfort reads"], db_path, &base, None);
        assertions::then_command_succeeded(
            create_status,
            &create_err,
            Some(&["Comfort reads", "id: 1"]),
            Some(&create_out),
        );

        // And: shelf list shows it
        let (list_out, list_err, list_status) =
            run_cli_command(&["shelf", "list"], db_path, &base, None);
        assertions::then_command_succeeded(
            list_status,
            &list_err,
            Some(&["Comfort reads"]),
            Some(&list_out),
        );

        // When: add the fic to the shelf (first shelf → id=1)
        let shelf_id = "1";
        let (_, add_shelf_err, add_shelf_status) = run_cli_command(
            &["shelf", "add", &fic_id.to_string(), shelf_id],
            db_path,
            &base,
            None,
        );
        assertions::then_command_succeeded(add_shelf_status, &add_shelf_err, None, None);

        // Then: shelf show lists the fic
        let (show_out, show_err, show_status) =
            run_cli_command(&["shelf", "show", shelf_id], db_path, &base, None);
        assertions::then_command_succeeded(
            show_status,
            &show_err,
            Some(&["Featherlight"]),
            Some(&show_out),
        );

        // And: library list still shows the fic (unaffected by shelf membership)
        let (lib_out, lib_err, lib_status) = run_cli_command(&["list"], db_path, &base, None);
        assertions::then_command_succeeded(
            lib_status,
            &lib_err,
            Some(&["Featherlight"]),
            Some(&lib_out),
        );

        // When: add the same fic again (idempotent — silent no-op)
        let (_, dup_err, dup_status) = run_cli_command(
            &["shelf", "add", &fic_id.to_string(), shelf_id],
            db_path,
            &base,
            None,
        );
        assertions::then_command_succeeded(dup_status, &dup_err, None, None);

        // Then: only one row in fic_shelf (not two)
        let shelf_ops = SqliteRepository::new(&test_db.conn);
        assert_eq!(shelf_ops.list_fics_in_shelf(1)?.len(), 1);

        // When: hard-delete the fic from the library
        let (_, del_err, del_status) =
            run_cli_command(&["delete", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(del_status, &del_err, None, None);

        // Then: FK cascade wiped the shelf membership
        let (show2_out, show2_err, show2_status) =
            run_cli_command(&["shelf", "show", shelf_id], db_path, &base, None);
        assertions::then_command_succeeded(
            show2_status,
            &show2_err,
            Some(&["No fanfictions"]),
            Some(&show2_out),
        );

        // When: delete the shelf itself
        let (_, shelf_del_err, shelf_del_status) =
            run_cli_command(&["shelf", "delete", shelf_id], db_path, &base, None);
        assertions::then_command_succeeded(shelf_del_status, &shelf_del_err, None, None);

        // Then: shelf list is empty
        let (final_out, final_err, final_status) =
            run_cli_command(&["shelf", "list"], db_path, &base, None);
        assertions::then_command_succeeded(
            final_status,
            &final_err,
            Some(&["No shelves"]),
            Some(&final_out),
        );

        Ok(())
    }

    #[test]
    fn test_shelf_create_rejects_empty_name() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, _) = fixtures::given_mock_ao3_server();

        let (_, stderr, status) = run_cli_command(
            &["shelf", "create", "   "],
            &test_db.db_path,
            &mock_server.base_url(),
            None,
        );

        assert_eq!(status, 1, "expected non-zero exit on validation error");
        assert!(
            stderr.contains("shelf name must not be empty"),
            "expected empty-name error, got stderr: {}",
            stderr
        );

        Ok(())
    }

    #[test]
    fn test_soft_delete_revive_resets_fanfiction() -> Result<(), Box<dyn Error>> {
        use ficflow::domain::fanfiction::{FanfictionOps, ReadingStatus};
        use ficflow::infrastructure::persistence::repository::SqliteRepository;

        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();
        let db_path = &test_db.db_path;

        // Given: fic added with user data
        let (_, add_err, add_status) =
            run_cli_command(&["add", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(add_status, &add_err, None, None);

        let (_, note_err, note_status) = run_cli_command(
            &["note", &fic_id.to_string(), "loved it"],
            db_path,
            &base,
            None,
        );
        assertions::then_command_succeeded(note_status, &note_err, None, None);

        let (_, rating_err, rating_status) =
            run_cli_command(&["rating", &fic_id.to_string(), "5"], db_path, &base, None);
        assertions::then_command_succeeded(rating_status, &rating_err, None, None);

        // When: soft-delete the fic
        let (_, del_err, del_status) =
            run_cli_command(&["delete", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(del_status, &del_err, None, None);

        // Then: get returns a NotFound error
        let (_, get_err, get_status) =
            run_cli_command(&["get", &fic_id.to_string()], db_path, &base, None);
        assert_ne!(
            get_status, 0,
            "expected `get` on soft-deleted fic to exit non-zero"
        );
        assert!(
            get_err.contains("not found"),
            "expected 'not found' in stderr, got: {}",
            get_err
        );

        // And: list is empty
        let (list_out, list_err, list_status) = run_cli_command(&["list"], db_path, &base, None);
        assertions::then_command_succeeded(list_status, &list_err, None, Some(&list_out));
        assert!(
            !list_out.contains("Featherlight"),
            "soft-deleted fic leaked into list output: {}",
            list_out
        );

        // When: re-add revives the fic
        let (_, readd_err, readd_status) =
            run_cli_command(&["add", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(readd_status, &readd_err, None, None);

        // Then: user data is reset to defaults
        let repo = SqliteRepository::new(&test_db.conn);
        let revived = repo.get_fanfiction_by_id(fic_id)?;
        assert_eq!(
            revived.personal_note, None,
            "personal_note should be wiped on revive"
        );
        assert_eq!(
            revived.user_rating, None,
            "user_rating should be wiped on revive"
        );
        assert_eq!(revived.read_count, 0, "read_count should be reset to 0");
        assert!(
            matches!(revived.reading_status, ReadingStatus::PlanToRead),
            "reading_status should be reset to PlanToRead, got {:?}",
            revived.reading_status
        );
        assert_eq!(
            revived.last_chapter_read, None,
            "last_chapter_read should be reset to None"
        );

        Ok(())
    }

    #[test]
    fn test_soft_delete_revive_drops_shelf_membership() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();
        let db_path = &test_db.db_path;

        // Given: fic on a shelf
        let (_, add_err, add_status) =
            run_cli_command(&["add", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(add_status, &add_err, None, None);
        let (_, sc_err, sc_status) =
            run_cli_command(&["shelf", "create", "Favorites"], db_path, &base, None);
        assertions::then_command_succeeded(sc_status, &sc_err, None, None);
        let (_, sa_err, sa_status) = run_cli_command(
            &["shelf", "add", &fic_id.to_string(), "1"],
            db_path,
            &base,
            None,
        );
        assertions::then_command_succeeded(sa_status, &sa_err, None, None);

        // When: delete fic (soft) then re-add (revive)
        let (_, del_err, del_status) =
            run_cli_command(&["delete", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(del_status, &del_err, None, None);
        let (_, readd_err, readd_status) =
            run_cli_command(&["add", &fic_id.to_string()], db_path, &base, None);
        assertions::then_command_succeeded(readd_status, &readd_err, None, None);

        // Then: revived fic is NOT on its old shelf (fic_shelf row was hard-deleted on revive)
        let (show_out, show_err, show_status) =
            run_cli_command(&["shelf", "show", "1"], db_path, &base, None);
        assertions::then_command_succeeded(
            show_status,
            &show_err,
            Some(&["No fanfictions"]),
            Some(&show_out),
        );

        Ok(())
    }

    #[test]
    fn test_soft_delete_shelf_flow() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, _) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();
        let db_path = &test_db.db_path;

        // Given: a shelf
        let (_, sc_err, sc_status) =
            run_cli_command(&["shelf", "create", "Favorites"], db_path, &base, None);
        assertions::then_command_succeeded(sc_status, &sc_err, None, None);

        // When: soft-delete the shelf
        let (_, del_err, del_status) =
            run_cli_command(&["shelf", "delete", "1"], db_path, &base, None);
        assertions::then_command_succeeded(del_status, &del_err, None, None);

        // Then: list is empty
        let (list_out, list_err, list_status) =
            run_cli_command(&["shelf", "list"], db_path, &base, None);
        assertions::then_command_succeeded(
            list_status,
            &list_err,
            Some(&["No shelves"]),
            Some(&list_out),
        );

        // And: show errors
        let (_, show_err, show_status) =
            run_cli_command(&["shelf", "show", "1"], db_path, &base, None);
        assert_ne!(
            show_status, 0,
            "expected `shelf show` on soft-deleted shelf to exit non-zero"
        );
        assert!(
            show_err.contains("not found"),
            "expected 'not found' in stderr, got: {}",
            show_err
        );

        // And: re-deleting reports the same ShelfNotFound
        let (_, del2_err, del2_status) =
            run_cli_command(&["shelf", "delete", "1"], db_path, &base, None);
        assert_ne!(
            del2_status, 0,
            "expected re-delete of soft-deleted shelf to exit non-zero"
        );
        assert!(
            del2_err.contains("not found"),
            "expected 'not found' in stderr, got: {}",
            del2_err
        );

        Ok(())
    }

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

    #[test]
    fn test_add_get_wipe_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();

        // When - Add fanfiction via CLI
        let (_add_stdout, add_stderr, add_status) = run_cli_command(
            &["add", &fic_id.to_string()],
            &test_db.db_path,
            &mock_server.base_url(),
            None,
        );

        // Then - Check if add was successful
        assertions::then_command_succeeded(add_status, &add_stderr, None, None);

        // When - Get the fanfiction details via CLI
        let (get_stdout, get_stderr, get_status) = run_cli_command(
            &["get", &fic_id.to_string()],
            &test_db.db_path,
            &mock_server.base_url(),
            None,
        );

        // Then - Check if get was successful and verify details
        let expected_strings = &["Featherlight", "Gummy_bean", "Hazbin Hotel"];
        assertions::then_command_succeeded(
            get_status,
            &get_stderr,
            Some(expected_strings),
            Some(&get_stdout),
        );

        // When - Wipe database via CLI
        let (_wipe_stdout, wipe_stderr, wipe_status) = run_cli_command(
            &["wipe"],
            &test_db.db_path,
            &mock_server.base_url(),
            Some(("FICFLOW_NON_INTERACTIVE", "1")),
        );

        // Then - Check if wipe was successful
        assertions::then_command_succeeded(wipe_status, &wipe_stderr, None, None);

        // Then - Verify database is empty using direct access
        assertions::then_database_was_wiped(&test_db.conn)?;

        Ok(())
    }
}
