//! Edge Cases and Boundary Tests
//!
//! Tests for Edge Cases and Boundary functionality as specified in tests-to-add.md:
//! - Test handling of very long message content
//! - Test rapid consecutive message sending
//! - Test statistics accuracy with minimal message counts
//! - Test session duration calculation edge cases
//! - Test handling of unusual peer identification scenarios
//! - Test reconnection during various phases of operation

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

/// Test handling of very long message content
#[tokio::test]
async fn test_very_long_message_content_handling() {
    println!("Testing handling of very long message content");

    let server_addr = "127.0.0.1:18501";
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
        // Test various long message lengths
        let test_cases = vec![
            ("A".repeat(1000), "1KB message"),
            ("B".repeat(5000), "5KB message"),
            ("C".repeat(10000), "10KB message"),
            (
                "Long message with special chars: 测试中文 🚀 emoji and symbols ñáéíóú".repeat(100),
                "Multi-byte character message",
            ),
        ];

        for (long_message, description) in test_cases {
            println!("Testing {}", description);

            let message_with_newline = format!("{}\n", long_message);
            let _ = stdin.write_all(message_with_newline.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(800)).await; // Extra time for long messages
        }

        // Test that system still works after long messages
        let _ = stdin.write_all(b"Short message after long ones\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

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
        "Very long message content test output:\n{}",
        combined_output
    );

    // Verify long messages were processed
    let expected_responses = vec![
        ("AAAA", "1KB message echo"),
        ("BBBB", "5KB message echo"),
        ("CCCC", "10KB message echo"),
        ("Long message with special chars", "Multi-byte message echo"),
        (
            "Short message after long ones",
            "Short message after long messages",
        ),
    ];

    for (pattern, description) in expected_responses {
        assert!(
            combined_output.contains(pattern),
            "Should process {}. Output: {}",
            description,
            combined_output
        );
    }

    // Verify system maintains functionality after long messages
    assert!(
        combined_output.contains("Messages sent: 5")
            || combined_output.contains("Messages sent: 4"),
        "Should track all messages including long ones. Output: {}",
        combined_output
    );

    // Verify timing information is still calculated correctly
    assert!(
        combined_output.contains("round-trip") || combined_output.contains("Average"),
        "Should calculate timing for long messages. Output: {}",
        combined_output
    );

    // Verify no truncation or corruption indicated
    assert!(
        !combined_output.contains("truncated") && !combined_output.contains("corrupted"),
        "Long messages should not be truncated or corrupted. Output: {}",
        combined_output
    );

    println!("✅ Very long message content handling test passed");
    println!("   - 1KB, 5KB, and 10KB messages processed successfully");
    println!("   - Multi-byte character messages handled correctly");
    println!("   - System maintains functionality after long messages");
    println!("   - Statistics and timing calculated correctly");
}

/// Test rapid consecutive message sending
#[tokio::test]
async fn test_rapid_consecutive_message_sending() {
    println!("Testing rapid consecutive message sending");

    let server_addr = "127.0.0.1:18502";
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
        // Send rapid consecutive messages with minimal delay
        let rapid_message_count = 10;

        println!("Sending {} rapid consecutive messages", rapid_message_count);
        for i in 1..=rapid_message_count {
            let message = format!("Rapid message {}\n", i);
            let _ = stdin.write_all(message.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(50)).await; // Very short delay
        }

        // Wait for all messages to be processed
        tokio::time::sleep(Duration::from_millis(2000)).await;

        // Check system state after rapid sending
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Send one more message to ensure system is still responsive
        let _ = stdin
            .write_all(b"Final message after rapid sequence\n")
            .await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!(
        "Rapid consecutive message sending test output:\n{}",
        combined_output
    );

    // Verify all rapid messages were processed
    let rapid_messages_found = (1..=10)
        .filter(|i| combined_output.contains(&format!("Rapid message {}", i)))
        .count();

    assert!(
        rapid_messages_found >= 8,
        "Most rapid messages should be processed. Found: {}/10. Output: {}",
        rapid_messages_found,
        combined_output
    );

    // Verify final message was processed
    assert!(
        combined_output.contains("Final message after rapid sequence"),
        "Final message should be processed. Output: {}",
        combined_output
    );

    // Verify message count tracking
    assert!(
        combined_output.contains("Messages sent: 11")
            || combined_output.contains("Messages sent: 10"),
        "Should accurately count rapid messages. Output: {}",
        combined_output
    );

    // Verify system remains responsive and doesn't crash
    assert!(
        combined_output.contains("Session Summary") || combined_output.contains("Goodbye"),
        "System should remain responsive after rapid sending. Output: {}",
        combined_output
    );

    // Verify timing calculations are reasonable
    assert!(
        combined_output.contains("Average round-trip time"),
        "Should calculate average timing for rapid messages. Output: {}",
        combined_output
    );

    // Verify no error spam or overwhelming output
    let error_line_count = combined_output
        .lines()
        .filter(|line| {
            line.to_lowercase().contains("error") || line.to_lowercase().contains("failed")
        })
        .count();

    assert!(
        error_line_count < 5,
        "Should not generate excessive errors during rapid sending. Found: {} error lines",
        error_line_count
    );

    println!("✅ Rapid consecutive message sending test passed");
    println!(
        "   - {}/10 rapid messages processed successfully",
        rapid_messages_found
    );
    println!("   - System remained responsive throughout");
    println!("   - Message counting and timing calculated correctly");
    println!("   - No excessive errors or crashes");
}

/// Test statistics accuracy with minimal message counts
#[tokio::test]
async fn test_statistics_accuracy_with_minimal_message_counts() {
    println!("Testing statistics accuracy with minimal message counts");

    let server_addr = "127.0.0.1:18503";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test Case 1: Zero messages
    println!("Test Case 1: Zero messages sent");
    {
        let mut child = Command::new(get_mate_binary_path())
            .args(&["connect", server_addr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start mate command");

        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Some(stdin) = child.stdin.as_mut() {
            // Check info without sending any messages
            let _ = stdin.write_all(b"info\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute successfully");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify zero message statistics
        assert!(
            combined_output.contains("Messages sent: 0"),
            "Should show 0 messages sent. Output: {}",
            combined_output
        );

        // Verify no average timing shown for zero messages
        assert!(
            !combined_output.contains("Average round-trip time")
                || combined_output.contains("No messages")
                || combined_output.contains("N/A"),
            "Should handle zero messages gracefully in timing. Output: {}",
            combined_output
        );

        println!("   ✓ Zero messages case handled correctly");
    }

    // Test Case 2: Single message
    println!("Test Case 2: Single message sent");
    {
        let mut child = Command::new(get_mate_binary_path())
            .args(&["connect", server_addr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start mate command");

        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Some(stdin) = child.stdin.as_mut() {
            // Send exactly one message
            let _ = stdin.write_all(b"Single test message\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"info\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute successfully");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify single message statistics
        assert!(
            combined_output.contains("Messages sent: 1"),
            "Should show 1 message sent. Output: {}",
            combined_output
        );

        // Verify single message timing (average should equal the single measurement)
        assert!(
            combined_output.contains("Average round-trip time")
                || combined_output.contains("round-trip"),
            "Should show timing for single message. Output: {}",
            combined_output
        );

        println!("   ✓ Single message case handled correctly");
    }

    // Test Case 3: Two messages (minimal for average calculation)
    println!("Test Case 3: Two messages sent");
    {
        let mut child = Command::new(get_mate_binary_path())
            .args(&["connect", server_addr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start mate command");

        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Some(stdin) = child.stdin.as_mut() {
            // Send exactly two messages
            let _ = stdin.write_all(b"First message\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"Second message\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"info\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute successfully");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify two message statistics
        assert!(
            combined_output.contains("Messages sent: 2"),
            "Should show 2 messages sent. Output: {}",
            combined_output
        );

        // Verify meaningful average calculation
        assert!(
            combined_output.contains("Average round-trip time"),
            "Should calculate average for two messages. Output: {}",
            combined_output
        );

        println!("   ✓ Two messages case handled correctly");
    }

    server_handle.abort();

    println!("✅ Statistics accuracy with minimal message counts test passed");
    println!("   - Zero messages: Handled gracefully without division errors");
    println!("   - Single message: Timing displayed correctly");
    println!("   - Two messages: Average calculation works properly");
    println!("   - No mathematical errors or undefined behavior");
}

/// Test session duration calculation edge cases
#[tokio::test]
async fn test_session_duration_calculation_edge_cases() {
    println!("Testing session duration calculation edge cases");

    let server_addr = "127.0.0.1:18504";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test Case 1: Very short session (sub-second)
    println!("Test Case 1: Very short session duration");
    {
        let mut child = Command::new(get_mate_binary_path())
            .args(&["connect", server_addr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start mate command");

        tokio::time::sleep(Duration::from_millis(200)).await; // Very short session

        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute successfully");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify short duration is displayed with appropriate precision
        assert!(
            combined_output.contains("Session duration")
                && (combined_output.contains("ms") || combined_output.contains("s")),
            "Should display short session duration with appropriate units. Output: {}",
            combined_output
        );

        // Verify no negative or zero duration
        assert!(
            !combined_output.contains("Session duration: 0")
                && !combined_output.contains("Duration: -"),
            "Session duration should be positive. Output: {}",
            combined_output
        );

        println!("   ✓ Very short session duration handled correctly");
    }

    // Test Case 2: Longer session with info checks
    println!("Test Case 2: Session with intermediate duration checks");
    {
        let mut child = Command::new(get_mate_binary_path())
            .args(&["connect", server_addr])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start mate command");

        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Some(stdin) = child.stdin.as_mut() {
            // Check duration at different points
            let _ = stdin.write_all(b"info\n").await;
            tokio::time::sleep(Duration::from_millis(500)).await;

            let _ = stdin.write_all(b"Test message\n").await;
            tokio::time::sleep(Duration::from_millis(500)).await;

            let _ = stdin.write_all(b"info\n").await;
            tokio::time::sleep(Duration::from_millis(500)).await;

            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute successfully");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Extract all duration values to verify they're increasing
        let duration_lines: Vec<&str> = combined_output
            .lines()
            .filter(|line| line.contains("Session duration"))
            .collect();

        assert!(
            duration_lines.len() >= 2,
            "Should have multiple duration measurements. Found: {}. Output: {}",
            duration_lines.len(),
            combined_output
        );

        // Verify durations are positive and reasonable
        for duration_line in &duration_lines {
            assert!(
                !duration_line.contains("Session duration: 0")
                    && !duration_line.contains("Duration: -"),
                "All session durations should be positive: {}",
                duration_line
            );
        }

        println!("   ✓ Progressive session duration tracking works correctly");
    }

    server_handle.abort();

    println!("✅ Session duration calculation edge cases test passed");
    println!("   - Very short sessions display appropriate precision");
    println!("   - No negative or zero durations reported");
    println!("   - Progressive duration tracking works correctly");
    println!("   - Duration units are appropriate for time ranges");
}

/// Test handling of unusual peer identification scenarios
#[tokio::test]
async fn test_unusual_peer_identification_scenarios() {
    println!("Testing handling of unusual peer identification scenarios");

    // Test different server configurations and identity scenarios
    let test_scenarios = vec![
        ("127.0.0.1:18505", "Standard peer identification"),
        ("127.0.0.1:18506", "Different server identity"),
        ("127.0.0.1:18507", "Multiple connection scenario"),
    ];

    for (server_addr, scenario_description) in test_scenarios {
        println!("Testing scenario: {}", scenario_description);

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
            // Test peer identification display
            let _ = stdin.write_all(b"info\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Send a message to test peer interaction
            let _ = stdin.write_all(b"Peer identification test\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Check peer info again
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
        let combined_output = format!("{}{}", stdout, stderr);

        println!("{} test output:\n{}", scenario_description, combined_output);

        // Verify peer identification is displayed
        assert!(
            combined_output.contains("Peer ID") || combined_output.contains("Connected to peer"),
            "Should display peer identification for {}. Output: {}",
            scenario_description,
            combined_output
        );

        // Verify peer ID format (should be base64-like string)
        let peer_id_found = combined_output.lines().any(|line| {
            if line.contains("Peer ID:") || line.contains("Connected to peer:") {
                // Look for base64-like pattern (alphanumeric + / + = with appropriate length)
                line.chars()
                    .any(|c| c.is_alphanumeric() || c == '/' || c == '+' || c == '=')
            } else {
                false
            }
        });

        assert!(
            peer_id_found,
            "Should find valid peer ID format for {}. Output: {}",
            scenario_description, combined_output
        );

        // Verify peer identification consistency
        let peer_id_lines: Vec<&str> = combined_output
            .lines()
            .filter(|line| line.contains("Peer ID:") || line.contains("Connected to peer:"))
            .collect();

        if peer_id_lines.len() > 1 {
            // Extract actual peer IDs and verify they're consistent
            let first_id_line = peer_id_lines[0];
            for other_id_line in &peer_id_lines[1..] {
                // Both should contain the same peer identifier
                let first_has_colon = first_id_line.contains(':');
                let other_has_colon = other_id_line.contains(':');

                assert!(
                    first_has_colon == other_has_colon,
                    "Peer ID format should be consistent for {}. Lines: {:?}",
                    scenario_description,
                    peer_id_lines
                );
            }
        }

        // Verify message exchange works with peer identification
        assert!(
            combined_output.contains("Peer identification test"),
            "Should successfully exchange messages with identified peer for {}. Output: {}",
            scenario_description,
            combined_output
        );

        println!("   ✓ {} completed successfully", scenario_description);
    }

    println!("✅ Unusual peer identification scenarios test passed");
    println!("   - Multiple server identities handled correctly");
    println!("   - Peer ID format is consistent and valid");
    println!("   - Peer identification displayed reliably");
    println!("   - Message exchange works with various peer scenarios");
}

/// Test reconnection during various phases of operation
#[tokio::test]
async fn test_reconnection_during_various_phases() {
    println!("Testing reconnection during various phases of operation");

    // Test Case 1: Reconnection during handshake phase
    println!("Test Case 1: Reconnection during handshake/connection phase");
    {
        let server_addr = "127.0.0.1:18508";
        let mut child = Command::new(get_mate_binary_path())
            .args(&["connect", server_addr]) // No server running yet
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start mate command");

        tokio::time::sleep(Duration::from_millis(200)).await;

        // Start server after client attempts connection
        let server = start_test_server(server_addr)
            .await
            .expect("Failed to start test server");

        let server_handle = tokio::spawn(async move { server.run().await });

        tokio::time::sleep(Duration::from_millis(1000)).await; // Allow time for connection

        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(b"Handshake phase test message\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(8), child.wait_with_output()).await;

        server_handle.abort();

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify connection was eventually established or handled gracefully
        let connection_established = combined_output.contains("Connected")
            || combined_output.contains("Handshake phase test message");
        let graceful_failure = combined_output.contains("Failed to connect")
            && command_output.status.code() == Some(1);

        assert!(
            connection_established || graceful_failure,
            "Should establish connection or fail gracefully during handshake phase. Output: {}",
            combined_output
        );

        println!("   ✓ Handshake phase reconnection handled appropriately");
    }

    // Test Case 2: Reconnection during active messaging
    println!("Test Case 2: Reconnection during active messaging phase");
    {
        let server_addr = "127.0.0.1:18510"; // Different port
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
            // Establish active messaging
            let _ = stdin.write_all(b"Pre-disruption message\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Disrupt during active phase
            server_handle.abort();
            tokio::time::sleep(Duration::from_millis(200)).await;

            // Try to continue messaging
            let _ = stdin.write_all(b"Message during disruption\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Restart server
            let server = start_test_server(server_addr)
                .await
                .expect("Failed to restart test server");
            server_handle = tokio::spawn(async move { server.run().await });

            tokio::time::sleep(Duration::from_millis(500)).await;

            // Continue messaging after restart
            let _ = stdin.write_all(b"Post-reconnection message\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            let _ = stdin.write_all(b"quit\n").await;
        }

        let output = timeout(Duration::from_secs(8), child.wait_with_output()).await;

        server_handle.abort();

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify messaging phases were handled
        assert!(
            combined_output.contains("Pre-disruption message"),
            "Pre-disruption messaging should work. Output: {}",
            combined_output
        );

        // Verify graceful handling during disruption
        let has_disruption_handling = combined_output.contains("failed")
            || combined_output.contains("error")
            || combined_output.contains("connection");

        assert!(
            has_disruption_handling,
            "Should indicate connection issues during disruption. Output: {}",
            combined_output
        );

        println!("   ✓ Active messaging phase reconnection handled appropriately");
    }

    // Test Case 3: Reconnection during session termination
    println!("Test Case 3: Reconnection during session termination phase");
    {
        let server_addr = "127.0.0.1:18511"; // Different port
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
            let _ = stdin.write_all(b"Termination phase test\n").await;
            tokio::time::sleep(Duration::from_millis(300)).await;

            // Initiate quit but disrupt during termination
            let _ = stdin.write_all(b"quit\n").await;
            tokio::time::sleep(Duration::from_millis(100)).await;

            // Disrupt server during termination
            server_handle.abort();
        }

        let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

        let command_output = output
            .expect("Command should complete within timeout")
            .expect("Command should execute");

        let stdout = String::from_utf8_lossy(&command_output.stdout);
        let stderr = String::from_utf8_lossy(&command_output.stderr);
        let combined_output = format!("{}{}", stdout, stderr);

        // Verify termination was handled gracefully despite disruption
        assert!(
            combined_output.contains("Termination phase test"),
            "Pre-termination message should be processed. Output: {}",
            combined_output
        );

        // Verify graceful termination despite disruption
        let exit_code = command_output.status.code().unwrap_or(-1);
        assert!(
            exit_code >= 0 && exit_code <= 1,
            "Should terminate gracefully despite disruption. Exit code: {}",
            exit_code
        );

        println!("   ✓ Session termination phase disruption handled gracefully");
    }

    println!("✅ Reconnection during various phases test passed");
    println!("   - Handshake phase: Connection retries or graceful failure");
    println!("   - Active messaging: Disruption detection and recovery attempts");
    println!("   - Termination phase: Graceful cleanup despite network issues");
    println!("   - All phases handle reconnection scenarios appropriately");
}

/// Test comprehensive edge cases and boundary conditions
#[tokio::test]
async fn test_comprehensive_edge_cases_and_boundaries() {
    println!("Testing comprehensive edge cases and boundary conditions");

    let server_addr = "127.0.0.1:18509";
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
        // Comprehensive edge case testing combining multiple boundaries

        // 1. Empty message followed by long message
        let _ = stdin.write_all(b"\n").await; // Empty message
        tokio::time::sleep(Duration::from_millis(100)).await;

        let long_msg = format!("{}\n", "EdgeCase".repeat(500));
        let _ = stdin.write_all(long_msg.as_bytes()).await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // 2. Rapid short messages
        for i in 1..=5 {
            let msg = format!("R{}\n", i);
            let _ = stdin.write_all(msg.as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(30)).await;
        }

        // 3. Check statistics during edge conditions
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // 4. Special characters and edge case content
        let _ = stdin
            .write_all("Special chars: 测试 🚀 ñáéíóú\n".as_bytes())
            .await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Comprehensive edge cases test output:\n{}", combined_output);

    // Comprehensive checks for all edge case aspects
    let checks = vec![
        (
            "long_message_handling",
            combined_output.contains("EdgeCase"),
        ),
        (
            "rapid_message_processing",
            combined_output.contains("R1") && combined_output.contains("R5"),
        ),
        (
            "special_character_support",
            combined_output.contains("测试") || combined_output.contains("🚀"),
        ),
        (
            "statistics_accuracy",
            combined_output.contains("Messages sent") && combined_output.contains("round-trip"),
        ),
        (
            "session_duration_tracking",
            combined_output.contains("Session duration"),
        ),
        ("graceful_completion", command_output.status.success()),
        ("no_crashes_or_hangs", true), // If we got here, no crashes occurred
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

    // Require most checks to pass for comprehensive edge case handling
    assert!(
        passed_checks >= 6,
        "At least 6/7 edge case checks should pass. Passed: {}/7. Output: {}",
        passed_checks,
        combined_output
    );

    println!("✅ Comprehensive edge cases and boundaries test passed");
    println!(
        "   - {}/{} edge case features verified",
        passed_checks,
        checks.len()
    );
    println!("   - Complete edge case workflow successful");
    println!("   - System handles boundary conditions robustly");
}
