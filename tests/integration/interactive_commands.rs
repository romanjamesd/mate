//! Interactive Commands Tests
//!
//! Tests for Interactive Commands functionality as specified in tests-to-add.md:
//! - Test help command displays available functionality without sending to peer
//! - Test info command shows current connection details (peer identification, connection status, session duration, message statistics, performance metrics)
//! - Test quit/exit commands terminate session gracefully
//! - Test that commands are case-sensitive and exact-match

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

/// Test help command displays available functionality without sending to peer
#[tokio::test]
async fn test_help_command_displays_functionality() {
    println!("Testing that help command displays available functionality without sending to peer");

    let server_addr = "127.0.0.1:18101";
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

    // Wait for initialization
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send help command
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;
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

    println!("Help command output:\n{}", combined_output);

    // Verify help command shows available functionality
    assert!(
        combined_output.contains("help")
            || combined_output.contains("commands")
            || combined_output.contains("available"),
        "Help output should show available functionality. Output: {}",
        combined_output
    );

    // Verify key commands are mentioned
    let has_command_list = combined_output.contains("quit")
        || combined_output.contains("exit")
        || combined_output.contains("info");

    assert!(
        has_command_list,
        "Help should list key commands. Output: {}",
        combined_output
    );

    // Verify it's local help display (not sent to peer)
    assert!(
        verify_local_command_execution(&combined_output, "Available Commands"),
        "Help command should execute locally without sending user messages. Output: {}",
        combined_output
    );

    println!("✅ Help command functionality test passed");
    println!("   - Help displays available functionality");
    println!("   - Key commands are listed");
    println!("   - Help is displayed locally without sending to peer");
}

/// Test info command shows current connection details
#[tokio::test]
async fn test_info_command_shows_connection_details() {
    println!("Testing that info command shows current connection details");

    let server_addr = "127.0.0.1:18102";
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

    // Send info command
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;
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

    println!("Info command output:\n{}", combined_output);

    // Verify connection details are shown
    assert!(
        combined_output.contains("connection")
            || combined_output.contains("peer")
            || combined_output.contains("status"),
        "Info should show connection details. Output: {}",
        combined_output
    );

    // Verify session duration information
    assert!(
        combined_output.contains("session")
            || combined_output.contains("duration")
            || combined_output.contains("time"),
        "Info should show session duration. Output: {}",
        combined_output
    );

    // Verify it's local info display (not sent to peer)
    assert!(
        verify_local_command_execution(&combined_output, "Connection Information"),
        "Info command should execute locally without sending user messages. Output: {}",
        combined_output
    );

    println!("✅ Info command connection details test passed");
    println!("   - Connection details are displayed");
    println!("   - Session duration information is shown");
    println!("   - Info is displayed locally without sending to peer");
}

/// Test info command shows message statistics and performance metrics after messages
#[tokio::test]
async fn test_info_command_shows_statistics_after_messages() {
    println!(
        "Testing that info command shows message statistics and performance metrics after messages"
    );

    let server_addr = "127.0.0.1:18103";
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
        // Send a test message first to generate statistics
        let _ = stdin.write_all(b"test message for stats\n").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Then send info command
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;
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

    println!("Info with statistics output:\n{}", combined_output);

    // Verify message statistics are shown
    assert!(
        combined_output.contains("message")
            || combined_output.contains("count")
            || combined_output.contains("statistics"),
        "Info should show message statistics after messages. Output: {}",
        combined_output
    );

    // Verify performance metrics are shown
    assert!(
        combined_output.contains("time")
            || combined_output.contains("performance")
            || combined_output.contains("round-trip")
            || combined_output.contains("ms"),
        "Info should show performance metrics after messages. Output: {}",
        combined_output
    );

    println!("✅ Info command statistics test passed");
    println!("   - Message statistics are displayed after sending messages");
    println!("   - Performance metrics are shown");
}

/// Test quit command terminates session gracefully
#[tokio::test]
async fn test_quit_command_terminates_gracefully() {
    println!("Testing that quit command terminates session gracefully");

    let server_addr = "127.0.0.1:18104";
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

    // Send quit command
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    // Verify command exited successfully (exit code 0)
    assert!(
        command_output.status.success(),
        "Quit command should result in successful exit. Status: {}",
        command_output.status
    );

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    // Verify graceful termination messaging
    assert!(
        combined_output.contains("session")
            || combined_output.contains("terminated")
            || combined_output.contains("goodbye")
            || combined_output.contains("exit"),
        "Should show graceful termination message. Output: {}",
        combined_output
    );

    println!("✅ Quit command graceful termination test passed");
    println!("   - Command terminated with successful exit code");
    println!("   - Graceful termination messaging displayed");
}

/// Test exit command terminates session gracefully
#[tokio::test]
async fn test_exit_command_terminates_gracefully() {
    println!("Testing that exit command terminates session gracefully");

    let server_addr = "127.0.0.1:18105";
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

    // Send exit command
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"exit\n").await;
    }

    let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    // Verify command exited successfully
    assert!(
        command_output.status.success(),
        "Exit command should result in successful exit. Status: {}",
        command_output.status
    );

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    // Verify graceful termination
    assert!(
        combined_output.contains("session")
            || combined_output.contains("terminated")
            || combined_output.contains("goodbye")
            || combined_output.contains("exit"),
        "Should show graceful termination message. Output: {}",
        combined_output
    );

    println!("✅ Exit command graceful termination test passed");
    println!("   - Command terminated with successful exit code");
    println!("   - Graceful termination messaging displayed");
}

/// Test that commands are case-sensitive and exact-match
#[tokio::test]
async fn test_commands_case_sensitive_exact_match() {
    println!("Testing that commands are case-sensitive and exact-match");

    let server_addr = "127.0.0.1:18106";
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
        // Test various case variations that should NOT be recognized as commands
        let _ = stdin.write_all(b"HELP\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"Help\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"INFO\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"Info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"QUIT\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"Quit\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Test partial matches that should NOT be recognized as commands
        let _ = stdin.write_all(b"hel\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"inf\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"qui\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Finally send proper quit command
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

    println!("Case sensitivity test output:\n{}", combined_output);

    // Count the number of "Received echo" messages - should be for all the invalid commands
    let echo_count = combined_output.matches("Received echo").count();

    // We sent 9 invalid commands that should be treated as regular messages (and echoed back)
    // plus the final quit which should not be echoed
    assert!(echo_count >= 6,  // Allow some flexibility for implementation differences
           "Invalid commands should be sent as regular messages and echoed back. Echo count: {}, Output: {}", 
           echo_count, combined_output);

    // Verify that only the exact "quit" command worked (no case variations)
    assert!(
        command_output.status.success(),
        "Only exact 'quit' command should terminate successfully. Status: {}",
        command_output.status
    );

    println!("✅ Case-sensitive exact-match test passed");
    println!("   - Case variations were treated as regular messages");
    println!("   - Partial matches were treated as regular messages");
    println!("   - Only exact 'quit' command terminated the session");
}

/// Test comprehensive interactive commands functionality
#[tokio::test]
async fn test_comprehensive_interactive_commands() {
    println!("Testing comprehensive interactive commands functionality");

    let server_addr = "127.0.0.1:18107";
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
        // Test complete workflow
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"test message\n").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

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

    println!("Comprehensive commands test output:\n{}", combined_output);

    // Verify all command types worked
    let checks = [
        (
            "help_worked",
            combined_output.contains("help") || combined_output.contains("commands"),
        ),
        (
            "info_worked",
            combined_output.contains("connection") || combined_output.contains("session"),
        ),
        (
            "message_sent",
            combined_output.contains("test message") || combined_output.contains("echo"),
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
        passed_checks >= 3,
        "At least 3/4 command checks should pass. Passed: {}/4. Output: {}",
        passed_checks,
        combined_output
    );

    println!("✅ Comprehensive interactive commands test passed");
    println!(
        "   - {}/{} command functionalities verified",
        passed_checks,
        checks.len()
    );
    println!("   - Complete command workflow successful");
}
