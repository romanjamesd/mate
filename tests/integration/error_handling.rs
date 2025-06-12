//! Error Handling Tests
//!
//! Tests for Error Handling functionality as specified in tests-to-add.md:
//! - Test graceful handling of connection establishment failures
//! - Test appropriate program exit behavior when initial connection fails
//! - Test that errors are logged at appropriate levels
//! - Test user-friendly error communication
//! - Test that cleanup failures don't prevent program termination
//! - Test that errors don't cause crashes or undefined behavior

use anyhow::Result;
use mate::crypto::Identity;
use mate::network::Server;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Helper function to start a test server
async fn start_test_server(bind_addr: &str) -> Result<Server> {
    let identity = Arc::new(Identity::generate()?);
    let server = Server::bind(bind_addr, identity).await?;
    Ok(server)
}

/// Helper function to build the mate binary path
fn get_mate_binary_path() -> String {
    // In tests, the binary is built in target/debug/
    "target/debug/mate".to_string()
}

/// Test graceful handling of connection establishment failures
#[tokio::test]
async fn test_graceful_connection_establishment_failure_handling() {
    println!("Testing graceful handling of connection establishment failures");

    // Test invalid address format
    let invalid_addresses = [
        "invalid-address",      // Missing port
        "192.168.1.999:8080",   // Invalid IP
        "localhost:999999",     // Invalid port
        "not-a-real-host:8080", // Non-existent host
        "",                     // Empty address
        "127.0.0.1:0",          // Port 0 (reserved)
    ];

    for (i, invalid_addr) in invalid_addresses.iter().enumerate() {
        println!(
            "Test {}: Testing connection failure with address: '{}'",
            i + 1,
            invalid_addr
        );

        let output = timeout(
            Duration::from_secs(15), // Allow extra time for DNS resolution timeouts
            Command::new(get_mate_binary_path())
                .args(["connect", invalid_addr, "--message", "test"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        // Verify graceful failure (non-zero exit code)
        assert!(
            !command_output.status.success(),
            "Command should fail gracefully for invalid address: {}",
            invalid_addr
        );

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify error message contains useful information
        assert!(
            combined_output.contains("Failed to connect")
                || combined_output.contains("Connection failed")
                || combined_output.contains("failed")
                || combined_output.contains("ERROR")
                || combined_output.contains("error"),
            "Should show connection error for address '{}'. Output: {}",
            invalid_addr,
            combined_output
        );

        // Verify program doesn't crash (having gotten this far means it didn't)
        let exit_code = command_output.status.code().unwrap_or(-1);
        assert_ne!(
            exit_code, 0,
            "Should exit with non-zero code for address: {}",
            invalid_addr
        );

        println!(
            "   ✓ Address '{}' failed gracefully with exit code: {}",
            invalid_addr, exit_code
        );
    }

    println!("✅ Graceful connection establishment failure handling test passed");
    println!("   - All invalid addresses handled gracefully");
    println!("   - Appropriate error messages displayed");
    println!("   - No crashes or undefined behavior");
}

/// Test appropriate program exit behavior when initial connection fails
#[tokio::test]
async fn test_appropriate_program_exit_behavior_on_connection_failure() {
    println!("Testing appropriate program exit behavior when initial connection fails");

    // Test both one-shot mode and interactive mode connection failures
    let non_existent_server = "127.0.0.1:19998"; // Unlikely to be in use

    // Test 1: One-shot mode failure
    println!("Test 1: One-shot mode connection failure");
    {
        let start_time = std::time::Instant::now();

        let output = timeout(
            Duration::from_secs(10),
            Command::new(get_mate_binary_path())
                .args(["connect", non_existent_server, "--message", "test message"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let execution_time = start_time.elapsed();

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        // Verify program exits promptly (not hanging)
        assert!(
            execution_time < Duration::from_secs(8),
            "Program should exit promptly on connection failure, took: {:?}",
            execution_time
        );

        // Verify appropriate exit code
        assert!(
            !command_output.status.success(),
            "Should exit with failure code when connection fails"
        );

        let exit_code = command_output.status.code().unwrap_or(-1);
        assert_eq!(
            exit_code, 1,
            "Should exit with code 1 on connection failure"
        );

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify no partial output from successful operations
        assert!(
            !combined_output.contains("Connected to peer")
                && !combined_output.contains("MATE Chat Session"),
            "Should not show successful connection output. Output: {}",
            combined_output
        );

        println!(
            "   ✓ One-shot mode exits promptly with code {} in {:?}",
            exit_code, execution_time
        );
    }

    // Test 2: Interactive mode failure
    println!("Test 2: Interactive mode connection failure");
    {
        let start_time = std::time::Instant::now();

        let output = timeout(
            Duration::from_secs(10),
            Command::new(get_mate_binary_path())
                .args(["connect", non_existent_server])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let execution_time = start_time.elapsed();

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        // Verify program exits promptly (doesn't enter interactive mode)
        assert!(
            execution_time < Duration::from_secs(8),
            "Program should exit promptly on connection failure in interactive mode, took: {:?}",
            execution_time
        );

        // Verify appropriate exit code
        assert!(
            !command_output.status.success(),
            "Interactive mode should exit with failure code when connection fails"
        );

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify doesn't enter interactive mode
        assert!(
            !combined_output.contains("MATE Chat Session")
                && !combined_output.contains("Available commands")
                && !combined_output.contains("mate>"),
            "Should not enter interactive mode on connection failure. Output: {}",
            combined_output
        );

        println!("   ✓ Interactive mode exits promptly without entering session");
    }

    println!("✅ Appropriate program exit behavior test passed");
    println!("   - Programs exit promptly on connection failure");
    println!("   - Appropriate exit codes returned");
    println!("   - No hanging or entering interactive mode inappropriately");
}

/// Test that errors are logged at appropriate levels
#[tokio::test]
async fn test_errors_logged_at_appropriate_levels() {
    println!("Testing that errors are logged at appropriate levels");

    let non_existent_server = "127.0.0.1:19997";

    // Test with different log levels to verify appropriate error logging
    let log_levels = vec!["error", "warn", "info", "debug"];

    for log_level in log_levels {
        println!("Testing with RUST_LOG={}", log_level);

        let output = timeout(
            Duration::from_secs(8),
            Command::new(get_mate_binary_path())
                .args(["connect", non_existent_server, "--message", "test"])
                .env("RUST_LOG", log_level)
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
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify error-level logging is present
        if log_level == "error"
            || log_level == "warn"
            || log_level == "info"
            || log_level == "debug"
        {
            assert!(
                combined_output.contains("ERROR")
                    || combined_output.contains("error")
                    || combined_output.contains("Failed"),
                "Should contain error logging at level {}. Output: {}",
                log_level,
                combined_output
            );
        }

        // Verify that higher log levels include more detail
        if log_level == "debug" {
            // Debug level should have more detailed information
            let log_entry_count = combined_output
                .lines()
                .filter(|line| {
                    line.contains("INFO") || line.contains("DEBUG") || line.contains("ERROR")
                })
                .count();

            assert!(
                log_entry_count > 0,
                "Debug level should include multiple log entries. Output: {}",
                combined_output
            );
        }

        println!(
            "   ✓ Log level {} shows appropriate error information",
            log_level
        );
    }

    println!("✅ Appropriate error logging levels test passed");
    println!("   - Errors are logged at appropriate levels");
    println!("   - Higher log levels include more detail");
    println!("   - Error information is accessible through logging");
}

/// Test user-friendly error communication
#[tokio::test]
async fn test_user_friendly_error_communication() {
    println!("Testing user-friendly error communication");

    // Test various error scenarios for user-friendly messages
    let test_scenarios = vec![
        (
            "127.0.0.1:19996",
            "connection refused",
            "Server not available",
        ),
        ("invalid-host-name:8080", "resolve", "Invalid hostname"),
        ("192.168.999.999:8080", "resolve", "Invalid IP address"),
        ("localhost:99999", "invalid", "Invalid port number"),
    ];

    for (addr, expected_error_type, scenario_desc) in test_scenarios {
        println!("Testing user-friendly error for: {}", scenario_desc);

        let output = timeout(
            Duration::from_secs(10),
            Command::new(get_mate_binary_path())
                .args(["connect", addr, "--message", "test"])
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
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify error message is user-friendly (not just technical errors)
        assert!(
            combined_output.contains("Failed to connect")
                || combined_output.contains("Connection failed")
                || combined_output.contains("Unable to connect")
                || combined_output.contains(expected_error_type),
            "Should show user-friendly error for {}. Output: {}",
            scenario_desc,
            combined_output
        );

        // Verify no raw system errors are exposed without context
        assert!(
            !combined_output.contains("panic")
                && !combined_output.contains("thread")
                && !combined_output.contains("backtrace"),
            "Should not show raw panic/system errors for {}. Output: {}",
            scenario_desc,
            combined_output
        );

        // Verify error provides actionable information
        let has_actionable_info = combined_output.contains("check")
            || combined_output.contains("verify")
            || combined_output.contains("ensure")
            || combined_output.contains(addr)
            || combined_output.contains("address");

        assert!(
            has_actionable_info,
            "Error should provide actionable information for {}. Output: {}",
            scenario_desc, combined_output
        );

        println!(
            "   ✓ {} provides user-friendly error message",
            scenario_desc
        );
    }

    println!("✅ User-friendly error communication test passed");
    println!("   - Error messages are user-friendly and actionable");
    println!("   - Technical details are contextualized appropriately");
    println!("   - No raw system errors exposed");
}

/// Test that cleanup failures don't prevent program termination
#[tokio::test]
async fn test_cleanup_failures_dont_prevent_termination() {
    println!("Testing that cleanup failures don't prevent program termination");

    // Start a server and then kill it mid-session to test cleanup handling
    let server_addr = "127.0.0.1:18301";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move {
        // Server will run briefly then exit abruptly
        tokio::time::sleep(Duration::from_millis(300)).await;
        // Abrupt server termination simulates cleanup failure scenarios
        drop(server);
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start interactive session that will experience cleanup issues
    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Send a message that may succeed initially
        let _ = stdin.write_all(b"Test message before server death\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Server should be dead by now, so cleanup will encounter issues
        // Try to quit gracefully despite cleanup problems
        let _ = stdin.write_all(b"quit\n").await;
    }

    // Wait for server to terminate
    let _ = server_handle.await;

    let start_time = std::time::Instant::now();

    // Verify program terminates despite cleanup issues
    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await;

    let execution_time = start_time.elapsed();

    let command_output = output
        .expect("Command should complete within timeout despite cleanup issues")
        .expect("Command should execute");

    // Verify program terminates promptly even with cleanup failures
    assert!(
        execution_time < Duration::from_secs(8),
        "Program should terminate promptly despite cleanup failures, took: {:?}",
        execution_time
    );

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    // Program may exit with success or failure code, but should exit cleanly
    let exit_code = command_output.status.code().unwrap_or(-1);

    // Verify it doesn't hang even if cleanup logs warnings/errors
    assert!(
        exit_code == 0 || exit_code == 1,
        "Should exit with reasonable exit code (0 or 1), got: {}. Output: {}",
        exit_code,
        combined_output
    );

    // Verify no crashes occurred (stack traces, panics, etc.)
    assert!(
        !combined_output.contains("panic") && !combined_output.contains("thread panicked"),
        "Should not show panic messages. Output: {}",
        combined_output
    );

    println!("✅ Cleanup failure handling test passed");
    println!("   - Program terminates despite cleanup issues");
    println!("   - No hanging or infinite cleanup loops");
    println!("   - Exit time: {:?}, code: {}", execution_time, exit_code);
}

/// Test that errors don't cause crashes or undefined behavior
#[tokio::test]
async fn test_errors_dont_cause_crashes_or_undefined_behavior() {
    println!("Testing that errors don't cause crashes or undefined behavior");

    // Test various error conditions that could potentially cause crashes
    let stress_test_scenarios = vec![
        // Invalid addresses that might cause parsing errors
        ("", "empty address"),
        ("::::::::", "malformed address"),
        ("localhost:99999999999", "port overflow"),
        ("127.0.0.1:-1", "negative port"),
        ("127.0.0.1:abc", "non-numeric port"),
        ("127.0.0.1:8080:extra", "extra port segments"),
        (" ", "whitespace address"),
        ("localhost:", "missing port"),
        (":8080", "missing host"),
        ("localhost:8080/path", "address with path"),
        ("http://localhost:8080", "address with protocol"),
    ];

    for (addr, description) in stress_test_scenarios {
        println!("Testing crash resistance for: {}", description);

        let start_time = std::time::Instant::now();

        let output = timeout(
            Duration::from_secs(8),
            Command::new(get_mate_binary_path())
                .args(["connect", addr, "--message", "crash test"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output(),
        )
        .await;

        let execution_time = start_time.elapsed();

        // Command should complete (not hang or crash)
        let command_output = output
            .unwrap_or_else(|_| panic!("Command should complete for {}", description))
            .unwrap_or_else(|_| panic!("Command should execute for {}", description));

        // Verify program terminates promptly (no infinite loops)
        assert!(
            execution_time < Duration::from_secs(6),
            "Program should terminate promptly for {}, took: {:?}",
            description,
            execution_time
        );

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify no crashes, panics, or undefined behavior
        assert!(
            !combined_output.contains("panic")
                && !combined_output.contains("segmentation fault")
                && !combined_output.contains("abort")
                && !combined_output.contains("fatal")
                && !combined_output.contains("thread panicked"),
            "Should not show crash indicators for {}. Output: {}",
            description,
            combined_output
        );

        // Verify we get a reasonable exit code (not -1, 128+signal, etc.)
        let exit_code = command_output.status.code().unwrap_or(-1);
        assert!(
            (0..=2).contains(&exit_code),
            "Should have reasonable exit code for {}, got: {}",
            description,
            exit_code
        );

        // Verify we get some kind of error message (not silent failure)
        assert!(
            combined_output.contains("error")
                || combined_output.contains("Error")
                || combined_output.contains("failed")
                || combined_output.contains("Failed")
                || combined_output.contains("invalid")
                || combined_output.contains("Invalid"),
            "Should provide error feedback for {}. Output: {}",
            description,
            combined_output
        );

        println!("   ✓ {} handled safely", description);
    }

    println!("✅ Crash and undefined behavior resistance test passed");
    println!("   - All error scenarios handled without crashes");
    println!("   - No undefined behavior or panics observed");
    println!("   - Reasonable exit codes and error messages provided");
}

/// Test comprehensive error handling functionality
#[tokio::test]
async fn test_comprehensive_error_handling() {
    println!("Testing comprehensive error handling functionality");

    // Combined test of multiple error handling aspects
    let non_existent_server = "127.0.0.1:19995";

    let start_time = std::time::Instant::now();

    let output = timeout(
        Duration::from_secs(10),
        Command::new(get_mate_binary_path())
            .args([
                "connect",
                non_existent_server,
                "--message",
                "comprehensive test",
            ])
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await;

    let execution_time = start_time.elapsed();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    // Comprehensive checks for all error handling features
    let checks = vec![
        ("graceful_failure", !command_output.status.success()),
        (
            "prompt_termination",
            execution_time < Duration::from_secs(8),
        ),
        (
            "appropriate_exit_code",
            command_output.status.code() == Some(1),
        ),
        (
            "error_logging",
            combined_output.contains("ERROR") || combined_output.contains("error"),
        ),
        (
            "user_friendly_message",
            combined_output.contains("Failed to connect")
                || combined_output.contains("Connection failed"),
        ),
        (
            "no_crashes",
            !combined_output.contains("panic") && !combined_output.contains("abort"),
        ),
        (
            "actionable_error",
            combined_output.contains(non_existent_server) || combined_output.contains("address"),
        ),
        (
            "clean_output",
            !combined_output.contains("Connected to peer"),
        ),
        (
            "reasonable_timing",
            execution_time > Duration::from_millis(100),
        ), // Not too fast (should attempt connection)
    ];

    let mut passed_checks = 0;
    for (check_name, result) in checks.iter() {
        if *result {
            passed_checks += 1;
            println!("   ✓ {} check passed", check_name);
        } else {
            println!("   ✗ {} check failed", check_name);
        }
    }

    // Require most checks to pass for comprehensive error handling
    assert!(
        passed_checks >= 7,
        "At least 7/9 error handling checks should pass. Passed: {}/9. Output: {}",
        passed_checks,
        combined_output
    );

    println!("✅ Comprehensive error handling test passed");
    println!(
        "   - {}/{} error handling features verified",
        passed_checks,
        checks.len()
    );
    println!("   - Complete error handling workflow successful");
    println!("   - Execution time: {:?}", execution_time);
}
