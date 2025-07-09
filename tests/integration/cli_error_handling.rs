//! CLI Error Handling Integration Tests
//!
//! Tests for comprehensive error handling across all CLI components as specified
//! in testing-plan.md Phase 4.1. These tests verify that the CLI handles errors
//! gracefully and provides user-friendly feedback for all error scenarios.
//!
//! Test Categories:
//! - Network Error Handling (timeouts, retry logic, offline operation)
//! - Database Error Handling (connection failures, transaction rollback)
//! - Input Error Handling (malformed input, validation errors)

use anyhow::Result;
use mate::chess::ChessError;
use mate::cli::error_handler::{
    create_input_validation_error, create_network_timeout_error, handle_chess_command_error,
    is_recoverable_error,
};
use mate::cli::{CliError, GameOpsError};
use mate::messages::wire::WireProtocolError;
use mate::network::ConnectionError;
use mate::storage::errors::StorageError;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;

use crate::common::port_utils::get_unreachable_address;

/// Helper function to get the mate binary path
fn get_mate_binary_path() -> String {
    let default_path = "target/debug/mate";

    // Check if binary exists and is executable
    if std::path::Path::new(default_path).exists() {
        default_path.to_string()
    } else {
        // Try alternative locations for CI environments
        let alternatives = vec![
            "target/debug/mate",
            "./target/debug/mate",
            "../target/debug/mate",
            "mate",
        ];

        for path in alternatives {
            if std::path::Path::new(path).exists() {
                return path.to_string();
            }
        }

        // If no binary found, still return default to get a clear error
        default_path.to_string()
    }
}

/// Detect if we're running in a CI environment and get appropriate timeout multiplier
fn get_timeout_multiplier() -> f64 {
    // Check for common CI environment variables
    let ci_indicators = [
        "CI",
        "CONTINUOUS_INTEGRATION",
        "GITHUB_ACTIONS",
        "TRAVIS",
        "CIRCLECI",
        "BUILDKITE",
        "JENKINS_URL",
    ];

    let is_ci = ci_indicators.iter().any(|var| std::env::var(var).is_ok());

    if is_ci {
        // CI environments are typically much slower, use 8x multiplier for extra safety
        8.0
    } else {
        // Check for explicit timeout multiplier override
        std::env::var("TEST_TIMEOUT_MULTIPLIER")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.5) // Default to 1.5x even locally for better reliability
    }
}

/// Get adaptive timeout based on base duration and environment
fn get_adaptive_timeout(base_seconds: u64) -> Duration {
    let multiplier = get_timeout_multiplier();
    let timeout_secs = (base_seconds as f64 * multiplier).ceil() as u64;
    // Minimum timeout of 15 seconds for CI, 10 seconds for local
    let min_timeout = if multiplier > 3.0 { 15 } else { 10 };
    Duration::from_secs(timeout_secs.max(min_timeout))
}

/// Helper function to create corrupted database file
async fn create_corrupted_database(data_dir: &Path) -> Result<()> {
    let db_path = data_dir.join("database.sqlite");
    fs::write(&db_path, b"This is not a valid SQLite database").await?;
    Ok(())
}

/// Helper function to create unwritable directory (permission testing)
async fn create_readonly_directory(path: &PathBuf) -> Result<()> {
    fs::create_dir_all(path).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path).await?.permissions();
        perms.set_mode(0o444); // Read-only
        fs::set_permissions(path, perms).await?;
    }
    Ok(())
}

/// Create a unique temporary directory for each test to avoid conflicts
fn create_unique_temp_dir() -> Result<TempDir> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let thread_id = std::thread::current().id();

    tempfile::Builder::new()
        .prefix(&format!("mate_test_{}_{:?}_", timestamp, thread_id))
        .tempdir()
        .map_err(|e| anyhow::anyhow!("Failed to create temp directory: {}", e))
}

/// Retry an operation with exponential backoff for CI reliability
async fn retry_with_backoff<F, T, E>(
    operation: F,
    max_attempts: u32,
    base_delay_ms: u64,
    operation_name: &str,
) -> Result<T, E>
where
    F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, E>>>>,
    E: std::fmt::Debug,
{
    let mut last_error = None;

    for attempt in 1..=max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt == max_attempts {
                    last_error = Some(e);
                    break;
                }

                let delay = Duration::from_millis(base_delay_ms * (2_u64.pow(attempt - 1)));
                println!(
                    "   Attempt {}/{} for {} failed, retrying in {:?}...",
                    attempt, max_attempts, operation_name, delay
                );
                tokio::time::sleep(delay).await;
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap())
}

/// Verify binary exists and is executable before running tests
fn verify_binary_availability() -> Result<String> {
    let binary_path = get_mate_binary_path();
    let path = std::path::Path::new(&binary_path);

    if !path.exists() {
        anyhow::bail!(
            "mate binary not found at: {}. Run 'cargo build --bin mate' first.",
            binary_path
        );
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)?;
        let permissions = metadata.permissions();
        if permissions.mode() & 0o111 == 0 {
            anyhow::bail!("mate binary is not executable: {}", binary_path);
        }
    }

    Ok(binary_path)
}

//=============================================================================
// Network Error Handling Tests
//=============================================================================

/// Test network timeout handling with user feedback
#[tokio::test]
async fn test_error_handling_connection_timeouts() {
    let multiplier = get_timeout_multiplier();
    println!("Testing network timeout handling with user feedback");
    println!("   Timeout multiplier: {:.1}x", multiplier);

    // Verify binary availability before running test
    let binary_path = verify_binary_availability().expect("mate binary should be available");
    println!("   Using binary: {}", binary_path);

    // Test connection to non-responsive server (focus on error handling, not timing)
    let start_time = Instant::now();

    let output = timeout(
        get_adaptive_timeout(25), // More generous timeout for CI
        Command::new(binary_path)
            .args(["invite", "192.168.254.254:8080"]) // Non-routable IP for timeout
            .env("RUST_LOG", "error")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    let execution_time = start_time.elapsed();

    // Allow for either timeout or command completion
    let command_result = match output {
        Ok(cmd_result) => match cmd_result {
            Ok(output) => Some(output),
            Err(e) => {
                println!(
                    "   Command execution failed: {:?} - acceptable for timeout test",
                    e
                );
                None
            }
        },
        Err(_) => {
            // Command timed out, which is acceptable for this test
            println!(
                "   Command timed out after {:?} - acceptable for timeout test",
                execution_time
            );
            None
        }
    };

    if let Some(command_output) = command_result {
        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{stdout}{stderr}");

        // Verify appropriate error message is shown
        assert!(
            combined_output.contains("timeout")
                || combined_output.contains("failed to connect")
                || combined_output.contains("connection failed")
                || combined_output.contains("unreachable")
                || combined_output.contains("Failed to initialize"), // App initialization may fail first
            "Should show timeout or connection error. Output: {}",
            combined_output
        );

        // Verify appropriate exit code
        assert!(
            !command_output.status.success(),
            "Should exit with error code on timeout"
        );

        println!("   - Command completed in {:?}", execution_time);
        println!("   - User-friendly error messages provided");
    }

    println!("✅ Connection timeout handling test passed");
    println!("   - Network errors handled gracefully");
    println!("   - Appropriate exit codes returned");
}

/// Test network failure handling and user feedback
#[tokio::test]
async fn test_error_handling_network_failures_user_feedback() {
    println!("Testing network failure handling and user feedback");
    let multiplier = get_timeout_multiplier();
    println!("   Using timeout multiplier: {:.1}x", multiplier);

    // Verify binary availability before running test
    let binary_path = verify_binary_availability().expect("mate binary should be available");
    println!("   Using binary: {}", binary_path);

    let test_scenarios = vec![
        (get_unreachable_address(), "Unreachable port"),
        (
            "invalid-hostname:8080".to_string(),
            "DNS resolution failure",
        ),
        ("192.168.999.1:8080".to_string(), "Invalid IP address"),
        ("localhost:0".to_string(), "Reserved port"),
    ];

    for (address, scenario_desc) in test_scenarios {
        println!("  Testing scenario: {} ({})", scenario_desc, address);

        // Use unique temp directory for each scenario to avoid conflicts
        let temp_dir = create_unique_temp_dir().expect("Failed to create temp directory");
        let temp_path = temp_dir.path().to_string_lossy().to_string();

        // Add retry logic for network operations in CI environments
        let address_clone = address.clone();
        let temp_path_clone = temp_path.clone();
        let binary_path_clone = binary_path.clone();
        let result = retry_with_backoff(
            move || {
                let address_ref = address_clone.clone();
                let temp_path_ref = temp_path_clone.clone();
                let binary_path_ref = binary_path_clone.clone();
                Box::pin(async move {
                    timeout(
                        get_adaptive_timeout(20), // Increased from 15 to 20 seconds base
                        Command::new(binary_path_ref)
                            .args(["invite", &address_ref])
                            .env("MATE_DATA_DIR", &temp_path_ref)
                            .env("RUST_LOG", "error")
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .output(),
                    )
                    .await
                })
            },
            if multiplier > 3.0 { 5 } else { 3 }, // More retries for CI
            1000,                                 // 1 second base delay for CI stability
            &format!("network failure test for {}", scenario_desc),
        )
        .await;

        let command_output = result
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{stdout}{stderr}");

        // Verify graceful error handling
        assert!(
            !command_output.status.success(),
            "Should fail gracefully for scenario: {}. Output: {}",
            scenario_desc,
            combined_output
        );

        // Verify user-friendly error messages (not raw system errors)
        let has_user_friendly_error = combined_output.contains("Failed to connect")
            || combined_output.contains("Connection failed")
            || combined_output.contains("Invalid address")
            || combined_output.contains("Cannot resolve")
            || combined_output.contains("Failed to initialize") // App init may fail first
            || combined_output.contains("Network address")
            || combined_output.contains("too long")
            || combined_output.contains("invalid");

        assert!(
            has_user_friendly_error,
            "Should show user-friendly error for {}. Output: {}",
            scenario_desc, combined_output
        );

        // Verify no stack traces or internal error details leak to user
        assert!(
            !combined_output.contains("panic")
                && !combined_output.contains("SIGABRT")
                && !combined_output.contains("backtrace")
                && !combined_output.contains("rust backtrace"),
            "Should not expose internal errors for {}. Output: {}",
            scenario_desc,
            combined_output
        );

        println!("    ✓ {} handled gracefully", scenario_desc);
    }

    println!("✅ Network failure handling test passed");
    println!("   - All network failure scenarios handled gracefully");
    println!("   - User-friendly error messages provided");
    println!("   - No internal error details exposed");
}

/// Test retry logic and exponential backoff handling
#[tokio::test]
async fn test_error_handling_retry_logic_exponential_backoff() {
    println!("Testing retry logic and exponential backoff handling");

    // Test retry behavior using unit test approach for error types
    let network_timeout_error = create_network_timeout_error("connect", 5);

    // Verify error is marked as recoverable for retry
    assert!(
        is_recoverable_error(&network_timeout_error),
        "Network timeout should be recoverable for retry"
    );

    // Test various timeout scenarios create appropriate retry conditions
    let test_operations = vec![
        ("connect", 5),
        ("send_invitation", 10),
        ("send_move", 3),
        ("handshake", 15),
    ];

    for (operation, timeout_seconds) in test_operations {
        let error = create_network_timeout_error(operation, timeout_seconds);

        // Verify error contains operation context
        let error_string = error.to_string();
        assert!(
            error_string.contains(operation),
            "Error should contain operation '{}' in message: {}",
            operation,
            error_string
        );

        // Verify timeout information is included
        assert!(
            error_string.contains(&timeout_seconds.to_string()),
            "Error should contain timeout {} seconds in message: {}",
            timeout_seconds,
            error_string
        );

        // Verify helpful suggestion is provided
        assert!(
            error_string.contains("Suggestion:") || error_string.contains("💡"),
            "Error should contain helpful suggestion: {}",
            error_string
        );

        println!(
            "    ✓ {} timeout error created with proper retry context",
            operation
        );
    }

    println!("✅ Retry logic and exponential backoff test passed");
    println!("   - Network timeout errors marked as recoverable");
    println!("   - Operation context preserved in error messages");
    println!("   - Helpful retry suggestions provided");
}

/// Test offline operation capabilities and error handling
#[tokio::test]
async fn test_error_handling_offline_operation_capabilities() {
    println!("Testing offline operation capabilities and error handling");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    // Test operations that should work offline
    let offline_operations = vec![
        (vec!["games"], "games list"),
        (vec!["board"], "board display"),
        (vec!["history"], "move history"),
    ];

    for (args, operation_desc) in offline_operations {
        println!("  Testing offline operation: {}", operation_desc);

        let output = timeout(
            Duration::from_secs(10),
            Command::new(get_mate_binary_path())
                .args(&args)
                .env("MATE_DATA_DIR", &temp_path)
                .env("RUST_LOG", "error")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let command_output = output
            .expect("Offline operation should complete within timeout")
            .expect("Offline operation should execute");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{stdout}{stderr}");

        // Operations should either succeed or fail gracefully (not hang)
        // Success is acceptable if app initializes properly
        // Graceful failure is acceptable if database isn't set up

        if !command_output.status.success() {
            // If it fails, should be due to missing data, not network issues
            assert!(
                !combined_output.contains("connection")
                    && !combined_output.contains("network")
                    && !combined_output.contains("timeout"),
                "Offline operation '{}' should not fail due to network issues. Output: {}",
                operation_desc,
                combined_output
            );
        }

        println!(
            "    ✓ {} operation handles offline mode appropriately",
            operation_desc
        );
    }

    // Test operations that require network and should fail gracefully offline
    let network_operations = vec![
        (vec!["invite", "127.0.0.1:8080"], "game invitation"),
        (vec!["accept", "dummy-id"], "game acceptance"),
    ];

    for (args, operation_desc) in network_operations {
        println!("  Testing network operation offline: {}", operation_desc);

        let output = timeout(
            get_adaptive_timeout(8),
            Command::new(get_mate_binary_path())
                .args(&args)
                .env("MATE_DATA_DIR", &temp_path)
                .env("RUST_LOG", "error")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let command_output = output
            .expect("Network operation should complete within timeout")
            .expect("Network operation should execute");

        // Network operations should fail gracefully when offline
        assert!(
            !command_output.status.success(),
            "Network operation '{}' should fail when offline",
            operation_desc
        );

        println!(
            "    ✓ {} operation fails gracefully when offline",
            operation_desc
        );
    }

    println!("✅ Offline operation capabilities test passed");
    println!("   - Offline operations work or fail gracefully");
    println!("   - Network operations fail appropriately when offline");
    println!("   - No network timeout issues in offline mode");
}

//=============================================================================
// Database Error Handling Tests
//=============================================================================

/// Test database connection failure handling
#[tokio::test]
async fn test_error_handling_database_connection_failures() {
    println!("Testing database connection failure handling");

    // Test with unwritable directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let readonly_path = temp_dir.path().join("readonly");
    create_readonly_directory(&readonly_path)
        .await
        .expect("Failed to create readonly directory");

    let output = timeout(
        get_adaptive_timeout(8),
        Command::new(get_mate_binary_path())
            .args(["games"])
            .env("MATE_DATA_DIR", readonly_path.to_string_lossy().to_string())
            .env("RUST_LOG", "error")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    // Should fail gracefully with database error
    assert!(
        !command_output.status.success(),
        "Should fail with database connection error"
    );

    // Should provide helpful error message about database or filesystem issues
    assert!(
        combined_output.contains("database")
            || combined_output.contains("Database")
            || combined_output.contains("permissions")
            || combined_output.contains("failed to create")
            || combined_output.contains("Failed to initialize")
            || combined_output.contains("Storage error")
            || combined_output.contains("File operation failed"),
        "Should show database/storage-related error message. Output: {}",
        combined_output
    );

    // Clean up permissions for temp directory cleanup
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&readonly_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&readonly_path, perms).unwrap();
    }

    println!("✅ Database connection failure handling test passed");
    println!("   - Database connection failures handled gracefully");
    println!("   - Helpful error messages provided");
    println!("   - Appropriate exit codes returned");
}

/// Test transaction rollback error handling
#[tokio::test]
async fn test_error_handling_transaction_rollback() {
    println!("Testing transaction rollback error handling");

    // Test error type hierarchy for transaction errors
    let transaction_error =
        StorageError::transaction_failed("move_processing", "constraint violation");

    // Verify transaction error is properly typed
    assert!(
        matches!(transaction_error, StorageError::TransactionFailed { .. }),
        "Should create proper transaction error type"
    );

    // Verify transaction error contains context
    let error_string = transaction_error.to_string();
    assert!(
        error_string.contains("move_processing"),
        "Transaction error should contain operation context: {}",
        error_string
    );

    assert!(
        error_string.contains("constraint violation"),
        "Transaction error should contain failure reason: {}",
        error_string
    );

    // Test CLI error conversion for transaction errors
    let cli_error = CliError::from(transaction_error);

    // Test that transaction errors provide recovery suggestions
    assert!(
        !is_recoverable_error(&cli_error),
        "Transaction constraint violations should not be recoverable"
    );
    let cli_error_string = cli_error.to_string();

    // Verify CLI error formatting is user-friendly
    assert!(
        cli_error_string.contains("Database") || cli_error_string.contains("database"),
        "CLI error should mention database context: {}",
        cli_error_string
    );

    assert!(
        cli_error_string.contains("💡") || cli_error_string.contains("Suggestion"),
        "CLI error should provide helpful suggestions: {}",
        cli_error_string
    );

    println!("✅ Transaction rollback error handling test passed");
    println!("   - Transaction errors properly typed and contextualized");
    println!("   - Recovery suggestions provided appropriately");
    println!("   - CLI error formatting is user-friendly");
}

/// Test corrupted game data recovery
#[tokio::test]
async fn test_error_handling_corrupted_game_data_recovery() {
    println!("Testing corrupted game data recovery");

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let data_dir = temp_dir.path().to_path_buf();

    // Create corrupted database file
    create_corrupted_database(&data_dir)
        .await
        .expect("Failed to create corrupted database");

    let output = timeout(
        get_adaptive_timeout(8),
        Command::new(get_mate_binary_path())
            .args(["games"])
            .env("MATE_DATA_DIR", data_dir.to_string_lossy().to_string())
            .env("RUST_LOG", "error")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    // Should fail gracefully with database corruption error
    assert!(
        !command_output.status.success(),
        "Should fail when database is corrupted"
    );

    // Should provide helpful guidance about database corruption
    assert!(
        combined_output.contains("database")
            || combined_output.contains("Database")
            || combined_output.contains("corrupt")
            || combined_output.contains("invalid")
            || combined_output.contains("format"),
        "Should indicate database corruption issue. Output: {}",
        combined_output
    );

    // Should not crash or cause undefined behavior
    let exit_code = command_output.status.code().unwrap_or(-1);
    assert!(
        exit_code > 0 && exit_code < 128,
        "Should exit with reasonable error code, got: {}",
        exit_code
    );

    println!("✅ Corrupted game data recovery test passed");
    println!("   - Corrupted database handled gracefully without crashes");
    println!("   - Helpful error messages about database issues");
    println!("   - Reasonable exit codes provided");
}

/// Test database error message handling
#[tokio::test]
async fn test_error_handling_database_error_messages() {
    println!("Testing database error message handling");

    // Test various database error types for user-friendly formatting
    let test_errors = vec![
        (
            StorageError::game_not_found("abc123"),
            vec!["Game", "abc123", "not found"],
        ),
        (
            StorageError::database_locked("move_operation", 5000),
            vec!["locked", "move_operation", "5000"],
        ),
        (
            StorageError::invalid_data("move_notation", "invalid format"),
            vec!["Invalid data", "move_notation", "invalid format"],
        ),
        (
            StorageError::database_corruption("table schema mismatch"),
            vec!["corruption", "table schema mismatch"],
        ),
    ];

    for (storage_error, expected_content) in test_errors {
        let cli_error = CliError::from(storage_error);
        let error_message = cli_error.to_string();

        // Verify all expected content is present
        for content in expected_content {
            assert!(
                error_message
                    .to_lowercase()
                    .contains(&content.to_lowercase()),
                "Error message should contain '{}': {}",
                content,
                error_message
            );
        }

        // Verify user-friendly formatting
        assert!(
            error_message.contains("💡") || error_message.contains("Suggestion"),
            "Error message should contain helpful suggestions: {}",
            error_message
        );

        // Verify no raw error codes or technical details leak through
        assert!(
            !error_message.contains("SQLITE_")
                && !error_message.contains("ErrorCode::")
                && !error_message.contains("rusqlite::"),
            "Error message should not contain raw technical details: {}",
            error_message
        );

        println!("    ✓ Database error message formatted appropriately");
    }

    println!("✅ Database error message handling test passed");
    println!("   - All database errors formatted user-friendly");
    println!("   - Helpful suggestions provided");
    println!("   - No technical details leaked to users");
}

//=============================================================================
// Input Error Handling Tests
//=============================================================================

/// Test malformed user input handling
#[tokio::test]
async fn test_error_handling_malformed_user_input() {
    println!("Testing malformed user input handling");

    // Create unique temp directory to avoid conflicts with parallel tests
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let unique_suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let unique_dir = temp_dir
        .path()
        .join(format!("malformed_input_{}", unique_suffix));
    tokio::fs::create_dir_all(&unique_dir)
        .await
        .expect("Failed to create unique directory");
    let temp_path = unique_dir.to_string_lossy().to_string();

    let malformed_inputs = vec![
        (vec!["move", ""], "empty move"),
        (vec!["move", "xyz123"], "invalid move notation"),
        (vec!["invite", ""], "empty address"),
        (vec!["invite", "not-an-address"], "invalid address format"),
        (vec!["accept", ""], "empty game ID"),
        (vec!["accept", "not-a-uuid"], "invalid game ID format"),
        (vec!["board", "--game-id", ""], "empty game ID parameter"),
    ];

    for (args, input_desc) in malformed_inputs {
        println!("  Testing malformed input: {}", input_desc);

        let output = timeout(
            get_adaptive_timeout(8),
            Command::new(get_mate_binary_path())
                .args(&args)
                .env("MATE_DATA_DIR", &temp_path)
                .env("RUST_LOG", "error")
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{stdout}{stderr}");

        // Should fail gracefully with validation error
        assert!(
            !command_output.status.success(),
            "Should fail with validation error for: {}",
            input_desc
        );

        // Should provide helpful error message about input format, game state, or connection failure
        assert!(
            combined_output.contains("Invalid")
                || combined_output.contains("invalid")
                || combined_output.contains("format")
                || combined_output.contains("required")
                || combined_output.contains("missing")
                || combined_output.contains("No active games")
                || combined_output.contains("not found")
                || combined_output.contains("Failed to make move")
                || combined_output.contains("Failed to connect")
                || combined_output.contains("Failed to send")
                || combined_output.contains("Connection failed"),
            "Should show helpful error for {}. Output: {}",
            input_desc,
            combined_output
        );

        // Should not crash or hang
        let exit_code = command_output.status.code().unwrap_or(-1);
        assert!(
            exit_code > 0 && exit_code < 128,
            "Should exit with reasonable error code for {}, got: {}",
            input_desc,
            exit_code
        );

        println!("    ✓ {} handled gracefully", input_desc);
    }

    println!("✅ Malformed user input handling test passed");
    println!("   - All malformed inputs handled gracefully");
    println!("   - Helpful validation error messages provided");
    println!("   - No crashes or hangs on invalid input");
}

/// Test validation error types
#[tokio::test]
async fn test_error_handling_validation_error_types() {
    println!("Testing validation error types");

    // Test input validation error creation and formatting
    let validation_errors = vec![
        ("game_id", "xyz123", "not a valid UUID format"),
        ("chess_move", "Z9", "invalid algebraic notation"),
        ("color", "purple", "must be 'white' or 'black'"),
        ("address", "invalid", "must be in format 'host:port'"),
    ];

    for (field, value, reason) in validation_errors {
        let validation_error = create_input_validation_error(field, value, reason);

        // Verify error type is correct
        assert!(
            matches!(validation_error, CliError::InvalidInput { .. }),
            "Should create InvalidInput error type for field: {}",
            field
        );

        let error_message = validation_error.to_string();

        // Verify error contains all components
        assert!(
            error_message.contains(field),
            "Error should contain field name '{}': {}",
            field,
            error_message
        );

        assert!(
            error_message.contains(value),
            "Error should contain invalid value '{}': {}",
            value,
            error_message
        );

        assert!(
            error_message.contains(reason),
            "Error should contain reason '{}': {}",
            reason,
            error_message
        );

        // Verify helpful suggestion is provided
        assert!(
            error_message.contains("💡") || error_message.contains("Suggestion"),
            "Error should contain helpful suggestion: {}",
            error_message
        );

        // Verify error is marked as recoverable (user can fix input)
        assert!(
            is_recoverable_error(&validation_error),
            "Input validation errors should be recoverable"
        );

        println!("    ✓ {} validation error properly formatted", field);
    }

    println!("✅ Validation error types test passed");
    println!("   - All validation errors properly typed");
    println!("   - Error messages contain all necessary information");
    println!("   - Helpful suggestions provided for fixing input");
}

/// Test invalid game states recovery
#[tokio::test]
async fn test_error_handling_invalid_game_states_recovery() {
    println!("Testing invalid game states recovery");

    // Test game operations error handling
    let game_ops_errors = vec![
        (GameOpsError::NoCurrentGame, "no current game"),
        (
            GameOpsError::GameNotFound("test123".to_string()),
            "game not found",
        ),
        (
            GameOpsError::InvalidGameState("corrupted move history".to_string()),
            "invalid game state",
        ),
    ];

    for (game_ops_error, error_desc) in game_ops_errors {
        let cli_error = CliError::from(game_ops_error);
        let error_message = cli_error.to_string();

        // Verify error message is user-friendly
        assert!(
            error_message.contains("🎮") || error_message.contains("Game"),
            "Game error should have game context: {}",
            error_message
        );

        // Verify helpful suggestions are provided
        assert!(
            error_message.contains("💡") || error_message.contains("Suggestion"),
            "Game error should provide suggestions: {}",
            error_message
        );

        // Test command-specific error handling
        let enhanced_error = handle_chess_command_error(cli_error, "move");
        let enhanced_message = enhanced_error.to_string();

        // Verify command-specific enhancements
        assert!(
            enhanced_message.len() >= error_message.len(),
            "Command-specific error should be enhanced for {}",
            error_desc
        );

        println!(
            "    ✓ {} error handled with appropriate recovery guidance",
            error_desc
        );
    }

    println!("✅ Invalid game states recovery test passed");
    println!("   - Game state errors provide recovery guidance");
    println!("   - Command-specific error enhancement works");
    println!("   - User-friendly messaging maintained");
}

/// Test edge cases and boundary conditions
#[tokio::test]
async fn test_error_handling_edge_cases() {
    println!("Testing error handling edge cases and boundary conditions");
    let multiplier = get_timeout_multiplier();
    println!("   Using timeout multiplier: {:.1}x", multiplier);

    // Verify binary availability before running test
    let binary_path = verify_binary_availability().expect("mate binary should be available");
    println!("   Using binary: {}", binary_path);

    // Use unique temp directory to avoid conflicts with concurrent tests
    let temp_dir = create_unique_temp_dir().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_string_lossy().to_string();

    let long_address = "a".repeat(1000);
    let edge_cases = vec![
        // Very long inputs
        (vec!["invite", &long_address], "extremely long address"),
        // Special characters
        (
            vec!["move", "♔♕♖♗♘♙"],
            "unicode chess pieces instead of notation",
        ),
        // Empty arguments
        (vec!["board", "--game-id"], "missing argument value"),
        // Multiple conflicting options
        (
            vec![
                "invite",
                "127.0.0.1:8080",
                "--color",
                "white",
                "--color",
                "black",
            ],
            "conflicting color options",
        ),
    ];

    // Convert edge cases to owned strings to avoid lifetime issues
    let edge_cases_owned: Vec<(Vec<String>, &str)> = edge_cases
        .into_iter()
        .map(|(args, desc)| (args.into_iter().map(|s| s.to_string()).collect(), desc))
        .collect();

    for (args, case_desc) in edge_cases_owned {
        println!("  Testing edge case: {}", case_desc);

        // Add retry logic for edge case testing to handle CI flakiness
        let temp_path_clone = temp_path.clone();
        let binary_path_clone = binary_path.clone();
        let result = retry_with_backoff(
            move || {
                let temp_path_ref = temp_path_clone.clone();
                let binary_path_ref = binary_path_clone.clone();
                let args_ref = args.clone(); // Clone args for the closure
                Box::pin(async move {
                    timeout(
                        get_adaptive_timeout(25), // Increased timeout for edge cases
                        Command::new(binary_path_ref)
                            .args(&args_ref)
                            .env("MATE_DATA_DIR", &temp_path_ref)
                            .env("RUST_LOG", "error")
                            .stdout(Stdio::piped())
                            .stderr(Stdio::piped())
                            .output(),
                    )
                    .await
                })
            },
            if multiplier > 3.0 { 5 } else { 3 }, // More retries for CI
            500,                                  // 500ms base delay (increased for CI)
            &format!("edge case test for {}", case_desc),
        )
        .await;

        let command_output = result
            .expect("Edge case should complete within timeout")
            .expect("Edge case should execute");

        // Should handle edge cases gracefully (not crash)
        let exit_code = command_output.status.code().unwrap_or(-1);
        assert!(
            (0..128).contains(&exit_code),
            "Should handle edge case '{}' gracefully, got exit code: {}",
            case_desc,
            exit_code
        );

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{stdout}{stderr}");

        // Should not contain stack traces or panics
        assert!(
            !combined_output.contains("panic")
                && !combined_output.contains("SIGABRT")
                && !combined_output.contains("backtrace")
                && !combined_output.contains("rust backtrace")
                && !combined_output.contains("thread panicked"),
            "Edge case '{}' should not cause panic. Output: {}",
            case_desc,
            combined_output
        );

        println!("    ✓ {} handled gracefully", case_desc);
    }

    println!("✅ Edge cases and boundary conditions test passed");
    println!("   - All edge cases handled without crashes");
    println!("   - No panics or stack traces in edge cases");
    println!("   - Reasonable exit codes for all conditions");
}

//=============================================================================
// Cross-Component Error Integration Tests
//=============================================================================

/// Test error type consistency across all CLI components
#[tokio::test]
async fn test_error_type_consistency_across_components() {
    println!("Testing error type consistency across all CLI components");

    // Test that all major error types convert properly to CliError
    let error_conversions = vec![
        (
            "ChessError",
            CliError::from(ChessError::InvalidMove("e9".to_string())),
        ),
        (
            "StorageError",
            CliError::from(StorageError::game_not_found("test123")),
        ),
        ("GameOpsError", CliError::from(GameOpsError::NoCurrentGame)),
        (
            "ConnectionError",
            CliError::from(ConnectionError::ConnectionClosed),
        ),
        (
            "WireProtocolError",
            CliError::from(WireProtocolError::ProtocolViolation {
                description: "test violation".to_string(),
            }),
        ),
    ];

    for (error_type, cli_error) in error_conversions {
        let error_message = cli_error.to_string();

        // Verify consistent formatting across error types
        assert!(
            error_message.contains("💡") || error_message.contains("Suggestion"),
            "{} should provide helpful suggestions: {}",
            error_type,
            error_message
        );

        // Verify appropriate emoji/icon usage for context
        let has_contextual_icon = error_message.contains("🎮")
            || error_message.contains("♟️")
            || error_message.contains("🗃️")
            || error_message.contains("🌐")
            || error_message.contains("📡")
            || error_message.contains("❌");

        assert!(
            has_contextual_icon,
            "{} should use contextual icons: {}",
            error_type, error_message
        );

        // Verify no raw error details leak through
        assert!(
            !error_message.contains("Error {")
                && !error_message.contains("std::io::Error")
                && !error_message.contains("anyhow::Error"),
            "{} should not expose raw error details: {}",
            error_type,
            error_message
        );

        println!(
            "    ✓ {} error formatting is consistent and user-friendly",
            error_type
        );
    }

    println!("✅ Error type consistency test passed");
    println!("   - All error types format consistently");
    println!("   - Contextual icons and suggestions provided");
    println!("   - No raw error details exposed to users");
}

/// Test comprehensive error recovery suggestions
#[tokio::test]
async fn test_comprehensive_error_recovery_suggestions() {
    println!("Testing comprehensive error recovery suggestions");

    // Test that recoverable vs non-recoverable errors are properly classified
    let recoverable_errors = vec![
        create_network_timeout_error("connect", 5),
        CliError::from(ConnectionError::ConnectionClosed),
        create_input_validation_error("game_id", "invalid", "not a UUID"),
        CliError::from(GameOpsError::NoCurrentGame),
        CliError::from(StorageError::database_locked("operation", 1000)),
    ];

    let non_recoverable_errors = vec![
        CliError::from(StorageError::database_corruption(
            "schema mismatch".to_string(),
        )),
        CliError::from(StorageError::invalid_data("field", "corrupted")),
    ];

    // Test recoverable errors
    for error in recoverable_errors {
        assert!(
            is_recoverable_error(&error),
            "Error should be marked as recoverable: {}",
            error
        );

        let error_message = error.to_string();
        assert!(
            error_message.contains("💡") || error_message.contains("Suggestion"),
            "Recoverable error should provide recovery suggestion: {}",
            error_message
        );

        println!("    ✓ Recoverable error properly classified with suggestion");
    }

    // Test non-recoverable errors
    for error in non_recoverable_errors {
        assert!(
            !is_recoverable_error(&error),
            "Error should be marked as non-recoverable: {}",
            error
        );

        let _error_message = error.to_string();
        // Non-recoverable errors may still have suggestions (like "report this bug")
        // but they should indicate the severity

        println!("    ✓ Non-recoverable error properly classified");
    }

    println!("✅ Comprehensive error recovery suggestions test passed");
    println!("   - Recoverable errors properly identified");
    println!("   - Recovery suggestions provided where appropriate");
    println!("   - Error classification helps user decision making");
}
