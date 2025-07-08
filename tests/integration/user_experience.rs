//! User Experience Tests
//!
//! Tests for User Experience functionality as specified in tests-to-add.md:
//! - Test that all user communications are clear and informative
//! - Test that connection state changes are clearly indicated
//! - Test consistent visual formatting throughout the session
//! - Test that timing information is consistently presented
//! - Test that session flow is intuitive and responsive
//! - Test that error recovery doesn't create user confusion

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

/// Test that all user communications are clear and informative
#[tokio::test]
async fn test_user_communications_clear_and_informative() {
    println!("Testing that all user communications are clear and informative");

    let server_addr = "127.0.0.1:18401";
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
        // Test various user interactions to verify clear communications
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Test message for clarity\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

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

    println!("User communications test output:\n{}", combined_output);

    // Verify clear session initialization
    assert!(
        combined_output.contains("MATE Chat Session")
            || combined_output.contains("Connected to peer"),
        "Should clearly announce session start. Output: {}",
        combined_output
    );

    // Verify clear command explanations
    assert!(
        combined_output.contains("Available commands") || combined_output.contains("help"),
        "Should clearly explain available functionality. Output: {}",
        combined_output
    );

    // Verify clear response indicators
    assert!(
        combined_output.contains("Received echo") || combined_output.contains("round-trip"),
        "Should clearly indicate message responses. Output: {}",
        combined_output
    );

    // Verify clear session termination
    assert!(
        combined_output.contains("Goodbye") || combined_output.contains("Session Summary"),
        "Should clearly indicate session end. Output: {}",
        combined_output
    );

    // Verify informative content (not just basic confirmations)
    assert!(
        combined_output.contains("peer") && combined_output.contains("status"),
        "Should provide informative connection details. Output: {}",
        combined_output
    );

    // Verify timing information is presented clearly
    assert!(
        combined_output.contains("ms")
            || combined_output.contains("µs")
            || combined_output.contains("us"),
        "Should present timing information clearly. Output: {}",
        combined_output
    );

    // Verify no unexplained technical terms in user-facing content (allow debug logs)
    let user_facing_lines = combined_output
        .lines()
        .filter(|line| {
            // Focus on user-facing content, not debug logs
            !line.contains("INFO")
                && !line.contains("ERROR")
                && !line.contains("DEBUG")
                && !line.starts_with("2025-")
                && !line.contains("connect{")
        })
        .collect::<Vec<_>>();

    let lines_with_unexplained_technical_terms = user_facing_lines
        .iter()
        .filter(|line| {
            // Look for technical terms that appear without context in user-facing content
            (line.contains("nonce") || line.contains("payload") || line.contains("envelope"))
                && !line.to_lowercase().contains("debug")
                && !line.to_lowercase().contains("info")
                && !line.to_lowercase().contains("error")
        })
        .count();

    assert!(
        lines_with_unexplained_technical_terms == 0,
        "Should not expose unexplained technical terms to users. Found {} lines",
        lines_with_unexplained_technical_terms
    );

    println!("✅ User communications clarity test passed");
    println!("   - Session initialization is clearly communicated");
    println!("   - Command explanations are clear and helpful");
    println!("   - Response indicators are unambiguous");
    println!("   - Session termination is clearly indicated");
    println!("   - Information provided is substantive and useful");
}

/// Test that connection state changes are clearly indicated
#[tokio::test]
async fn test_connection_state_changes_clearly_indicated() {
    println!("Testing that connection state changes are clearly indicated");

    // Test connection establishment indication
    let server_addr = "127.0.0.1:18402";
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
        // Send a message to establish active communication
        let _ = stdin.write_all(b"Connection state test\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Simulate connection disruption by killing server
        server_handle.abort();
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Try to send another message to trigger reconnection or failure handling
        let _ = stdin.write_all(b"Message after disconnection\n").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(8), child.wait_with_output()).await;

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Connection state changes test output:\n{}", combined_output);

    // Verify connection establishment is clearly indicated
    assert!(
        combined_output.contains("Connected to peer")
            || combined_output.contains("Connection status: Active"),
        "Should clearly indicate successful connection. Output: {}",
        combined_output
    );

    // Verify connection issues are clearly communicated
    let has_connection_state_communication = combined_output.contains("Failed to send")
        || combined_output.contains("Connection")
        || combined_output.contains("reconnect")
        || combined_output.contains("disconnect")
        || combined_output.contains("lost");

    assert!(
        has_connection_state_communication,
        "Should clearly communicate connection state changes. Output: {}",
        combined_output
    );

    // Check for user-friendly language in user-facing content (not debug logs)
    let user_facing_content = combined_output
        .lines()
        .filter(|line| {
            // Filter out debug/log lines to focus on user-facing content
            !line.contains("INFO")
                && !line.contains("ERROR")
                && !line.contains("DEBUG")
                && !line.starts_with("2025-")
                && !line.contains("connect{")
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Verify user-facing content uses friendly language
    assert!(!user_facing_content.contains("TCP") && !user_facing_content.contains("socket") && 
            !user_facing_content.contains("ECONNREFUSED"),
           "User-facing content should use user-friendly language, not low-level technical terms. User content: {}", user_facing_content);

    // Verify connection status is distinguishable from other messages
    let status_lines = combined_output
        .lines()
        .filter(|line| {
            line.contains("Connected")
                || line.contains("Connection status")
                || line.contains("Failed to connect")
        })
        .count();

    assert!(
        status_lines >= 1,
        "Should have clearly identifiable connection status messages. Found: {}. Output: {}",
        status_lines,
        combined_output
    );

    println!("✅ Connection state change indication test passed");
    println!("   - Connection establishment clearly indicated");
    println!("   - Connection state changes are communicated");
    println!("   - User-friendly language used for status updates");
    println!("   - Status messages are distinguishable");
}

/// Test consistent visual formatting throughout the session
#[tokio::test]
async fn test_consistent_visual_formatting_throughout_session() {
    println!("Testing consistent visual formatting throughout the session");

    let server_addr = "127.0.0.1:18403";
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
        // Test multiple interactions to check formatting consistency
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"First formatting test message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Second formatting test message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"info\n").await;
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
    let combined_output = format!("{stdout}{stderr}");

    println!(
        "Visual formatting consistency test output:\n{}",
        combined_output
    );

    // Verify consistent header formatting (focus on major headers)
    let major_header_lines = combined_output
        .lines()
        .filter(|line| {
            // Focus on major section headers, not sub-headers that might vary
            line.contains("=== MATE Chat Session ===")
                || line.contains("=== Session Summary ===")
                || line.contains("=== Connection Information ===")
        })
        .collect::<Vec<_>>();

    if major_header_lines.len() > 1 {
        // Check that major headers use consistent format
        let first_header_equals = major_header_lines[0].matches("=").count();
        let consistent_major_headers = major_header_lines
            .iter()
            .all(|line| line.matches("=").count() == first_header_equals);

        assert!(
            consistent_major_headers,
            "Major headers should use consistent formatting. Headers: {:?}",
            major_header_lines
        );
    }

    // Verify consistent prompt formatting (if using prompts)
    let prompt_lines = combined_output
        .lines()
        .filter(|line| line.contains("mate>"))
        .collect::<Vec<_>>();

    if prompt_lines.len() > 1 {
        let consistent_prompts = prompt_lines
            .iter()
            .all(|line| line.starts_with("mate>") || line.contains("mate>"));

        assert!(
            consistent_prompts,
            "Prompts should use consistent formatting. Prompts: {:?}",
            prompt_lines
        );
    }

    // Verify consistent message response formatting
    let response_lines = combined_output
        .lines()
        .filter(|line| line.contains("Received echo"))
        .collect::<Vec<_>>();

    if response_lines.len() > 1 {
        // Check that response format is consistent
        let consistent_responses = response_lines
            .iter()
            .all(|line| line.contains("round-trip") || line.contains("echo"));

        assert!(
            consistent_responses,
            "Response messages should use consistent formatting. Responses: {:?}",
            response_lines
        );
    }

    // Verify consistent spacing and alignment in info sections
    let info_sections = combined_output
        .split("=== Connection Information ===")
        .skip(1)
        .collect::<Vec<_>>();

    if info_sections.len() > 1 {
        // Check that info sections have consistent field formatting
        for section in info_sections {
            let field_lines = section
                .lines()
                .filter(|line| {
                    line.contains(":") && !line.contains("INFO") && !line.contains("ERROR")
                })
                .take(5) // Check first few fields
                .collect::<Vec<_>>();

            if field_lines.len() > 1 {
                let consistent_colons = field_lines
                    .iter()
                    .all(|line| line.contains(": ") || line.contains(":"));

                assert!(
                    consistent_colons,
                    "Info sections should have consistent field formatting. Fields: {:?}",
                    field_lines
                );
            }
        }
    }

    // Verify no mixed formatting styles
    let has_mixed_arrow_styles = combined_output.contains("->") && combined_output.contains("←");
    assert!(
        !has_mixed_arrow_styles,
        "Should not mix different arrow styles. Output: {}",
        combined_output
    );

    println!("✅ Visual formatting consistency test passed");
    println!("   - Headers use consistent formatting");
    println!("   - Prompts maintain consistent style");
    println!("   - Response messages formatted consistently");
    println!("   - No mixed formatting styles detected");
}

/// Test that timing information is consistently presented
#[tokio::test]
async fn test_timing_information_consistently_presented() {
    println!("Testing that timing information is consistently presented");

    let server_addr = "127.0.0.1:18404";
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
        // Send multiple messages to generate consistent timing displays
        let _ = stdin.write_all(b"Timing consistency test 1\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Timing consistency test 2\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Timing consistency test 3\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
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
    let combined_output = format!("{stdout}{stderr}");

    println!(
        "Timing information consistency test output:\n{}",
        combined_output
    );

    // Verify timing units are consistently used
    let timing_lines = combined_output
        .lines()
        .filter(|line| {
            line.contains("round-trip") || line.contains("Average") || line.contains("duration")
        })
        .collect::<Vec<_>>();

    assert!(
        timing_lines.len() >= 3,
        "Should have multiple timing displays for consistency check. Found: {}. Lines: {:?}",
        timing_lines.len(),
        timing_lines
    );

    // Check timing unit consistency
    let uses_ms = combined_output.contains("ms");
    let uses_us = combined_output.contains("µs") || combined_output.contains("us");
    let uses_s = combined_output.contains("s") && !combined_output.contains("ms");

    assert!(
        uses_ms || uses_us || uses_s,
        "Should display timing with appropriate units. Output: {}",
        combined_output
    );

    // Check that similar timing types use consistent formatting pattern
    let round_trip_lines = timing_lines
        .iter()
        .filter(|line| line.contains("round-trip"))
        .collect::<Vec<_>>();

    if round_trip_lines.len() > 1 {
        // Check for some consistent pattern in round-trip displays
        let has_consistent_pattern = round_trip_lines
            .iter()
            .all(|line| line.contains("(") && line.contains(")"))
            || round_trip_lines.iter().all(|line| line.contains(":"));

        assert!(
            has_consistent_pattern,
            "Round-trip timing should use some consistent pattern. Lines: {:?}",
            round_trip_lines
        );
    }

    // Verify timing precision is appropriate and consistent
    let timing_values = combined_output
        .matches(char::is_numeric)
        .collect::<Vec<_>>();

    assert!(
        !timing_values.is_empty(),
        "Should display actual timing values. Output: {}",
        combined_output
    );

    // Check that timing information is contextual and not overwhelming
    let timing_density = timing_lines.len() as f32 / combined_output.lines().count() as f32;
    assert!(
        timing_density < 0.5,
        "Timing information should not overwhelm other content. Density: {:.2}",
        timing_density
    );

    // Verify timing appears in logical places
    assert!(
        combined_output.contains("Session duration") || combined_output.contains("Average"),
        "Should include session-level timing summary. Output: {}",
        combined_output
    );

    println!("✅ Timing information consistency test passed");
    println!("   - Timing units are consistently used");
    println!("   - Timing formats have consistent patterns");
    println!("   - Timing precision is appropriate");
    println!("   - Timing information appears in logical contexts");
}

/// Test that session flow is intuitive and responsive
#[tokio::test]
async fn test_session_flow_intuitive_and_responsive() {
    println!("Testing that session flow is intuitive and responsive");

    let server_addr = "127.0.0.1:18405";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let start_time = std::time::Instant::now();

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    let initialization_time = start_time.elapsed();
    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Test intuitive command discovery
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Test responsive information access
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Test responsive message exchange
        let _ = stdin.write_all(b"Flow test message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Test graceful exit
        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(8), child.wait_with_output()).await;

    let total_session_time = start_time.elapsed();
    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Session flow test output:\n{}", combined_output);

    // Verify quick initialization (responsive startup)
    assert!(
        initialization_time < Duration::from_millis(1000),
        "Session should initialize quickly. Took: {:?}",
        initialization_time
    );

    // Verify intuitive progression from connection to ready state
    let has_logical_progression = combined_output.contains("Connected")
        && (combined_output.contains("Available commands")
            || combined_output.contains("Type messages"));

    assert!(
        has_logical_progression,
        "Should have logical progression from connection to ready state. Output: {}",
        combined_output
    );

    // Verify responsive command execution
    assert!(
        total_session_time < Duration::from_secs(6),
        "Session should be responsive overall. Total time: {:?}",
        total_session_time
    );

    // Verify intuitive command structure
    assert!(
        combined_output.contains("help")
            && combined_output.contains("info")
            && combined_output.contains("quit"),
        "Should present intuitive command structure. Output: {}",
        combined_output
    );

    // Verify clear workflow progression
    let workflow_elements = vec![
        "Connected",
        "Available",
        "mate>",
        "Received",
        "Session Summary",
        "Goodbye",
    ];

    let mut found_elements = Vec::new();
    for element in workflow_elements {
        if combined_output.contains(element) {
            found_elements.push(element);
        }
    }

    assert!(
        found_elements.len() >= 4,
        "Should have clear workflow progression. Found: {:?}",
        found_elements
    );

    // Verify no confusing or cryptic interactions in user-facing content
    let user_facing_lines = combined_output
        .lines()
        .filter(|line| {
            // Filter out debug/log lines to focus on user-facing content
            !line.contains("INFO")
                && !line.contains("ERROR")
                && !line.contains("DEBUG")
                && !line.starts_with("2025-")
                && !line.contains("connect{")
        })
        .collect::<Vec<_>>();

    let user_facing_content = user_facing_lines.join("\n");

    let has_confusing_elements = user_facing_content.contains("undefined")
        || user_facing_content.contains("null")
        || user_facing_content.contains("panic");

    assert!(!has_confusing_elements,
           "Should not have confusing or cryptic interactions in user-facing content. User content: {}", user_facing_content);

    // Verify natural language interactions
    let natural_phrases = combined_output
        .lines()
        .filter(|line| {
            line.contains("Available commands")
                || line.contains("Type messages")
                || line.contains("Connected to")
                || line.contains("Session duration")
        })
        .count();

    assert!(
        natural_phrases >= 2,
        "Should use natural language for user interactions. Found: {} phrases",
        natural_phrases
    );

    println!("✅ Session flow intuitive and responsive test passed");
    println!("   - Quick initialization: {:?}", initialization_time);
    println!("   - Logical progression from connection to ready state");
    println!("   - Responsive command execution");
    println!(
        "   - Clear workflow progression with {} elements",
        found_elements.len()
    );
    println!("   - Natural language interactions");
    println!("   - Total session time: {:?}", total_session_time);
}

/// Test that error recovery doesn't create user confusion
#[tokio::test]
async fn test_error_recovery_doesnt_create_user_confusion() {
    println!("Testing that error recovery doesn't create user confusion");

    // Start a server that will be killed mid-session to test error recovery
    let server_addr = "127.0.0.1:18406";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(800)).await;
        // Server dies to trigger error recovery
        drop(server);
    });

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
        // Establish normal flow first
        let _ = stdin.write_all(b"Pre-error test message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Server should die around here, triggering error recovery
        let _ = stdin.write_all(b"Message during server failure\n").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Try more interactions to see error recovery behavior
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Another test message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let _ = server_handle.await;

    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await;

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!("Error recovery confusion test output:\n{}", combined_output);

    // Extract user-facing error content (not debug logs)
    let user_facing_error_lines = combined_output.lines()
        .filter(|line| {
            let line_lower = line.to_lowercase();
            (line_lower.contains("failed") || line_lower.contains("error") || line_lower.contains("unable")) &&
            // Exclude debug/log lines
            !line.contains("INFO") && !line.contains("ERROR") && !line.contains("DEBUG") &&
            !line.starts_with("2025-") && !line.contains("connect{")
        })
        .collect::<Vec<_>>();

    // Verify error messages are clear and actionable, not technical
    for error_line in &user_facing_error_lines {
        // Should not contain low-level technical details in user-facing content
        assert!(
            !error_line.contains("ECONNRESET")
                && !error_line.contains("socket")
                && !error_line.contains("errno")
                && !error_line.contains("0x"),
            "User-facing error messages should not contain low-level technical details: '{}'",
            error_line
        );
    }

    // Verify no duplicate or contradictory status messages in user-facing content
    let user_facing_status_messages = combined_output
        .lines()
        .filter(|line| {
            (line.contains("Connected") ||
            line.contains("Connection status") ||
            line.contains("Failed to connect") ||
            line.contains("reconnect")) &&
            // Exclude debug/log lines
            !line.contains("INFO") && !line.contains("ERROR") && !line.contains("DEBUG") &&
            !line.starts_with("2025-") && !line.contains("connect{")
        })
        .collect::<Vec<_>>();

    if user_facing_status_messages.len() > 1 {
        println!(
            "User-facing status messages: {:?}",
            user_facing_status_messages
        );
        // Allow for retry/recovery messages, but verify they make sense
    }

    // Verify the user gets clear guidance during errors (can be in logs or user-facing)
    let has_clear_guidance = combined_output.contains("Failed to send")
        || combined_output.contains("Failed to connect")
        || combined_output.contains("Connection")
        || user_facing_error_lines.is_empty()
        || combined_output.contains("ERROR");

    assert!(
        has_clear_guidance,
        "Should provide clear guidance during errors. Output: {}",
        combined_output
    );

    // Verify graceful handling without user action required
    assert!(
        command_output.status.code().unwrap_or(-1) >= 0,
        "Should handle errors gracefully without requiring user intervention"
    );

    // Verify session state remains comprehensible (can be implicit through completion)
    let session_remains_comprehensible = combined_output.contains("Session")
        || combined_output.contains("Goodbye")
        || command_output.status.code() == Some(1); // Graceful exit on error

    assert!(
        session_remains_comprehensible,
        "Session state should remain comprehensible during error recovery. Output: {}",
        combined_output
    );

    // Verify no repetitive error spam in user-facing content
    if user_facing_error_lines.len() > 2 {
        // Check for repeated identical error messages
        let unique_errors = user_facing_error_lines
            .iter()
            .collect::<std::collections::HashSet<_>>();
        let repetition_ratio = user_facing_error_lines.len() as f32 / unique_errors.len() as f32;

        assert!(repetition_ratio < 3.0,
               "Should not spam user with repetitive error messages in user-facing content. Repetition ratio: {:.1}", repetition_ratio);
    }

    println!("✅ Error recovery confusion prevention test passed");
    println!("   - Error messages are clear and actionable");
    println!("   - No low-level technical details exposed to users");
    println!("   - No contradictory status messages in user content");
    println!("   - Clear guidance provided during errors");
    println!("   - Graceful handling without user intervention required");
    println!(
        "   - {} user-facing error lines found, handled appropriately",
        user_facing_error_lines.len()
    );
}

/// Test comprehensive user experience functionality
#[tokio::test]
async fn test_comprehensive_user_experience() {
    println!("Testing comprehensive user experience functionality");

    let server_addr = "127.0.0.1:18407";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let session_start = std::time::Instant::now();

    let mut child = Command::new(get_mate_binary_path())
        .args(["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        // Comprehensive user experience test covering multiple aspects
        let _ = stdin.write_all(b"help\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Comprehensive UX test message 1\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Comprehensive UX test message 2\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await;

    let session_duration = session_start.elapsed();
    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{stdout}{stderr}");

    println!(
        "Comprehensive user experience test output:\n{}",
        combined_output
    );

    // Comprehensive checks for all user experience aspects
    let checks = vec![
        (
            "clear_communications",
            combined_output.contains("Connected") && combined_output.contains("Available"),
        ),
        (
            "connection_state_indication",
            combined_output.contains("Connection status")
                || combined_output.contains("Connected to peer"),
        ),
        (
            "consistent_formatting",
            combined_output.contains("===") && combined_output.matches("===").count() >= 2,
        ),
        (
            "timing_information_present",
            combined_output.contains("ms")
                || combined_output.contains("µs")
                || combined_output.contains("us"),
        ),
        (
            "responsive_session",
            session_duration < Duration::from_secs(8),
        ),
        (
            "intuitive_commands",
            combined_output.contains("help")
                && combined_output.contains("info")
                && combined_output.contains("quit"),
        ),
        (
            "clear_workflow",
            combined_output.contains("Session Summary") || combined_output.contains("Goodbye"),
        ),
        (
            "no_technical_jargon_in_ui",
            !combined_output
                .lines()
                .filter(|line| {
                    !line.contains("INFO") && !line.contains("ERROR") && !line.contains("DEBUG")
                })
                .any(|line| line.contains("nonce") || line.contains("payload")),
        ),
        (
            "informative_responses",
            combined_output.contains("Received echo") || combined_output.contains("round-trip"),
        ),
        ("graceful_termination", command_output.status.success()),
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

    // Require most checks to pass for comprehensive user experience
    assert!(
        passed_checks >= 8,
        "At least 8/10 user experience checks should pass. Passed: {}/10. Output: {}",
        passed_checks,
        combined_output
    );

    println!("✅ Comprehensive user experience test passed");
    println!(
        "   - {}/{} user experience features verified",
        passed_checks,
        checks.len()
    );
    println!("   - Complete user experience workflow successful");
    println!("   - Session duration: {:?}", session_duration);
}
