//! Configuration Management Tests for CLI
//!
//! Tests the configuration handling functionality in src/cli/app.rs
//! Following step 1.2 from the testing-plan.md

use anyhow::Result;
use mate::cli::app::Config;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;

// Use a global mutex to serialize tests that modify environment variables
// This prevents parallel tests from interfering with each other
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Helper function to create a temporary config directory without environment variables
fn setup_test_dirs(temp_dir: &TempDir) -> (PathBuf, PathBuf) {
    let config_dir = temp_dir.path().join("config");
    let data_dir = temp_dir.path().join("data");
    (config_dir, data_dir)
}

/// Helper function to safely set environment variables for a test
fn with_env_vars<F, R>(config_dir: &PathBuf, data_dir: &PathBuf, test_fn: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = ENV_MUTEX.lock().unwrap();

    // Store original values
    let original_config_dir = std::env::var("MATE_CONFIG_DIR").ok();
    let original_data_dir = std::env::var("MATE_DATA_DIR").ok();

    // Set test values
    std::env::set_var("MATE_CONFIG_DIR", config_dir);
    std::env::set_var("MATE_DATA_DIR", data_dir);

    // Run the test
    let result = test_fn();

    // Restore original values
    match original_config_dir {
        Some(original) => std::env::set_var("MATE_CONFIG_DIR", original),
        None => std::env::remove_var("MATE_CONFIG_DIR"),
    }
    match original_data_dir {
        Some(original) => std::env::set_var("MATE_DATA_DIR", original),
        None => std::env::remove_var("MATE_DATA_DIR"),
    }

    result
}

/// Helper function to create a valid TOML configuration file
fn create_valid_config_toml(config_dir: &PathBuf) -> Result<PathBuf> {
    fs::create_dir_all(config_dir)?;
    let config_file = config_dir.join("config.toml");

    let toml_content = r#"
data_dir = "/custom/data/path"
default_bind_addr = "192.168.1.100:9090"
max_concurrent_games = 5
"#;

    fs::write(&config_file, toml_content)?;
    Ok(config_file)
}

/// Helper function to create a corrupted TOML file
fn create_corrupted_config_toml(config_dir: &PathBuf) -> Result<PathBuf> {
    fs::create_dir_all(config_dir)?;
    let config_file = config_dir.join("config.toml");

    // Write binary data that's not valid UTF-8
    fs::write(&config_file, &[0xFF, 0xFE, 0xFD, 0xFC])?;
    Ok(config_file)
}

/// Helper function to create a malformed TOML file
fn create_malformed_config_toml(config_dir: &PathBuf) -> Result<PathBuf> {
    fs::create_dir_all(config_dir)?;
    let config_file = config_dir.join("config.toml");

    let malformed_toml = r#"
data_dir = "/some/path"
default_bind_addr = "incomplete_address
max_concurrent_games = not_a_number
extra_bracket = ]
"#;

    fs::write(&config_file, malformed_toml)?;
    Ok(config_file)
}

/// Helper function to create a TOML file with missing required fields
fn create_incomplete_config_toml(config_dir: &PathBuf) -> Result<PathBuf> {
    fs::create_dir_all(config_dir)?;
    let config_file = config_dir.join("config.toml");

    // Only include some fields, missing others
    let incomplete_toml = r#"
default_bind_addr = "127.0.0.1:8080"
# missing data_dir and max_concurrent_games
"#;

    fs::write(&config_file, incomplete_toml)?;
    Ok(config_file)
}

#[test]
fn test_config_default_creates_sensible_values() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (_, data_dir) = setup_test_dirs(&temp_dir);

    // Test with controlled environment
    with_env_vars(&temp_dir.path().join("config"), &data_dir, || {
        let config = Config::default();

        // Check that default values are sensible
        assert_eq!(config.default_bind_addr, "127.0.0.1:8080");
        assert_eq!(config.max_concurrent_games, 10);

        // Data directory should be set to the environment override or reasonable default
        assert!(!config.data_dir.as_os_str().is_empty());
        assert_eq!(config.data_dir, data_dir);
    });
}

#[test]
fn test_config_database_path_resolution() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (_, data_dir) = setup_test_dirs(&temp_dir);

    let config = Config {
        data_dir: data_dir.clone(),
        default_bind_addr: "127.0.0.1:8080".to_string(),
        max_concurrent_games: 10,
    };

    let db_path = config.database_path();
    let expected_path = data_dir.join("database.sqlite");

    assert_eq!(db_path, expected_path);
    assert_eq!(db_path.file_name().unwrap(), "database.sqlite");
}

#[test]
fn test_config_loading_from_existing_toml() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (config_dir, data_dir) = setup_test_dirs(&temp_dir);

    // Create a valid TOML configuration file
    create_valid_config_toml(&config_dir).expect("Failed to create test config");

    // Load configuration with controlled environment
    with_env_vars(&config_dir, &data_dir, || {
        let result = Config::load_or_create_default();
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.data_dir, PathBuf::from("/custom/data/path"));
        assert_eq!(config.default_bind_addr, "192.168.1.100:9090");
        assert_eq!(config.max_concurrent_games, 5);
    });
}

#[test]
fn test_config_creation_with_defaults_when_missing() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (config_dir, data_dir) = setup_test_dirs(&temp_dir);

    // Ensure config file doesn't exist
    let config_file = config_dir.join("config.toml");
    assert!(!config_file.exists());

    // Load configuration (should create default) with controlled environment
    with_env_vars(&config_dir, &data_dir, || {
        let result = Config::load_or_create_default();
        assert!(result.is_ok());

        let config = result.unwrap();

        // Should have default values
        assert_eq!(config.default_bind_addr, "127.0.0.1:8080");
        assert_eq!(config.max_concurrent_games, 10);

        // Should have created the config file
        assert!(config_file.exists());

        // Verify the saved file can be loaded again
        let content = fs::read_to_string(&config_file).expect("Failed to read saved config");
        let parsed_config: Config = toml::from_str(&content).expect("Failed to parse saved config");

        assert_eq!(config.default_bind_addr, parsed_config.default_bind_addr);
        assert_eq!(
            config.max_concurrent_games,
            parsed_config.max_concurrent_games
        );
    });
}

#[test]
fn test_config_persistence_across_app_restarts() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (config_dir, data_dir) = setup_test_dirs(&temp_dir);

    with_env_vars(&config_dir, &data_dir, || {
        // Create and save a custom configuration
        let original_config = Config {
            data_dir: data_dir.clone(),
            default_bind_addr: "10.0.0.1:3000".to_string(),
            max_concurrent_games: 15,
        };

        // Save the configuration
        let save_result = original_config.save();
        assert!(save_result.is_ok());

        // Simulate app restart by loading configuration again
        let reload_result = Config::load_or_create_default();
        assert!(reload_result.is_ok());

        let reloaded_config = reload_result.unwrap();

        // Verify persistence
        assert_eq!(original_config.data_dir, reloaded_config.data_dir);
        assert_eq!(
            original_config.default_bind_addr,
            reloaded_config.default_bind_addr
        );
        assert_eq!(
            original_config.max_concurrent_games,
            reloaded_config.max_concurrent_games
        );
    });
}

#[test]
fn test_config_handles_corrupted_toml_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (config_dir, data_dir) = setup_test_dirs(&temp_dir);

    // Create a corrupted TOML file
    create_corrupted_config_toml(&config_dir).expect("Failed to create corrupted config");

    // Try to load configuration with controlled environment
    with_env_vars(&config_dir, &data_dir, || {
        let result = Config::load_or_create_default();

        // Should return an error (not panic)
        assert!(result.is_err());

        // Check that we get an appropriate error type (not testing specific message)
        let err = result.unwrap_err();

        // The error should be related to reading/parsing the file
        // (Testing error type rather than specific error message string)
        let error_string = err.to_string().to_lowercase();
        assert!(
            error_string.contains("read")
                || error_string.contains("parse")
                || error_string.contains("utf")
                || error_string.contains("configuration")
        );
    });
}

#[test]
fn test_config_handles_malformed_toml_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (config_dir, data_dir) = setup_test_dirs(&temp_dir);

    // Create a malformed TOML file
    create_malformed_config_toml(&config_dir).expect("Failed to create malformed config");

    // Try to load configuration with controlled environment
    with_env_vars(&config_dir, &data_dir, || {
        let result = Config::load_or_create_default();

        // Should return an error
        assert!(result.is_err());

        // Check that we get an appropriate error type
        let err = result.unwrap_err();
        let error_string = err.to_string().to_lowercase();

        // Should be a parsing error
        assert!(
            error_string.contains("parse")
                || error_string.contains("toml")
                || error_string.contains("configuration")
        );
    });
}

#[test]
fn test_config_handles_missing_required_fields() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (config_dir, data_dir) = setup_test_dirs(&temp_dir);

    // Create a TOML file with missing required fields
    create_incomplete_config_toml(&config_dir).expect("Failed to create incomplete config");

    // Try to load configuration with controlled environment
    with_env_vars(&config_dir, &data_dir, || {
        let result = Config::load_or_create_default();

        // This might succeed with default values or fail - both are valid behaviors
        // If it succeeds, the missing fields should have reasonable defaults
        // If it fails, it should be a parsing/deserialization error

        match result {
            Ok(config) => {
                // If it loads successfully, check that we have sensible values
                assert!(!config.default_bind_addr.is_empty());
                assert!(config.max_concurrent_games > 0);
                assert!(!config.data_dir.as_os_str().is_empty());
            }
            Err(err) => {
                // If it fails, should be a deserialization error
                let error_string = err.to_string().to_lowercase();
                assert!(
                    error_string.contains("parse")
                        || error_string.contains("deserialize")
                        || error_string.contains("missing")
                        || error_string.contains("configuration")
                );
            }
        }
    });
}

#[test]
fn test_config_save_creates_directory_structure() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let (config_dir, data_dir) = setup_test_dirs(&temp_dir);

    with_env_vars(&config_dir, &data_dir, || {
        // Ensure config directory doesn't exist
        assert!(!config_dir.exists());

        let config = Config {
            data_dir: data_dir.clone(),
            default_bind_addr: "127.0.0.1:8080".to_string(),
            max_concurrent_games: 10,
        };

        // Save should create the directory structure
        let result = config.save();
        assert!(result.is_ok());

        // Check that directory and file were created
        assert!(config_dir.exists());
        assert!(config_dir.join("config.toml").exists());
    });
}

#[test]
fn test_config_directory_resolution_with_env_variables() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let custom_config_dir = temp_dir.path().join("custom_config");
    let custom_data_dir = temp_dir.path().join("custom_data");

    // Test environment variable override with controlled environment
    with_env_vars(&custom_config_dir, &custom_data_dir, || {
        let config_dir_result = Config::default_config_dir();
        let data_dir_result = Config::default_data_dir();

        assert!(config_dir_result.is_ok());
        assert!(data_dir_result.is_ok());

        assert_eq!(config_dir_result.unwrap(), custom_config_dir);
        assert_eq!(data_dir_result.unwrap(), custom_data_dir);
    });
}

#[test]
fn test_config_serialization_roundtrip() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let data_dir = temp_dir.path().join("test_data");

    let original_config = Config {
        data_dir,
        default_bind_addr: "0.0.0.0:9999".to_string(),
        max_concurrent_games: 42,
    };

    // Serialize to TOML
    let toml_string = toml::to_string(&original_config).expect("Failed to serialize config");

    // Deserialize back
    let deserialized_config: Config =
        toml::from_str(&toml_string).expect("Failed to deserialize config");

    // Verify roundtrip preservation
    assert_eq!(original_config.data_dir, deserialized_config.data_dir);
    assert_eq!(
        original_config.default_bind_addr,
        deserialized_config.default_bind_addr
    );
    assert_eq!(
        original_config.max_concurrent_games,
        deserialized_config.max_concurrent_games
    );
}
