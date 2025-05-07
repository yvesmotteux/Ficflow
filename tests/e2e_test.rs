use std::fs;
use std::process::Command;
use httpmock::{MockServer, Method::GET};
use rusqlite::Connection;
use std::error::Error;
use tempfile::TempDir;
use std::env;
use std::path::PathBuf;

use ficflow::{
    domain::config,
    infrastructure::{
        db::get_all_fanfictions,
        migration::run_migrations
    }
};

#[cfg(test)]
mod tests {
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
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        
        // Create the database connection
        let mut conn = Connection::open(&db_path).expect("Failed to open database");
        run_migrations(&mut conn).expect("Failed to run migrations");
        
        TestDatabase {
            conn,
            db_path,
            _temp_dir: temp_dir,
        }
    }
    
    // Helper function to set up a mock AO3 server
    fn setup_mock_ao3_server() -> (MockServer, u64) {
        let mock_server = MockServer::start();
        let fic_id = 53960491;
        
        // Given
        let html_content = fs::read_to_string("tests/fixtures/ao3_fic_example1.html")
            .expect("Failed to read mock HTML file");
            
        let _mock = mock_server.mock(|when, then| {
            when.method(GET).path(format!("/works/{}", fic_id));
            then.status(200).body(html_content);
        });
        
        (mock_server, fic_id)
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
        // Given
        let test_db = setup_test_db();
        let (mock_server, fic_id) = setup_mock_ao3_server();
        
        // Save original base URL
        let original_url = config::get_ao3_base_url();
        
        // When - Add fanfiction via CLI
        let (_add_stdout, add_stderr, add_status) = run_cli_command(
            &["add", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if add was successful
        assert_eq!(add_status, 0, "Command failed with stderr: {}", add_stderr);
        
        // When - List fanfictions via CLI
        let (list_stdout, list_stderr, list_status) = run_cli_command(
            &["list"], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if list shows our fanfiction
        assert_eq!(list_status, 0, "Command failed with stderr: {}", list_stderr);
        assert!(list_stdout.contains("Featherlight"), 
                "Expected to find fanfiction 'Featherlight' in list, got: {}", list_stdout);
        assert!(list_stdout.contains(&fic_id.to_string()), 
                "Expected to find ID {} in list, got: {}", fic_id, list_stdout);
        
        // When - Delete fanfiction via CLI
        let (_delete_stdout, delete_stderr, delete_status) = run_cli_command(
            &["delete", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if delete was successful
        assert_eq!(delete_status, 0, "Command failed with stderr: {}", delete_stderr);
        
        // Verify database is empty using direct access
        let remaining_fics = get_all_fanfictions(&test_db.conn)?;
        assert_eq!(remaining_fics.len(), 0, "Expected no fanfictions in the database after deletion");
        
        // Cleanup
        config::set_ao3_base_url(&original_url);
        
        Ok(())
    }
    
    #[test]
    fn test_add_get_wipe_fanfiction() -> Result<(), Box<dyn Error>> {
        // Given
        let test_db = setup_test_db();
        let (mock_server, fic_id) = setup_mock_ao3_server();
        
        // Save original base URL
        let original_url = config::get_ao3_base_url();
        
        // When - Add fanfiction via CLI
        let (_add_stdout, add_stderr, add_status) = run_cli_command(
            &["add", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if add was successful
        assert_eq!(add_status, 0, "Command failed with stderr: {}", add_stderr);
        
        // When - Get the fanfiction details via CLI
        let (get_stdout, get_stderr, get_status) = run_cli_command(
            &["get", &fic_id.to_string()], 
            &test_db.db_path, 
            &mock_server.base_url(),
            None
        );
        
        // Then - Check if get was successful and verify details
        assert_eq!(get_status, 0, "Command failed with stderr: {}", get_stderr);
        assert!(get_stdout.contains("Featherlight"), 
                "Expected to find title 'Featherlight', got: {}", get_stdout);
        assert!(get_stdout.contains("Gummy_bean"), 
                "Expected to find author 'Gummy_bean', got: {}", get_stdout);
        assert!(get_stdout.contains("Hazbin Hotel"), 
                "Expected to find fandom 'Hazbin Hotel', got: {}", get_stdout);
        
        // When - Wipe database via CLI
        let (_wipe_stdout, wipe_stderr, wipe_status) = run_cli_command(
            &["wipe"], 
            &test_db.db_path, 
            &mock_server.base_url(),
            Some(("FICFLOW_NON_INTERACTIVE", "1"))
        );
        
        // Then - Check if wipe was successful
        assert_eq!(wipe_status, 0, "Command failed with stderr: {}", wipe_stderr);
        
        // Verify database is empty using direct access
        let remaining_fics = get_all_fanfictions(&test_db.conn)?;
        assert_eq!(remaining_fics.len(), 0, "Expected no fanfictions in the database after wipe");
        
        // Cleanup
        config::set_ao3_base_url(&original_url);
        
        Ok(())
    }
}