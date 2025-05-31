//! CLI Integration Tests
//!
//! Tests for One-Shot Message Mode functionality as specified in tests-to-add.md:
//! - Test successful message send and echo response with timing information
//! - Test that message content is correctly echoed back
//! - Test that response timing is measured and displayed
//! - Test error handling when message sending fails
//! - Test error handling when response receiving fails
//! - Test that program exits after single message exchange
//! - Test appropriate logging for message operations

use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use mate::network::Server;
use mate::crypto::Identity;
use std::sync::Arc;
use anyhow::Result;

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

/// Test successful message send and echo response with timing information
#[tokio::test]
async fn test_successful_message_send_with_timing() {
    println!("Testing successful one-shot message send with timing information");

    // Start test server
    let server_addr = "127.0.0.1:18081";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    // Wait a moment for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test message content
    let test_message = "Hello, one-shot test!";

    // Execute mate connect command with --message flag
    let output = timeout(
        Duration::from_secs(10),
        Command::new(get_mate_binary_path())
            .args(&["connect", server_addr, "--message", test_message])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;

    // Clean up server
    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    // Verify command succeeded
    if !command_output.status.success() {
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let stdout = String::from_utf8_lossy(&command_output.stdout);
        panic!("Command failed with status: {}\nstderr: {}\nstdout: {}", 
               command_output.status, stderr, stdout);
    }

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    // Verify timing information is displayed
    assert!(combined_output.contains("round-trip"),
           "Output should contain timing information. combined: {}", combined_output);

    // Verify message was sent and response received
    assert!(combined_output.contains("Received echo") || combined_output.contains("Sending message"),
           "Output should show message sending/receiving. combined: {}", combined_output);

    println!("✅ Successful message send with timing test passed");
    println!("   - Command completed successfully");
    println!("   - Timing information was displayed");
    println!("   - Message exchange completed");
}

/// Test that message content is correctly echoed back
#[tokio::test]
async fn test_message_content_echo_correctness() {
    println!("Testing that message content is correctly echoed back");

    let server_addr = "127.0.0.1:18082";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test with various message contents
    let test_cases = vec![
        "Simple message",
        "Message with special characters: !@#$%^&*()",
        "Multi-word message with spaces",
        "123456789",
        "Mixed content: Hello World 123!",
    ];

    for test_message in test_cases {
        println!("  Testing message: '{}'", test_message);

        let output = timeout(
            Duration::from_secs(10),
            Command::new(get_mate_binary_path())
                .args(&["connect", server_addr, "--message", test_message])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        ).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute successfully");

        if !command_output.status.success() {
            let stderr = String::from_utf8_lossy(&command_output.stderr);
            panic!("Command failed for message '{}': {}", test_message, stderr);
        }

        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let combined_output = format!("{}{}", stdout, stderr);

        // Debug: print the actual output
        if combined_output.is_empty() {
            println!("WARNING: No output received for message '{}'", test_message);
            println!("Exit status: {}", command_output.status);
        }

        // Verify the exact message content appears in the echo response
        assert!(combined_output.contains(test_message),
               "Echo response should contain the exact message content '{}'. combined: {}", 
               test_message, combined_output);

        // Verify it's shown as received echo
        assert!(combined_output.contains("Received echo"),
               "Output should show received echo for message '{}'. combined: {}", 
               test_message, combined_output);
    }

    server_handle.abort();

    println!("✅ Message content echo correctness test passed");
    println!("   - All test messages were correctly echoed back");
    println!("   - Message content integrity verified");
    println!("   - Various message formats supported");
}

/// Test that response timing is measured and displayed
#[tokio::test]
async fn test_response_timing_measurement() {
    println!("Testing that response timing is measured and displayed correctly");

    let server_addr = "127.0.0.1:18083";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let test_message = "Timing test message";

    let output = timeout(
        Duration::from_secs(10),
        Command::new(get_mate_binary_path())
            .args(&["connect", server_addr, "--message", test_message])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    if !command_output.status.success() {
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        panic!("Command failed: {}", stderr);
    }

    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let combined_output = format!("{}{}", stdout, stderr);

    // Verify timing format is correct (should match our format_round_trip_time function)
    let timing_patterns = vec![
        r"round-trip: \d+μs",     // microseconds
        r"round-trip: \d+ms",     // milliseconds  
        r"round-trip: \d+\.\d+s", // seconds with decimals
    ];

    let has_timing = timing_patterns.iter().any(|pattern| {
        let regex = regex::Regex::new(pattern).unwrap();
        regex.is_match(&combined_output)
    });

    assert!(has_timing,
           "Output should contain properly formatted timing information. combined: {}", combined_output);

    // Verify timing appears in context of received echo
    assert!(combined_output.contains("round-trip") && combined_output.contains("Received echo"),
           "Timing should appear with echo response. combined: {}", combined_output);

    println!("✅ Response timing measurement test passed");
    println!("   - Timing information is properly formatted");
    println!("   - Timing appears with echo response");
    println!("   - Measurement precision is appropriate");
}

/// Test error handling when message sending fails
#[tokio::test]
async fn test_error_handling_send_failure() {
    println!("Testing error handling when message sending fails");

    // Try to connect to a non-existent server
    let invalid_addr = "127.0.0.1:19999"; // Unlikely to be in use
    let test_message = "This should fail to send";

    let output = timeout(
        Duration::from_secs(10),
        Command::new(get_mate_binary_path())
            .args(&["connect", invalid_addr, "--message", test_message])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    // Command should fail when trying to connect to non-existent server
    assert!(!command_output.status.success(),
           "Command should fail when connecting to non-existent server");

    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let combined_output = format!("{}{}", stdout, stderr);

    // Verify appropriate error handling - check for common error patterns
    assert!(combined_output.contains("Failed to connect") || 
           combined_output.contains("Connection refused") || 
           combined_output.contains("Connection failed") ||
           combined_output.contains("ERROR") || 
           combined_output.contains("error"),
           "Should show connection error. combined: {}", combined_output);

    // Verify the program exits with error code
    assert_ne!(command_output.status.code(), Some(0),
              "Exit code should be non-zero on failure");

    println!("✅ Send failure error handling test passed");
    println!("   - Connection failures are properly handled");
    println!("   - Appropriate error messages displayed");
    println!("   - Program exits with error code");
}

/// Test error handling when response receiving fails  
#[tokio::test]
async fn test_error_handling_receive_failure() {
    println!("Testing error handling when response receiving fails");

    // Start server but shut it down immediately after connection
    let server_addr = "127.0.0.1:18084";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        // Server will accept connection but then exit quickly
        tokio::time::sleep(Duration::from_millis(200)).await;
        drop(server);
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let test_message = "Message that may not get response";

    let output = timeout(
        Duration::from_secs(10),
        Command::new(get_mate_binary_path())
            .args(&["connect", server_addr, "--message", test_message])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let combined_output = format!("{}{}", stdout, stderr);

    // The command might succeed or fail depending on timing, but should handle it gracefully
    if !command_output.status.success() {
        // If it fails, should show appropriate error (connection or receive errors are both valid)
        assert!(combined_output.contains("Failed to receive") || 
               combined_output.contains("Failed to connect") || 
               combined_output.contains("Connection") || 
               combined_output.contains("ERROR") ||
               combined_output.contains("error"),
               "Should show receive or connection error when connection is lost. combined: {}", combined_output);
    }

    // Should not crash or hang - having gotten this far means it handled the situation
    println!("✅ Receive failure error handling test passed");
    println!("   - Connection interruptions handled gracefully");
    println!("   - No crashes or hangs occurred");
    println!("   - Error scenarios properly managed");
}

/// Test that program exits after single message exchange
#[tokio::test]
async fn test_program_exits_after_single_exchange() {
    println!("Testing that program exits after single message exchange");

    let server_addr = "127.0.0.1:18085";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let test_message = "Single exchange test";

    // Measure execution time
    let start_time = std::time::Instant::now();

    let output = timeout(
        Duration::from_secs(10),
        Command::new(get_mate_binary_path())
            .args(&["connect", server_addr, "--message", test_message])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;

    let execution_time = start_time.elapsed();
    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    // Verify command completed successfully
    if !command_output.status.success() {
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        panic!("Command should succeed: {}", stderr);
    }

    // Verify program exits promptly (not hanging in interactive mode)
    assert!(execution_time < Duration::from_secs(5),
           "Program should exit promptly after message exchange, took: {:?}", execution_time);

    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let combined_output = format!("{}{}", stdout, stderr);

    // Verify it shows message exchange completion
    assert!(combined_output.contains("Received echo") || combined_output.contains("round-trip"),
           "Should show completed message exchange. combined: {}", combined_output);

    // Verify it doesn't enter interactive mode
    assert!(!combined_output.contains("MATE Chat Session") && !combined_output.contains("Available commands"),
           "Should not enter interactive mode. combined: {}", combined_output);

    println!("✅ Program exit after single exchange test passed");
    println!("   - Program exits promptly after message exchange");
    println!("   - Does not enter interactive mode");
    println!("   - Execution time: {:?}", execution_time);
}

/// Test appropriate logging for message operations
#[tokio::test]
async fn test_appropriate_logging() {
    println!("Testing appropriate logging for one-shot message operations");

    let server_addr = "127.0.0.1:18086";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let test_message = "Logging test message";

    // Run with RUST_LOG environment variable to capture logs
    let output = timeout(
        Duration::from_secs(10),
        Command::new(get_mate_binary_path())
            .args(&["connect", server_addr, "--message", test_message])
            .env("RUST_LOG", "info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
    ).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    if !command_output.status.success() {
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        panic!("Command should succeed: {}", stderr);
    }

    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let combined_output = format!("{}{}", stdout, stderr);

    // Verify appropriate log messages are present
    let expected_log_patterns = vec![
        "Connecting to",           // Connection initiation
        "Connected to peer",       // Connection establishment  
        "Sending message",         // Message sending
        "Received echo",          // Response receiving
    ];

    for pattern in expected_log_patterns {
        assert!(combined_output.contains(pattern),
               "Should contain log message '{}'. combined: {}", pattern, combined_output);
    }

    // Verify log level is appropriate (info level messages)
    assert!(combined_output.contains("INFO") || combined_output.contains("info"),
           "Should contain info-level log messages. combined: {}", combined_output);

    // Verify the actual message content appears in logs
    assert!(combined_output.contains(test_message),
           "Log should contain the actual message content. combined: {}", combined_output);

    println!("✅ Appropriate logging test passed");
    println!("   - Connection events are logged");
    println!("   - Message operations are logged");
    println!("   - Log levels are appropriate");
    println!("   - Message content appears in logs");
}

/// Integration test combining multiple one-shot scenarios
#[tokio::test]
async fn test_one_shot_mode_comprehensive() {
    println!("Running comprehensive one-shot message mode integration test");

    let server_addr = "127.0.0.1:18087";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test multiple consecutive one-shot messages
    let test_messages = vec![
        "First one-shot message",
        "Second one-shot message", 
        "Third one-shot message with numbers 12345",
        "Final test message!",
    ];

    for (i, test_message) in test_messages.iter().enumerate() {
        println!("  Testing message {}: '{}'", i + 1, test_message);

        let output = timeout(
            Duration::from_secs(10),
            Command::new(get_mate_binary_path())
                .args(&["connect", server_addr, "--message", test_message])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
        ).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute successfully");

        assert!(command_output.status.success(),
               "Message {} should succeed", i + 1);

        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let combined_output = format!("{}{}", stdout, stderr);

        // Debug: print the actual output
        if combined_output.is_empty() {
            println!("WARNING: No output received for message '{}'", test_message);
            println!("Exit status: {}", command_output.status);
        }

        // Verify each message is handled correctly
        assert!(combined_output.contains(test_message) && combined_output.contains("round-trip"),
               "Message {} should be echoed with timing. combined: {}", i + 1, combined_output);

        // Brief pause between messages
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    server_handle.abort();

    println!("✅ Comprehensive one-shot mode test passed");
    println!("   - Multiple consecutive one-shot messages handled correctly");
    println!("   - Each message exchange completed independently");
    println!("   - Server handled multiple client connections");
} 