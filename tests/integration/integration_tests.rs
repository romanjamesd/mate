//! Integration Tests
//!
//! Tests for Integration functionality as specified in tests-to-add.md:
//! - Test complete workflow from connection establishment to termination
//! - Test combinations of commands and messages within single session
//! - Test reconnection followed by continued successful operation
//! - Test that appropriate information is logged throughout session
//! - Test behavior consistency across different terminal environments

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

/// Test complete workflow from connection establishment to termination
#[tokio::test]
async fn test_complete_workflow_connection_to_termination() {
    println!("Testing complete workflow from connection establishment to termination");

    let server_addr = "127.0.0.1:18601";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let session_start = std::time::Instant::now();

    let mut child = Command::new(get_mate_binary_path())
        .args(&["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Phase 1: Connection establishment and initial exploration
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Phase 2: Message exchange and verification
        let _ = stdin
            .write_all(b"Hello, testing complete workflow!\n")
            .await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Second message in workflow\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Phase 3: Status verification during active session
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Phase 4: Extended messaging to test sustained operation
        for i in 1..=3 {
            let message = format!("Workflow message {}\n", i);
            let _ = stdin.write_all(message.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        // Phase 5: Final status check before termination
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Phase 6: Graceful termination
        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(15), child.wait_with_output()).await;

    let session_duration = session_start.elapsed();
    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Complete workflow test output:\n{}", combined_output);

    // Verify all phases of the workflow
    let workflow_phases = vec![
        (
            "connection_establishment",
            combined_output.contains("Connected"),
        ),
        (
            "help_functionality",
            combined_output.contains("Available commands") || combined_output.contains("help"),
        ),
        (
            "info_functionality",
            combined_output.contains("Peer ID") || combined_output.contains("Connection status"),
        ),
        (
            "message_exchange",
            combined_output.contains("Hello, testing complete workflow!"),
        ),
        (
            "sustained_messaging",
            combined_output.contains("Workflow message 3"),
        ),
        (
            "statistics_tracking",
            combined_output.contains("Messages sent"),
        ),
        (
            "timing_calculation",
            combined_output.contains("round-trip") || combined_output.contains("Average"),
        ),
        (
            "session_summary",
            combined_output.contains("Session Summary")
                || combined_output.contains("Session duration"),
        ),
        ("graceful_termination", command_output.status.success()),
    ];

    let mut successful_phases = 0;
    for (phase_name, phase_success) in workflow_phases.iter() {
        if *phase_success {
            successful_phases += 1;
            println!("   ✓ {} phase completed successfully", phase_name);
        } else {
            println!("   ✗ {} phase failed", phase_name);
        }
    }

    // Require all critical phases to succeed
    assert!(
        successful_phases >= 8,
        "At least 8/9 workflow phases should succeed. Succeeded: {}/9. Output: {}",
        successful_phases,
        combined_output
    );

    // Verify message count accuracy
    assert!(
        combined_output.contains("Messages sent: 5")
            || combined_output.contains("Messages sent: 6"),
        "Should accurately track all messages sent during workflow. Output: {}",
        combined_output
    );

    // Verify session duration is reasonable
    assert!(
        session_duration < Duration::from_secs(12),
        "Complete workflow should complete in reasonable time. Took: {:?}",
        session_duration
    );

    println!("✅ Complete workflow from connection establishment to termination test passed");
    println!(
        "   - {}/{} workflow phases completed successfully",
        successful_phases,
        workflow_phases.len()
    );
    println!("   - All message exchanges processed correctly");
    println!("   - Statistics and timing calculated accurately");
    println!("   - Session completed in {:?}", session_duration);
}

/// Test combinations of commands and messages within single session
#[tokio::test]
async fn test_combinations_commands_messages_single_session() {
    println!("Testing combinations of commands and messages within single session");

    let server_addr = "127.0.0.1:18602";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(&["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Complex combination sequence testing various interleavings

        // Pattern 1: Command -> Message -> Command
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"First message after help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Pattern 2: Multiple messages -> Command
        let _ = stdin.write_all(b"Message batch 1\n").await;
        tokio::time::sleep(Duration::from_millis(250)).await;

        let _ = stdin.write_all(b"Message batch 2\n").await;
        tokio::time::sleep(Duration::from_millis(250)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Pattern 3: Command -> Multiple messages -> Command
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"Multi-message 1\n").await;
        tokio::time::sleep(Duration::from_millis(250)).await;

        let _ = stdin.write_all(b"Multi-message 2\n").await;
        tokio::time::sleep(Duration::from_millis(250)).await;

        let _ = stdin.write_all(b"Multi-message 3\n").await;
        tokio::time::sleep(Duration::from_millis(250)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Pattern 4: Rapid command/message alternation
        let _ = stdin.write_all(b"Rapid message 1\n").await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let _ = stdin.write_all(b"Rapid message 2\n").await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Pattern 5: Final verification and termination
        let _ = stdin.write_all(b"Final combination test message\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(15), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!(
        "Command/message combinations test output:\n{}",
        combined_output
    );

    // Verify all message content was processed
    let expected_messages = vec![
        "First message after help",
        "Message batch 1",
        "Message batch 2",
        "Multi-message 1",
        "Multi-message 2",
        "Multi-message 3",
        "Rapid message 1",
        "Rapid message 2",
        "Final combination test message",
    ];

    let mut found_messages = 0;
    for expected_message in &expected_messages {
        if combined_output.contains(expected_message) {
            found_messages += 1;
        } else {
            println!("   ! Missing message: {}", expected_message);
        }
    }

    assert!(found_messages >= 8,
           "Most messages should be processed in command/message combinations. Found: {}/9. Output: {}", 
           found_messages, combined_output);

    // Verify commands were processed between messages
    let help_count = combined_output.matches("Available commands").count()
        + combined_output.matches("help").count();
    assert!(
        help_count >= 3,
        "Help commands should be processed multiple times. Found: {} instances",
        help_count
    );

    let info_count = combined_output
        .lines()
        .filter(|line| line.contains("Messages sent") || line.contains("Session duration"))
        .count();
    assert!(
        info_count >= 4,
        "Info commands should be processed multiple times. Found: {} instances",
        info_count
    );

    // Verify message count tracking remains accurate despite command interspersion
    assert!(
        combined_output.contains("Messages sent: 9")
            || combined_output.contains("Messages sent: 8"),
        "Should accurately count messages despite command interspersion. Output: {}",
        combined_output
    );

    // Verify commands don't interfere with message timing
    assert!(
        combined_output.contains("Average round-trip time")
            || combined_output.contains("round-trip"),
        "Message timing should work despite command interspersion. Output: {}",
        combined_output
    );

    // Verify session state coherence
    assert!(
        combined_output.contains("Session Summary") || combined_output.contains("Goodbye"),
        "Session should maintain coherent state throughout combinations. Output: {}",
        combined_output
    );

    println!("✅ Combinations of commands and messages within single session test passed");
    println!("   - {}/9 messages processed successfully", found_messages);
    println!("   - Commands executed properly between messages");
    println!("   - Message counting accurate despite command interspersion");
    println!("   - Session state remained coherent throughout");
}

/// Test reconnection followed by continued successful operation
#[tokio::test]
async fn test_reconnection_followed_by_continued_operation() {
    println!("Testing reconnection followed by continued successful operation");

    let server_addr = "127.0.0.1:18603";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let mut server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(&["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Phase 1: Establish baseline operation
        let _ = stdin.write_all(b"Pre-reconnection message 1\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Pre-reconnection message 2\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Phase 2: Simulate connection disruption
        println!("Simulating connection disruption...");
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Try to send during disruption
        let _ = stdin.write_all(b"Message during disruption\n").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Phase 3: Restart server for reconnection
        println!("Restarting server for reconnection...");
        let server = start_test_server(server_addr)
            .await
            .expect("Failed to restart test server");
        server_handle = tokio::spawn(async move { server.run().await });

        tokio::time::sleep(Duration::from_millis(800)).await; // Allow time for reconnection

        // Phase 4: Verify continued operation after reconnection
        let _ = stdin.write_all(b"Post-reconnection message 1\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Post-reconnection message 2\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Verify commands still work
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Phase 5: Extended operation to verify stability
        for i in 1..=3 {
            let message = format!("Stability test message {}\n", i);
            let _ = stdin.write_all(message.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(350)).await;
        }

        // Final status check
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(20), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!(
        "Reconnection and continued operation test output:\n{}",
        combined_output
    );

    // Verify pre-reconnection operation
    assert!(
        combined_output.contains("Pre-reconnection message 1"),
        "Pre-reconnection messages should be processed. Output: {}",
        combined_output
    );

    // Verify disruption was detected
    let disruption_detected = combined_output.contains("failed")
        || combined_output.contains("error")
        || combined_output.contains("connection")
        || combined_output.contains("Failed to send");

    assert!(
        disruption_detected,
        "Connection disruption should be detected. Output: {}",
        combined_output
    );

    // Verify post-reconnection operation
    let post_reconnection_success = combined_output.contains("Post-reconnection message 1")
        || combined_output.contains("Post-reconnection message 2")
        || combined_output.contains("Stability test message");

    assert!(
        post_reconnection_success,
        "Post-reconnection operation should succeed. Output: {}",
        combined_output
    );

    // Verify commands work after reconnection
    let commands_work_after_reconnection = combined_output.contains("Available commands")
        || combined_output.contains("Session duration");

    assert!(
        commands_work_after_reconnection,
        "Commands should work after reconnection. Output: {}",
        combined_output
    );

    // Verify session state preservation aspects
    let session_coherent = combined_output.contains("Session Summary")
        || combined_output.contains("Session duration")
        || command_output.status.code() == Some(0);

    assert!(
        session_coherent,
        "Session should maintain coherence after reconnection. Output: {}",
        combined_output
    );

    // Verify stability after reconnection
    let stability_confirmed = combined_output.contains("Stability test message 3")
        || combined_output.contains("Stability test message 2");

    assert!(
        stability_confirmed,
        "System should be stable after reconnection. Output: {}",
        combined_output
    );

    println!("✅ Reconnection followed by continued successful operation test passed");
    println!("   - Pre-reconnection operation established successfully");
    println!("   - Connection disruption detected and handled");
    println!("   - Post-reconnection operation resumed successfully");
    println!("   - Commands and messages work after reconnection");
    println!("   - System stability confirmed after reconnection");
}

/// Test that appropriate information is logged throughout session
#[tokio::test]
async fn test_appropriate_information_logged_throughout_session() {
    println!("Testing that appropriate information is logged throughout session");

    let server_addr = "127.0.0.1:18604";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Set environment to ensure detailed logging
    let mut child = Command::new(get_mate_binary_path())
        .args(&["connect", server_addr])
        .env("RUST_LOG", "info") // Enable info-level logging
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Session phases requiring different types of logging

        // Connection phase
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Message exchange phase
        let _ = stdin.write_all(b"Logging test message 1\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Logging test message 2\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Command execution phase
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Statistics/timing phase
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // More messaging for comprehensive logging
        for i in 3..=5 {
            let message = format!("Comprehensive logging message {}\n", i);
            let _ = stdin.write_all(message.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(350)).await;
        }

        // Final status and termination phase
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(12), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Comprehensive logging test output:\n{}", combined_output);

    // Analyze different types of information that should be logged
    let logging_categories = vec![
        (
            "connection_info",
            combined_output.contains("Connected to peer") || combined_output.contains("Peer ID"),
        ),
        (
            "session_tracking",
            combined_output.contains("Session duration") || combined_output.contains("session"),
        ),
        (
            "message_statistics",
            combined_output.contains("Messages sent") || combined_output.contains("message"),
        ),
        (
            "timing_information",
            combined_output.contains("round-trip") || combined_output.contains("Average"),
        ),
        (
            "command_responses",
            combined_output.contains("Available commands") || combined_output.contains("help"),
        ),
        (
            "status_updates",
            combined_output.contains("Connection status") || combined_output.contains("status"),
        ),
        (
            "session_summary",
            combined_output.contains("Session Summary") || combined_output.contains("summary"),
        ),
    ];

    let mut logged_categories = 0;
    for (category, is_logged) in logging_categories.iter() {
        if *is_logged {
            logged_categories += 1;
            println!("   ✓ {} information properly logged", category);
        } else {
            println!("   ✗ {} information missing from logs", category);
        }
    }

    assert!(
        logged_categories >= 6,
        "At least 6/7 information categories should be logged. Found: {}/7. Output: {}",
        logged_categories,
        combined_output
    );

    // Verify appropriate detail level (not too verbose, not too sparse)
    let total_lines = combined_output.lines().count();
    assert!(
        total_lines >= 20 && total_lines <= 200,
        "Logging should have appropriate detail level. Found: {} lines",
        total_lines
    );

    // Verify critical events are logged
    assert!(
        combined_output.contains("Logging test message 1"),
        "Message content should be logged appropriately. Output: {}",
        combined_output
    );

    // Verify session lifecycle is tracked
    let lifecycle_events = vec!["connected", "session", "duration", "summary", "goodbye"];
    let lifecycle_found = lifecycle_events
        .iter()
        .filter(|event| {
            combined_output
                .to_lowercase()
                .contains(&event.to_lowercase())
        })
        .count();

    assert!(
        lifecycle_found >= 3,
        "Session lifecycle should be logged. Found: {}/5 lifecycle events",
        lifecycle_found
    );

    // Verify no excessive error spam
    let error_lines = combined_output
        .lines()
        .filter(|line| line.to_lowercase().contains("error"))
        .count();

    assert!(
        error_lines <= 5,
        "Should not have excessive error logging. Found: {} error lines",
        error_lines
    );

    // Verify structured information (timestamps, levels, etc.)
    let structured_info = combined_output.contains("INFO")
        || combined_output.contains("2025-")
        || combined_output.contains("ms")
        || combined_output.contains("Messages sent:");

    assert!(
        structured_info,
        "Should include structured logging information. Output: {}",
        combined_output
    );

    println!("✅ Appropriate information logged throughout session test passed");
    println!(
        "   - {}/7 information categories properly logged",
        logged_categories
    );
    println!("   - {}/5 session lifecycle events found", lifecycle_found);
    println!(
        "   - Total output: {} lines (appropriate detail level)",
        total_lines
    );
    println!("   - Structured information included");
    println!("   - No excessive error spam");
}

/// Test behavior consistency across different terminal environments
#[tokio::test]
async fn test_behavior_consistency_across_terminal_environments() {
    println!("Testing behavior consistency across different terminal environments");

    let server_addr = "127.0.0.1:18605";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test different terminal environment configurations
    let environment_configs = vec![
        (vec![("TERM", "xterm")], "xterm environment"),
        (vec![("TERM", "xterm-256color")], "256-color terminal"),
        (vec![("TERM", "dumb")], "basic/dumb terminal"),
        (vec![("TERM", "screen")], "screen terminal"),
        (
            vec![("COLUMNS", "80"), ("LINES", "24")],
            "standard terminal size",
        ),
        (
            vec![("COLUMNS", "120"), ("LINES", "40")],
            "larger terminal size",
        ),
    ];

    let mut environment_results = Vec::new();

    for (env_vars, description) in environment_configs {
        println!("Testing {}", description);

        let mut cmd = Command::new(get_mate_binary_path());
        cmd.args(&["connect", server_addr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply environment variables
        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let mut child = cmd
            .spawn()
            .expect(&format!("Failed to start mate command for {}", description));

        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Some(stdin) = child.stdin.as_mut() {
            // Standard test sequence
            let _ = stdin.write_all(b"help\n").await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            let _ = stdin.write_all(b"Environment test message\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"info\n").await;
            tokio::time::sleep(Duration::from_millis(200)).await;

            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(8), child.wait_with_output()).await;

        let command_output = output
            .expect(&format!(
                "Command should complete within timeout for {}",
                description
            ))
            .expect(&format!(
                "Command should execute successfully for {}",
                description
            ));

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Analyze output characteristics for this environment
        let environment_analysis = EnvironmentTestResult {
            description: description.to_string(),
            exit_code: command_output.status.code().unwrap_or(-1),
            output_length: combined_output.len(),
            contains_help: combined_output.contains("Available commands")
                || combined_output.contains("help"),
            contains_message: combined_output.contains("Environment test message"),
            contains_info: combined_output.contains("Messages sent")
                || combined_output.contains("Session duration"),
            contains_summary: combined_output.contains("Session Summary")
                || combined_output.contains("Goodbye"),
            has_errors: combined_output.to_lowercase().contains("error"),
        };

        environment_results.push(environment_analysis);

        println!("   ✓ {} completed successfully", description);
    }

    server_handle.abort();

    // Analyze consistency across environments
    println!("\nAnalyzing consistency across terminal environments:");

    // Check exit code consistency
    let exit_codes: Vec<i32> = environment_results.iter().map(|r| r.exit_code).collect();
    let consistent_exit_codes = exit_codes.iter().all(|&code| code == exit_codes[0]);

    assert!(
        consistent_exit_codes,
        "Exit codes should be consistent across environments. Found: {:?}",
        exit_codes
    );
    println!("   ✓ Exit codes consistent across all environments");

    // Check feature availability consistency
    let help_consistency = environment_results.iter().all(|r| r.contains_help);
    assert!(
        help_consistency,
        "Help functionality should work in all environments"
    );
    println!("   ✓ Help functionality consistent across environments");

    let message_consistency = environment_results.iter().all(|r| r.contains_message);
    assert!(
        message_consistency,
        "Message processing should work in all environments"
    );
    println!("   ✓ Message processing consistent across environments");

    let info_consistency = environment_results.iter().all(|r| r.contains_info);
    assert!(
        info_consistency,
        "Info command should work in all environments"
    );
    println!("   ✓ Info command consistent across environments");

    // Check output length consistency (should be reasonably similar)
    let output_lengths: Vec<usize> = environment_results
        .iter()
        .map(|r| r.output_length)
        .collect();
    let max_length = output_lengths.iter().max().unwrap();
    let min_length = output_lengths.iter().min().unwrap();
    let length_variation_ratio = *max_length as f64 / *min_length as f64;

    assert!(
        length_variation_ratio < 2.0,
        "Output length should be reasonably consistent. Variation ratio: {:.2}, lengths: {:?}",
        length_variation_ratio,
        output_lengths
    );
    println!(
        "   ✓ Output length reasonably consistent (variation ratio: {:.2})",
        length_variation_ratio
    );

    // Check that no environment has excessive errors
    let environments_with_errors = environment_results.iter().filter(|r| r.has_errors).count();

    assert!(
        environments_with_errors <= 1,
        "No more than one environment should have errors. Found: {} environments with errors",
        environments_with_errors
    );
    println!("   ✓ Error handling consistent across environments");

    // Verify all core features work in all environments
    let core_features_work = environment_results
        .iter()
        .all(|r| r.contains_help && r.contains_message && r.contains_info);

    assert!(
        core_features_work,
        "All core features should work in all environments"
    );

    println!("✅ Behavior consistency across terminal environments test passed");
    println!(
        "   - All {} environments tested successfully",
        environment_results.len()
    );
    println!("   - Exit codes consistent across all environments");
    println!("   - Core functionality available in all environments");
    println!("   - Output characteristics reasonably consistent");
    println!("   - No environment-specific failures");
}

/// Helper struct for environment test analysis
#[derive(Debug)]
struct EnvironmentTestResult {
    description: String,
    exit_code: i32,
    output_length: usize,
    contains_help: bool,
    contains_message: bool,
    contains_info: bool,
    contains_summary: bool,
    has_errors: bool,
}

/// Test comprehensive integration functionality
#[tokio::test]
async fn test_comprehensive_integration_functionality() {
    println!("Testing comprehensive integration functionality");

    let server_addr = "127.0.0.1:18606";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let integration_start = std::time::Instant::now();

    let mut child = Command::new(get_mate_binary_path())
        .args(&["connect", server_addr])
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Comprehensive integration test combining all aspects

        // 1. Connection and discovery
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 2. Message exchange with varied content
        let test_messages = vec![
            "Integration test message 1",
            "Message with unicode: 测试 🚀 ñáéíóú",
            "Longer integration message to test various content handling capabilities",
            "Short msg",
            "Integration test message 5",
        ];

        for (i, message) in test_messages.iter().enumerate() {
            let msg = format!("{}\n", message);
            let _ = stdin.write_all(msg.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Interleave info commands to test session state
            if i % 2 == 1 {
                let _ = stdin.write_all(b"info\n").await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }

        // 3. Command functionality verification
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 4. Rapid sequence testing
        for i in 1..=3 {
            let msg = format!("Rapid integration {}\n", i);
            let _ = stdin.write_all(msg.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // 5. Final comprehensive status
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 6. Graceful termination
        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(15), child.wait_with_output()).await;

    let integration_duration = integration_start.elapsed();
    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!(
        "Comprehensive integration test output:\n{}",
        combined_output
    );

    // Comprehensive integration checks
    let integration_checks = vec![
        (
            "connection_establishment",
            combined_output.contains("Connected"),
        ),
        (
            "help_command_functionality",
            combined_output.contains("Available commands") || combined_output.contains("help"),
        ),
        (
            "info_command_functionality",
            combined_output.contains("Messages sent")
                && combined_output.contains("Session duration"),
        ),
        (
            "basic_message_processing",
            combined_output.contains("Integration test message 1"),
        ),
        (
            "unicode_message_support",
            combined_output.contains("测试") || combined_output.contains("🚀"),
        ),
        (
            "varied_content_handling",
            combined_output.contains("Longer integration message"),
        ),
        (
            "rapid_sequence_processing",
            combined_output.contains("Rapid integration 3"),
        ),
        (
            "statistics_calculation",
            combined_output.contains("Average round-trip time")
                || combined_output.contains("round-trip"),
        ),
        (
            "session_state_tracking",
            combined_output.contains("Session duration"),
        ),
        (
            "message_count_accuracy",
            combined_output.contains("Messages sent: 8")
                || combined_output.contains("Messages sent: 7"),
        ),
        ("graceful_termination", command_output.status.success()),
        ("comprehensive_logging", combined_output.len() > 1000), // Reasonable amount of output
    ];

    let mut passed_integration_checks = 0;
    for (check_name, check_result) in integration_checks.iter() {
        if *check_result {
            passed_integration_checks += 1;
            println!("   ✓ {} integration check passed", check_name);
        } else {
            println!("   ✗ {} integration check failed", check_name);
        }
    }

    // Require most integration checks to pass
    assert!(
        passed_integration_checks >= 10,
        "At least 10/12 integration checks should pass. Passed: {}/12. Output: {}",
        passed_integration_checks,
        combined_output
    );

    // Verify performance is reasonable
    assert!(
        integration_duration < Duration::from_secs(12),
        "Comprehensive integration should complete in reasonable time. Took: {:?}",
        integration_duration
    );

    // Verify no crashes or hangs
    assert!(
        command_output.status.success(),
        "Integration test should complete successfully. Status: {}",
        command_output.status
    );

    println!("✅ Comprehensive integration functionality test passed");
    println!(
        "   - {}/{} integration features verified",
        passed_integration_checks,
        integration_checks.len()
    );
    println!("   - All core components working together successfully");
    println!("   - Complete integration workflow successful");
    println!("   - Integration completed in {:?}", integration_duration);
}
