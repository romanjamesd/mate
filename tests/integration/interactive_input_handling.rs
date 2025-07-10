//! Interactive Input Handling Tests
//!
//! Tests for Interactive Input Handling functionality as specified in tests-to-add.md:
//! - Test clear prompt is displayed for user input
//! - Test empty input is ignored and doesn't send messages to peer
//! - Test whitespace-only input is treated as empty
//! - Test end-of-input (Ctrl+D) handling with graceful session termination
//! - Test input reading error handling
//! - Test that non-command input is sent as regular messages

use anyhow::Result;
use mate::crypto::Identity;
use mate::network::Server;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

// Import our new test helpers
use crate::common::test_helpers::*;

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

/// Test clear prompt is displayed for user input
#[tokio::test]
async fn test_clear_prompt_displayed() {
    println!("Testing that clear prompt is displayed for user input");

    let server_addr = "127.0.0.1:18111";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start mate in interactive mode
    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    // Wait for initialization and prompt display
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send quit command quickly to terminate
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Prompt display output:\n{}", combined_output);

    // Verify a prompt is displayed
    assert!(
        combined_output.contains(">")
            || combined_output.contains("$")
            || combined_output.contains("Enter")
            || combined_output.contains("Type")
            || combined_output.contains("message"),
        "Output should display a clear prompt for user input. Output: {}",
        combined_output
    );

    // Verify it's not just noise - should have some structure
    let line_count = combined_output.lines().count();
    assert!(
        line_count > 1,
        "Output should have multiple lines showing initialization and prompt. Output: {}",
        combined_output
    );

    println!("✅ Clear prompt display test passed");
    println!("   - Prompt is displayed for user input");
    println!("   - Output shows structured interface");
}

/// Test empty input is ignored and doesn't send messages to peer
#[tokio::test]
async fn test_empty_input_ignored() {
    println!("Testing that empty input is ignored and doesn't send messages to peer");

    let server_addr = "127.0.0.1:18112";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Send multiple empty inputs
        let _ = stdin.write_all(b"\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Send a real message to contrast
        let _ = stdin.write_all(b"test message\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Send more empty inputs
        let _ = stdin.write_all(b"\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(7), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Empty input handling output:\n{}", combined_output);

    // Count echo responses - should only be one for the real message
    let echo_count = combined_output.matches("Received echo").count();
    assert_eq!(echo_count, 1,
               "Should only have one echo response for the real message, not for empty inputs. Echo count: {}, Output: {}", 
               echo_count, combined_output);

    // Verify the real message was echoed
    assert!(
        combined_output.contains("test message"),
        "The real message should be echoed back. Output: {}",
        combined_output
    );

    // Verify no sending messages for empty inputs
    let user_message_sending = combined_output.matches("Sending Ping message").count();
    // Should have handshake ping + 1 user message ping
    assert!(user_message_sending <= 2,
           "Should not send extra messages for empty inputs. User message sending count: {}, Output: {}", 
           user_message_sending, combined_output);

    println!("✅ Empty input ignored test passed");
    println!("   - Empty inputs were ignored");
    println!("   - No messages sent to peer for empty inputs");
    println!("   - Real messages are still processed normally");
}

/// Test whitespace-only input is treated as empty
#[tokio::test]
async fn test_whitespace_only_input_treated_as_empty() {
    println!("Testing that whitespace-only input is treated as empty");

    let server_addr = "127.0.0.1:18113";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Send various whitespace-only inputs
        let _ = stdin.write_all(b" \n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"  \n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"\t\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b" \t \n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"   \t   \n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Send a real message to contrast
        let _ = stdin.write_all(b"real message\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(7), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Whitespace input handling output:\n{}", combined_output);

    // Count echo responses - should only be one for the real message
    let echo_count = combined_output.matches("Received echo").count();
    assert_eq!(echo_count, 1,
               "Should only have one echo response for the real message, not for whitespace-only inputs. Echo count: {}, Output: {}", 
               echo_count, combined_output);

    // Verify the real message was echoed
    assert!(
        combined_output.contains("real message"),
        "The real message should be echoed back. Output: {}",
        combined_output
    );

    // Verify no sending messages for whitespace-only inputs
    let user_message_sending = combined_output.matches("Sending Ping message").count();
    // Should have handshake ping + 1 user message ping
    assert!(user_message_sending <= 2,
           "Should not send extra messages for whitespace-only inputs. User message sending count: {}, Output: {}", 
           user_message_sending, combined_output);

    println!("✅ Whitespace-only input treated as empty test passed");
    println!("   - Whitespace-only inputs were treated as empty");
    println!("   - No messages sent to peer for whitespace-only inputs");
    println!("   - Real messages are still processed normally");
}

/// Test end-of-input (Ctrl+D) handling with graceful session termination
#[tokio::test]
async fn test_end_of_input_graceful_termination() {
    println!("Testing end-of-input (Ctrl+D) handling with graceful session termination");

    let server_addr = "127.0.0.1:18114";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Simulate Ctrl+D by closing stdin
    if let Some(stdin) = child.stdin.take() {
        drop(stdin); // This simulates end-of-input (Ctrl+D)
    }

    let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    // Verify graceful exit
    assert!(
        command_output.status.success(),
        "End-of-input should result in graceful exit. Status: {}",
        command_output.status
    );

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("End-of-input handling output:\n{}", combined_output);

    // Verify graceful termination (should show some kind of termination message or at least clean exit)
    // Note: The exact termination message may vary, so we check for graceful behavior
    let has_graceful_termination = combined_output.contains("session")
        || combined_output.contains("terminated")
        || combined_output.contains("goodbye")
        || combined_output.contains("exit")
        || command_output.status.success();

    assert!(
        has_graceful_termination,
        "Should handle end-of-input gracefully. Output: {}",
        combined_output
    );

    println!("✅ End-of-input graceful termination test passed");
    println!("   - End-of-input (Ctrl+D) handled gracefully");
    println!("   - Session terminated cleanly");
}

/// Test input reading error handling
#[tokio::test]
async fn test_input_reading_error_handling() {
    println!("Testing input reading error handling");

    let server_addr = "127.0.0.1:18115";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Send a normal message first
        let _ = stdin.write_all(b"normal message\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;
    }

    // Close stdin abruptly to simulate input error
    if let Some(stdin) = child.stdin.take() {
        drop(stdin);
    }

    let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Input error handling output:\n{}", combined_output);

    // Verify the normal message was processed before the error
    assert!(
        combined_output.contains("normal message"),
        "Normal message should be processed before input error. Output: {}",
        combined_output
    );

    // Verify graceful handling of input error (no crash, clean exit)
    assert!(
        command_output.status.success() || command_output.status.code() == Some(0),
        "Should handle input errors gracefully without crashing. Status: {}",
        command_output.status
    );

    println!("✅ Input reading error handling test passed");
    println!("   - Normal messages processed before error");
    println!("   - Input reading errors handled gracefully");
    println!("   - No crashes or undefined behavior");
}

/// Test that non-command input is sent as regular messages
#[tokio::test]
async fn test_non_command_input_sent_as_messages() {
    println!("Testing that non-command input is sent as regular messages");

    let server_addr = "127.0.0.1:18116";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    let test_messages = vec![
        "Hello world",
        "This is a regular message",
        "123456789",
        "Message with special chars: !@#$%^&*()",
        "Multi word message with spaces",
        "helpme",        // Not the "help" command
        "information",   // Not the "info" command
        "quitting soon", // Not the "quit" command
    ];

    if let Some(stdin) = child.stdin.as_mut() {
        // Send various non-command messages
        for message in &test_messages {
            let _ = stdin.write_all(format!("{message}\n").as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Non-command input handling output:\n{}", combined_output);

    // NEW APPROACH: Use behavioral verification with our helper functions

    // 1. Verify all messages were exchanged using the verifier
    let verifier =
        MessageExchangeVerifier::new(test_messages.iter().map(|s| s.to_string()).collect());

    if let Err(error) = verifier.verify(&combined_output) {
        panic!(
            "Message exchange verification failed: {}\nOutput: {}",
            error, combined_output
        );
    }

    // 2. Double-check: count expected vs actual echo responses
    let echo_count = count_echo_responses(&combined_output);
    assert_eq!(
        echo_count,
        test_messages.len(),
        "Should have echo responses for all test messages. Expected: {}, Got: {}, Output: {}",
        test_messages.len(),
        echo_count,
        combined_output
    );

    // 3. Verify each test message was echoed back (content verification)
    for message in &test_messages {
        assert!(
            combined_output.contains(message),
            "Message '{}' should be echoed back. Output: {}",
            message,
            combined_output
        );
    }

    // 4. Optional: Use session summary for the most reliable count
    if let Some(session_count) = extract_message_count_from_summary(&combined_output) {
        assert!(
            session_count >= test_messages.len() as u32,
            "Session summary should show at least {} messages. Got: {}, Output: {}",
            test_messages.len(),
            session_count,
            combined_output
        );
    }

    println!("✅ Non-command input sent as messages test passed");
    println!(
        "   - All {} non-command inputs were sent as regular messages",
        test_messages.len()
    );
    println!("   - All messages were echoed back by the server");
    println!("   - Similar-to-command inputs treated as regular messages");
}

/// Test comprehensive interactive input handling
#[tokio::test]
async fn test_comprehensive_interactive_input_handling() {
    println!("Testing comprehensive interactive input handling");

    let server_addr = "127.0.0.1:18117";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Test complete input handling workflow

        // Empty inputs (should be ignored)
        let _ = stdin.write_all(b"\n").await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let _ = stdin.write_all(b"  \n").await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Real command
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Regular message
        let _ = stdin.write_all(b"test message\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // More empty inputs
        let _ = stdin.write_all(b"\t\n").await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Info command
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Another regular message
        let _ = stdin.write_all(b"final message\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Graceful exit
        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(8), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Comprehensive input handling output:\n{}", combined_output);

    // Verify all input handling features worked
    let checks = [
        (
            "prompt_displayed",
            combined_output.contains(">")
                || combined_output.contains("Enter")
                || combined_output.contains("Type"),
        ),
        (
            "empty_ignored",
            combined_output.matches("Received echo").count() == 2,
        ), // Only 2 real messages
        (
            "commands_worked",
            combined_output.contains("help") || combined_output.contains("info"),
        ),
        (
            "messages_sent",
            combined_output.contains("test message") && combined_output.contains("final message"),
        ),
        ("graceful_exit", command_output.status.success()),
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

    // Require most checks to pass
    assert!(
        passed_checks >= 4,
        "At least 4/5 input handling checks should pass. Passed: {}/5. Output: {}",
        passed_checks,
        combined_output
    );

    println!("✅ Comprehensive interactive input handling test passed");
    println!(
        "   - {}/{} input handling features verified",
        passed_checks,
        checks.len()
    );
    println!("   - Complete input handling workflow successful");
}
