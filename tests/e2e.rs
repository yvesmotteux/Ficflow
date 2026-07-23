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

    /// CLI parser accepts several aliases for each reading status —
    /// "plan", "ptr", "tbr" all map to PlanToRead; "finished" /
    /// "completed" map to Read; etc. The GUI uses the typed enum
    /// directly so it never exercises this parser; that means this
    /// is the only test that prevents an accidental alias removal
    /// from breaking users' shell scripts and aliases.
    #[test]
    fn test_cli_status_aliases() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();

        let (_, add_err, add_status) =
            run_cli_command(&["add", &fic_id.to_string()], &test_db.db_path, &base, None);
        assertions::then_command_succeeded(add_status, &add_err, None, None);

        // Each alias should round-trip without error. We don't assert
        // the canonical name in stdout — we only need to know the
        // parser accepted the input.
        for alias in [
            "inprogress",
            "read",
            "finished",
            "completed",
            "plantoread",
            "plan-to-read",
            "plan_to_read",
            "plan",
            "ptr",
            "tbr",
            "paused",
            "abandoned",
        ] {
            let (_, err, status) = run_cli_command(
                &["status", &fic_id.to_string(), alias],
                &test_db.db_path,
                &base,
                None,
            );
            assert_eq!(
                status, 0,
                "alias {:?} should be accepted; stderr was: {}",
                alias, err
            );
        }

        // Garbage strings should still be rejected.
        let (_, _, bad_status) = run_cli_command(
            &["status", &fic_id.to_string(), "definitelynotastatus"],
            &test_db.db_path,
            &base,
            None,
        );
        assert_ne!(bad_status, 0, "unknown status should be rejected");

        Ok(())
    }

    /// CLI parser accepts numeric forms ("1"-"5"), word forms
    /// ("one"-"five"), and several "remove the rating" sentinels
    /// ("0", "none", "clear", "remove"). Same shape as
    /// `test_cli_status_aliases` — the GUI passes typed values so this
    /// is the only test that prevents an alias removal from breaking
    /// users' shell scripts. Also exercises the rejection branch of
    /// `parse_user_rating`, which the GUI never reaches.
    #[test]
    fn test_cli_rating_aliases() -> Result<(), Box<dyn Error>> {
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        let base = mock_server.base_url();

        let (_, add_err, add_status) =
            run_cli_command(&["add", &fic_id.to_string()], &test_db.db_path, &base, None);
        assertions::then_command_succeeded(add_status, &add_err, None, None);

        for alias in [
            "1", "2", "3", "4", "5", "one", "two", "three", "four", "five", "0", "none", "clear",
            "remove",
        ] {
            let (_, err, status) = run_cli_command(
                &["rating", &fic_id.to_string(), alias],
                &test_db.db_path,
                &base,
                None,
            );
            assert_eq!(
                status, 0,
                "alias {:?} should be accepted; stderr was: {}",
                alias, err
            );
        }

        // Garbage strings should still be rejected.
        let (_, _, bad_status) = run_cli_command(
            &["rating", &fic_id.to_string(), "definitelynotarating"],
            &test_db.db_path,
            &base,
            None,
        );
        assert_ne!(bad_status, 0, "unknown rating should be rejected");

        Ok(())
    }

    fn binary_path() -> PathBuf {
        env::current_dir()
            .unwrap()
            .join("target")
            .join("debug")
            .join("ficflow")
    }

    fn write_config_with_library_path(config_home: &Path, library_path: &Path) {
        let cfg = ficflow::interfaces::gui::AppConfig {
            library_path: Some(library_path.to_path_buf()),
            ..ficflow::interfaces::gui::AppConfig::default()
        };
        let dir = config_home.join("ficflow");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            toml::to_string_pretty(&cfg).unwrap(),
        )
        .unwrap();
    }

    fn run_cli(args: &[&str], config_home: &Path, db_env: Option<&Path>) -> (String, i32) {
        let mut cmd = Command::new(binary_path());
        cmd.env("XDG_CONFIG_HOME", config_home)
            .env_remove("FICFLOW_DB_PATH");
        if let Some(path) = db_env {
            cmd.env("FICFLOW_DB_PATH", path);
        }
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd.output().expect("Failed to execute command");
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        (stdout, output.status.code().unwrap_or(0))
    }

    #[test]
    fn cli_reads_and_writes_the_configured_library_path() -> Result<(), Box<dyn Error>> {
        let config_home = TempDir::new()?;
        let lib_dir = TempDir::new()?;
        let configured_db = lib_dir.path().join("my-library.db");
        write_config_with_library_path(config_home.path(), &configured_db);

        let (_, create_status) =
            run_cli(&["shelf", "create", "Configured"], config_home.path(), None);
        assert_eq!(create_status, 0);
        assert!(
            configured_db.exists(),
            "CLI should create the DB at the configured path"
        );

        let (list_out, _) = run_cli(&["shelf", "list"], config_home.path(), None);
        assert!(
            list_out.contains("Configured"),
            "CLI should read back from the configured library; got: {}",
            list_out
        );
        Ok(())
    }

    #[test]
    fn ficflow_db_path_env_overrides_the_configured_library_path() -> Result<(), Box<dyn Error>> {
        let config_home = TempDir::new()?;
        let config_lib = TempDir::new()?;
        let configured_db = config_lib.path().join("configured.db");
        write_config_with_library_path(config_home.path(), &configured_db);

        let env_lib = TempDir::new()?;
        let env_db = env_lib.path().join("env.db");

        let (_, status) = run_cli(
            &["shelf", "create", "FromEnv"],
            config_home.path(),
            Some(&env_db),
        );
        assert_eq!(status, 0);
        assert!(
            env_db.exists(),
            "env override should decide the library path"
        );
        assert!(
            !configured_db.exists(),
            "configured path must be ignored when FICFLOW_DB_PATH is set"
        );
        Ok(())
    }
}
