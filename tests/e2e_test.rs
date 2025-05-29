use std::process::Command;
use std::error::Error;
use std::env;
use std::path::PathBuf;

use ficflow::domain::url_config;

#[path = "common/mod.rs"]
mod common;
use common::{fixtures, assertions};

/// Helper function to check if running in CI environment
fn is_ci_environment() -> bool {
    std::env::var("CI").is_ok()
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;
    use tempfile::TempDir;
    use super::*;
    
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
    fn run_cli_command(args: &[&str], db_path: &PathBuf, mock_url: &str, additional_env: Option<(&str, &str)>) -> (String, String, i32) {
        let binary_path = env::current_dir().unwrap()
            .join("target")
            .join("debug")
            .join("ficflow");
            
        // Make sure the binary exists
        assert!(binary_path.exists(), "Binary not found at {:?}. Run `cargo build` first.", binary_path);
        
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
        // Skip this test if running in CI environment
        if is_ci_environment() {
            println!("Skipping test_add_list_remove_fanfiction in CI environment");
            return Ok(());
        }
        
        // Given
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        
        // Save original base URL
        let original_url = url_config::get_ao3_base_url();
        
        // When - Add fanfiction via CLI
        let (_add_stdout, add_stderr, add_status) = run_cli_command(
            &["add", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if add was successful
        assertions::then_command_succeeded(add_status, &add_stderr, None, None);
        
        // When - List fanfictions via CLI
        let (list_stdout, list_stderr, list_status) = run_cli_command(
            &["list"], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if list shows our fanfiction
        let expected_strings = &["Featherlight", &fic_id.to_string()];
        assertions::then_command_succeeded(list_status, &list_stderr, Some(expected_strings), Some(&list_stdout));
        
        // When - Delete fanfiction via CLI
        let (_delete_stdout, delete_stderr, delete_status) = run_cli_command(
            &["delete", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if delete was successful
        assertions::then_command_succeeded(delete_status, &delete_stderr, None, None);
        
        // Then - Verify database is empty using direct access
        assertions::then_fanfiction_was_deleted(&test_db.conn, fic_id)?;
        
        // Cleanup
        url_config::set_ao3_base_url(&original_url);
        
        Ok(())
    }
    
    #[test]
    fn test_add_get_wipe_fanfiction() -> Result<(), Box<dyn Error>> {
        // Skip this test if running in CI environment
        if is_ci_environment() {
            println!("Skipping test_add_get_wipe_fanfiction in CI environment");
            return Ok(());
        }
        
        // Given
        let test_db = setup_test_db();
        let (mock_server, fic_id) = fixtures::given_mock_ao3_server();
        
        // Save original base URL
        let original_url = url_config::get_ao3_base_url();
        
        // When - Add fanfiction via CLI
        let (_add_stdout, add_stderr, add_status) = run_cli_command(
            &["add", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if add was successful
        assertions::then_command_succeeded(add_status, &add_stderr, None, None);
        
        // When - Get the fanfiction details via CLI
        let (get_stdout, get_stderr, get_status) = run_cli_command(
            &["get", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if get was successful and verify details
        let expected_strings = &["Featherlight", "Gummy_bean", "Hazbin Hotel"];
        assertions::then_command_succeeded(get_status, &get_stderr, Some(expected_strings), Some(&get_stdout));
        
        // When - Wipe database via CLI
        let (_wipe_stdout, wipe_stderr, wipe_status) = run_cli_command(
            &["wipe"], 
            &test_db.db_path, 
            &mock_server.base_url(),
            Some(("FICFLOW_NON_INTERACTIVE", "1"))
        );
        
        // Then - Check if wipe was successful
        assertions::then_command_succeeded(wipe_status, &wipe_stderr, None, None);
        
        // Then - Verify database is empty using direct access
        assertions::then_database_was_wiped(&test_db.conn)?;
        
        // Cleanup
        url_config::set_ao3_base_url(&original_url);
        
        Ok(())
    }
}