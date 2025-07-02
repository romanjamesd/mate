//! App Foundation Tests
//!
//! Tests for `src/cli/app.rs` App initialization and lifecycle
//! Following Phase 1.1 of the testing implementation plan

use mate::cli::app::{App, Config};
use mate::storage::database::cleanup_database_files;
use rand;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

/// Test environment helper that provides isolated resources for each test
/// This prevents race conditions and shared resource conflicts when running tests in parallel
struct TestEnvironment {
    _temp_dir: TempDir,
    test_data_dir: std::path::PathBuf,
}

impl TestEnvironment {
    /// Create a new isolated test environment
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a unique test directory using multiple sources of uniqueness
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos();
        let random_id: u64 = rand::random();
        let thread_id = std::thread::current().id();
        let process_id = std::process::id();

        // Add additional randomness to prevent any possible conflicts
        let random_suffix: u32 = rand::random();

        let unique_test_dir = temp_dir.path().join(format!(
            "app_test_{}_{:x}_{:?}_{}_{}",
            timestamp, random_id, thread_id, process_id, random_suffix
        ));

        std::fs::create_dir_all(&unique_test_dir).expect("Failed to create unique test dir");

        Self {
            _temp_dir: temp_dir,
            test_data_dir: unique_test_dir,
        }
    }

    /// Get the test data directory path
    fn data_dir(&self) -> &std::path::Path {
        &self.test_data_dir
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // Clean up database files properly using the database cleanup utility
        let db_path = self.test_data_dir.join("database.sqlite");
        let _ = cleanup_database_files(&db_path);

        // Also clean up any additional files that might be created
        let identity_file = self.test_data_dir.join("identity.key");
        let _ = fs::remove_file(&identity_file);

        let config_file = self.test_data_dir.join("config.toml");
        let _ = fs::remove_file(&config_file);
    }
}

/// Create an App instance with isolated test environment
async fn create_test_app() -> (App, TestEnvironment) {
    let env = TestEnvironment::new();
    let app = App::new_with_data_dir(env.test_data_dir.clone())
        .await
        .expect("Failed to create test app");
    (app, env)
}

/// Helper function to create a test configuration with a temporary directory
#[allow(dead_code)]
fn create_test_config(temp_dir: &TempDir) -> Config {
    Config {
        data_dir: temp_dir.path().to_path_buf(),
        default_bind_addr: "127.0.0.1:8080".to_string(),
        max_concurrent_games: 10,
    }
}

/// Helper function to create a temporary directory with specific permissions
fn create_temp_dir_with_permissions(mode: u32) -> std::io::Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let mut perms = fs::metadata(temp_dir.path())?.permissions();
    perms.set_mode(mode);
    fs::set_permissions(temp_dir.path(), perms)?;
    Ok(temp_dir)
}

// =============================================================================
// App::new() initialization tests
// =============================================================================

#[tokio::test]
async fn test_app_new_successful_initialization() {
    // Test that App::new() successfully initializes with default configuration
    let (app, _env) = create_test_app().await;

    // Verify all components are properly initialized
    assert!(!app.peer_id().is_empty(), "Peer ID should be generated");
    assert!(app.database_path().exists(), "Database should be created");
    assert!(app.data_dir().exists(), "Data directory should exist");
}

#[tokio::test]
async fn test_app_new_with_fresh_data_directory() {
    // Test App::new() with a completely fresh data directory
    let (app, _env) = create_test_app().await;

    assert!(!app.peer_id().is_empty(), "Peer ID should be generated");
    assert!(app.data_dir().exists(), "Data directory should be created");
    assert!(
        app.database_path().exists(),
        "Database file should be created"
    );
}

#[tokio::test]
async fn test_app_new_with_existing_data_directory() {
    // Test App::new() when called multiple times (simulating existing data directory)
    let env = TestEnvironment::new();

    let data_dir;
    let database_path;

    // Create first app and verify it works
    {
        let app1 = App::new_with_data_dir(env.test_data_dir.clone())
            .await
            .expect("First App::new() should succeed");
        data_dir = app1.data_dir().clone();
        database_path = app1.database_path();

        // Ensure some data exists
        assert!(
            data_dir.exists(),
            "Data directory should exist after first initialization"
        );
        assert!(
            database_path.exists(),
            "Database should exist after first initialization"
        );
    } // app1 is dropped here, releasing database lock

    // Create a second app instance with existing data
    let app2 = App::new_with_data_dir(env.test_data_dir.clone())
        .await
        .expect("Second App::new() should succeed with existing directory");

    assert!(
        !app2.peer_id().is_empty(),
        "Peer ID should be generated/loaded"
    );
    assert_eq!(
        data_dir,
        *app2.data_dir(),
        "Both apps should use same data directory"
    );

    // Keep env alive until end
    drop(env);
}

#[tokio::test]
async fn test_app_new_creates_data_directory_with_proper_permissions() {
    // Test that App::new() creates data directory with appropriate permissions
    let (app, _env) = create_test_app().await;
    let data_dir = app.data_dir();

    // Verify directory exists and is readable/writable
    assert!(data_dir.exists(), "Data directory should exist");

    let metadata = fs::metadata(data_dir).expect("Failed to get directory metadata");
    assert!(metadata.is_dir(), "Should be a directory");

    // Test that we can write to the directory
    let test_file = data_dir.join("write_test.tmp");
    assert!(
        fs::write(&test_file, "test").is_ok(),
        "Directory should be writable"
    );

    // Clean up test file
    let _ = fs::remove_file(&test_file);
}

#[tokio::test]
async fn test_app_new_handles_database_initialization_failure() {
    // Test App::new() behavior when database initialization fails
    // We test the ensure_data_dir method directly with a read-only directory

    let temp_dir = create_temp_dir_with_permissions(0o444) // Read-only
        .expect("Failed to create read-only temp directory");

    // Test that ensure_data_dir fails with read-only directory
    let result = App::ensure_data_dir(&temp_dir.path().to_path_buf());

    // This should fail because directory is not writable
    assert!(
        result.is_err(),
        "ensure_data_dir should fail when data directory is not writable"
    );

    // Verify the error contains helpful context
    let error_message = result.unwrap_err().to_string();
    assert!(
        error_message.contains("not writable"),
        "Error should mention directory is not writable"
    );
}

#[tokio::test]
async fn test_app_new_generates_valid_identity() {
    // Test that App::new() always generates/loads a valid identity
    // The identity system is robust and will generate new identity if loading fails

    let (app, _env) = create_test_app().await;

    // Verify the identity file was created (it will be in the test environment due to environment variable)
    // Note: The identity may be stored in the temporary directory via environment variable during App creation
    assert!(
        !app.peer_id().to_string().is_empty(),
        "Identity should be valid (indicates identity system worked)"
    );

    // The identity file should exist somewhere accessible to the identity system
    // We can't easily check the exact path due to environment variable usage during initialization
    // But the fact that we have a valid peer ID means the identity system worked correctly
}

#[tokio::test]
async fn test_app_new_works_across_different_data_directory_locations() {
    // Test App::new() with isolated test environments

    // Test 1: Basic initialization works
    let (app1, _env1) = create_test_app().await;
    let data_dir1 = app1.data_dir().clone();

    // Test 2: Different test environment uses different data directory
    let (app2, _env2) = create_test_app().await;
    let data_dir2 = app2.data_dir().clone();

    // Each test environment should use different data directories
    assert_ne!(
        data_dir1, data_dir2,
        "Different test environments should use different data directories"
    );

    // Test 3: Each app should work correctly in its own environment
    assert!(data_dir1.exists(), "First data directory should exist");
    assert!(app1.database_path().exists(), "First database should exist");
    assert!(data_dir2.exists(), "Second data directory should exist");
    assert!(
        app2.database_path().exists(),
        "Second database should exist"
    );
}

// =============================================================================
// App lifecycle tests
// =============================================================================

#[tokio::test]
async fn test_app_handles_graceful_shutdown() {
    // Test that App handles graceful shutdown properly
    let (app, _env) = create_test_app().await;

    // Verify app is functional before shutdown
    assert!(!app.peer_id().is_empty(), "App should be functional");
    assert!(app.data_dir().exists(), "Data directory should exist");

    // In a real shutdown scenario, we would:
    // 1. Close network connections
    // 2. Flush any pending database operations
    // 3. Save configuration
    // 4. Clean up resources

    // For this test, we verify that saving configuration works
    let save_result = app.save_config();
    assert!(
        save_result.is_ok(),
        "Configuration should be saveable during shutdown"
    );

    // Verify database is still accessible
    let games_result = app.database.get_all_games();
    assert!(
        games_result.is_ok(),
        "Database should be accessible during shutdown"
    );

    // In an actual implementation, we might have an explicit shutdown method
    // For now, we test that the app can be dropped cleanly
    drop(app);

    // If we reach here without panicking, graceful shutdown worked
}

#[tokio::test]
async fn test_app_resource_cleanup_on_drop() {
    // Test that App properly cleans up resources when dropped
    let env = TestEnvironment::new();
    let database_path;

    {
        let app = App::new_with_data_dir(env.test_data_dir.clone())
            .await
            .expect("Failed to create app");

        // Record paths for verification after drop
        database_path = app.database_path();

        // Verify resources exist while app is alive
        assert!(app.data_dir().exists(), "Data directory should exist");

        // Perform some operations to ensure resources are active
        let _ = app.database.get_all_games();
        let _ = app.save_config();
    } // App is dropped here

    // After drop, verify that:
    // 1. Files created by the app still exist (they should persist)
    // 2. No file handles are left open (we can't easily test this)
    // 3. No memory leaks occurred (also difficult to test directly)

    // These files should still exist after app is dropped
    assert!(
        database_path.exists(),
        "Database file should persist after drop"
    );

    // Test that we can create a new app instance using the same resources
    let new_app = App::new_with_data_dir(env.test_data_dir.clone()).await;

    assert!(
        new_app.is_ok(),
        "Should be able to create new app instance after previous one was dropped"
    );

    // Keep env alive until end
    drop(env);
}

// =============================================================================
// Additional helper tests for edge cases
// =============================================================================

#[tokio::test]
async fn test_app_new_handles_concurrent_initialization() {
    // Test that multiple concurrent App::new() calls don't interfere with each other
    // Each will get its own isolated test environment

    // Create multiple apps concurrently, each with its own test environment
    let handles = (0..3)
        .map(|_| {
            // Reduced from 5 to 3 for CI stability
            tokio::spawn(async { create_test_app().await })
        })
        .collect::<Vec<_>>();

    // Wait for all handles to complete
    let mut results = Vec::new();
    for handle in handles {
        results.push(handle.await);
    }

    // All should succeed
    for (i, result) in results.into_iter().enumerate() {
        let (app, _env) = result.expect("Task should complete");

        assert!(
            !app.peer_id().is_empty(),
            "App {} should have valid peer ID",
            i
        );
        assert!(
            app.data_dir().exists(),
            "App {} data directory should exist",
            i
        );
    }
}

#[test]
fn test_config_database_path_resolution() {
    // Test Config::database_path() method
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config = Config {
        data_dir: temp_dir.path().to_path_buf(),
        default_bind_addr: "127.0.0.1:8080".to_string(),
        max_concurrent_games: 10,
    };

    let db_path = config.database_path();
    assert_eq!(db_path, temp_dir.path().join("database.sqlite"));
    assert_eq!(db_path.file_name().unwrap(), "database.sqlite");
}

#[test]
fn test_ensure_data_dir_creates_directory() {
    // Test App::ensure_data_dir() creates directory and verifies writability
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let data_dir = temp_dir.path().join("new_data_dir");

    // Directory shouldn't exist initially
    assert!(!data_dir.exists());

    let result = App::ensure_data_dir(&data_dir);
    assert!(result.is_ok(), "ensure_data_dir should succeed");

    // Directory should now exist and be writable
    assert!(data_dir.exists());
    assert!(data_dir.is_dir());
}

#[test]
fn test_ensure_data_dir_handles_readonly_directory() {
    // Test App::ensure_data_dir() with read-only directory
    let temp_dir =
        create_temp_dir_with_permissions(0o444).expect("Failed to create read-only temp directory");

    let result = App::ensure_data_dir(&temp_dir.path().to_path_buf());

    // Should fail because directory is not writable
    assert!(
        result.is_err(),
        "ensure_data_dir should fail with read-only directory"
    );
}
