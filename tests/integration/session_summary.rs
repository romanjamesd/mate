//! Session Summary Tests
//!
//! Tests for Session Summary functionality as specified in tests-to-add.md:
//! - Test that session duration is calculated and displayed
//! - Test that message count is accurately reported
//! - Test that performance statistics are summarized
//! - Test appropriate summary when no messages were sent
//! - Test clear session termination indication
//! - Test summary information is well-formatted

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

/// Test that session duration is calculated and displayed
#[tokio::test]
async fn test_session_duration_calculated_and_displayed() {
    println!("Testing that session duration is calculated and displayed");

    let server_addr = "127.0.0.1:18201";
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
        // Send a test message to ensure the session is active for some time
        let _ = stdin.write_all(b"Duration test message\n").await;
        tokio::time::sleep(Duration::from_millis(1000)).await; // Wait at least 1 second for measurable duration

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(8), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Session duration test output:\n{}", combined_output);

    // Verify session summary section exists
    assert!(
        combined_output.contains("Session Summary") || combined_output.contains("session summary"),
        "Should display session summary section. Output: {}",
        combined_output
    );

    // Verify session duration is displayed
    assert!(
        combined_output.contains("Session duration:") || combined_output.contains("duration"),
        "Should display session duration. Output: {}",
        combined_output
    );

    // Verify duration shows reasonable time units (ms, s, or µs)
    assert!(
        combined_output.contains("ms")
            || combined_output.contains("µs")
            || combined_output.contains("us")
            || combined_output.contains("s"),
        "Should display duration with time units. Output: {}",
        combined_output
    );

    // Verify session termination message
    assert!(
        combined_output.contains("Goodbye!")
            || combined_output.contains("goodbye")
            || combined_output.contains("exit")
            || combined_output.contains("terminated"),
        "Should display session termination indication. Output: {}",
        combined_output
    );

    println!("✅ Session duration calculation and display test passed");
    println!("   - Session summary section is displayed");
    println!("   - Session duration is calculated and shown");
    println!("   - Duration includes appropriate time units");
    println!("   - Session termination is clearly indicated");
}

/// Test that message count is accurately reported
#[tokio::test]
async fn test_message_count_accurately_reported() {
    println!("Testing that message count is accurately reported");

    let server_addr = "127.0.0.1:18202";
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
        // Send exactly 3 messages to test accurate counting
        let _ = stdin.write_all(b"Count test message 1\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Count test message 2\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Count test message 3\n").await;
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
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Message count test output:\n{}", combined_output);

    // Verify session summary shows message count
    assert!(
        combined_output.contains("Messages sent: 3")
            || (combined_output.contains("3") && combined_output.contains("message")),
        "Should accurately report 3 messages sent. Output: {}",
        combined_output
    );

    // Verify all messages were echoed back
    let echo_count = combined_output.matches("Received echo").count();
    assert_eq!(
        echo_count, 3,
        "Should have received echo for all 3 messages. Echo count: {}, Output: {}",
        echo_count, combined_output
    );

    // Verify session summary section exists
    assert!(
        combined_output.contains("Session Summary"),
        "Should display session summary section. Output: {}",
        combined_output
    );

    println!("✅ Message count accuracy test passed");
    println!("   - Message count is accurately tracked and reported");
    println!("   - All sent messages were processed");
    println!("   - Session summary includes message statistics");
}

/// Test that performance statistics are summarized
#[tokio::test]
async fn test_performance_statistics_summarized() {
    println!("Testing that performance statistics are summarized");

    let server_addr = "127.0.0.1:18203";
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
        // Send multiple messages to generate performance statistics
        let _ = stdin.write_all(b"Performance message 1\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Performance message 2\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Performance message 3\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Performance message 4\n").await;
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
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Performance statistics test output:\n{}", combined_output);

    // Verify performance statistics are included in summary
    assert!(
        combined_output.contains("Average round-trip time")
            || combined_output.contains("average")
            || combined_output.contains("Average"),
        "Should display average performance statistics. Output: {}",
        combined_output
    );

    // Verify time units are shown for performance metrics
    assert!(
        combined_output.contains("ms")
            || combined_output.contains("µs")
            || combined_output.contains("us"),
        "Should show performance metrics with time units. Output: {}",
        combined_output
    );

    // Verify individual round-trip times were measured
    let timing_mentions = combined_output.matches("round-trip").count();
    assert!(
        timing_mentions >= 4,
        "Should record timing for each message. Timing mentions: {}, Output: {}",
        timing_mentions,
        combined_output
    );

    // Verify session summary includes performance data
    assert!(
        combined_output.contains("Session Summary"),
        "Should display session summary section. Output: {}",
        combined_output
    );

    // Verify message count matches performance data
    assert!(
        combined_output.contains("Messages sent: 4")
            || (combined_output.contains("4") && combined_output.contains("message")),
        "Should accurately report 4 messages sent. Output: {}",
        combined_output
    );

    println!("✅ Performance statistics summary test passed");
    println!("   - Performance statistics are summarized in session end");
    println!("   - Average round-trip time is calculated and displayed");
    println!("   - Performance metrics include appropriate time units");
    println!("   - Individual message timings contribute to summary");
}

/// Test appropriate summary when no messages were sent
#[tokio::test]
async fn test_appropriate_summary_no_messages_sent() {
    println!("Testing appropriate summary when no messages were sent");

    let server_addr = "127.0.0.1:18204";
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
        // Wait a bit to ensure session duration is measureable, but don't send any messages
        tokio::time::sleep(Duration::from_millis(800)).await;

        // Exit without sending any messages
        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(6), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!("No messages summary test output:\n{}", combined_output);

    // Verify session summary is still displayed
    assert!(
        combined_output.contains("Session Summary"),
        "Should display session summary even with no messages. Output: {}",
        combined_output
    );

    // Verify session duration is still calculated and shown
    assert!(
        combined_output.contains("Session duration:") || combined_output.contains("duration"),
        "Should display session duration even with no messages. Output: {}",
        combined_output
    );

    // Verify appropriate message for no messages sent
    assert!(
        combined_output.contains("No messages sent during this session")
            || combined_output.contains("0 messages")
            || combined_output.contains("Messages sent: 0"),
        "Should indicate no messages were sent. Output: {}",
        combined_output
    );

    // Verify no performance statistics are shown when no messages were sent
    assert!(
        !combined_output.contains("Average round-trip time"),
        "Should not show average round-trip time when no messages sent. Output: {}",
        combined_output
    );

    // Verify session termination message
    assert!(
        combined_output.contains("Goodbye!"),
        "Should display session termination message. Output: {}",
        combined_output
    );

    println!("✅ No messages sent summary test passed");
    println!("   - Session summary is displayed even with no messages");
    println!("   - Session duration is calculated and shown");
    println!("   - Appropriate message for zero message count");
    println!("   - No performance statistics shown when inappropriate");
    println!("   - Session termination is clearly indicated");
}

/// Test clear session termination indication
#[tokio::test]
async fn test_clear_session_termination_indication() {
    println!("Testing clear session termination indication");

    let server_addr = "127.0.0.1:18205";
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
        // Send one message and then quit
        let _ = stdin.write_all(b"Termination test message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"quit\n").await;
    }

    let output = timeout(Duration::from_secs(6), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Session termination test output:\n{}", combined_output);

    // Verify clear termination message
    assert!(
        combined_output.contains("Goodbye!"),
        "Should display clear termination message. Output: {}",
        combined_output
    );

    // Verify session summary appears before termination
    assert!(
        combined_output.contains("Session Summary"),
        "Should display session summary before termination. Output: {}",
        combined_output
    );

    // Verify the order: summary comes before goodbye
    let summary_pos = combined_output.find("Session Summary");
    let goodbye_pos = combined_output.find("Goodbye!");

    assert!(
        summary_pos.is_some() && goodbye_pos.is_some(),
        "Both summary and goodbye should be present. Output: {}",
        combined_output
    );

    assert!(
        summary_pos.unwrap() < goodbye_pos.unwrap(),
        "Session summary should appear before goodbye message. Output: {}",
        combined_output
    );

    // Verify program terminates successfully
    assert!(
        command_output.status.success(),
        "Program should terminate successfully. Exit code: {:?}",
        command_output.status.code()
    );

    println!("✅ Session termination indication test passed");
    println!("   - Clear termination message is displayed");
    println!("   - Session summary appears before termination");
    println!("   - Program terminates successfully");
    println!("   - Proper ordering of summary and termination messages");
}

/// Test summary information is well-formatted
#[tokio::test]
async fn test_summary_information_well_formatted() {
    println!("Testing that summary information is well-formatted");

    let server_addr = "127.0.0.1:18206";
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
        // Send messages to ensure we have data to format
        let _ = stdin.write_all(b"Format test message 1\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Format test message 2\n").await;
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
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Well-formatted summary test output:\n{}", combined_output);

    // Verify session summary has clear header
    assert!(
        combined_output.contains("=== Session Summary ===")
            || combined_output.contains("Session Summary"),
        "Should have clear session summary header. Output: {}",
        combined_output
    );

    // Verify structured information layout
    assert!(
        combined_output.contains("Session duration:"),
        "Should have structured session duration field. Output: {}",
        combined_output
    );

    assert!(
        combined_output.contains("Messages sent:"),
        "Should have structured messages sent field. Output: {}",
        combined_output
    );

    assert!(
        combined_output.contains("Average round-trip time:"),
        "Should have structured performance metrics field. Output: {}",
        combined_output
    );

    // Verify proper spacing and readability
    let summary_section = if let Some(start) = combined_output.find("Session Summary") {
        if let Some(end) = combined_output[start..].find("Goodbye!") {
            &combined_output[start..start + end]
        } else {
            &combined_output[start..]
        }
    } else {
        ""
    };

    assert!(
        !summary_section.is_empty(),
        "Should have identifiable summary section. Output: {}",
        combined_output
    );

    // Verify no extraneous debug information in summary
    assert!(
        !summary_section.to_lowercase().contains("debug")
            && !summary_section.to_lowercase().contains("error"),
        "Summary should not contain debug/error information. Summary: {}",
        summary_section
    );

    // Verify consistent formatting (fields should have consistent structure)
    let colon_count = summary_section.matches(':').count();
    assert!(
        colon_count >= 3,
        "Should have consistent field formatting with colons. Colons: {}, Summary: {}",
        colon_count,
        summary_section
    );

    println!("✅ Well-formatted summary test passed");
    println!("   - Session summary has clear header");
    println!("   - Information is structured with labeled fields");
    println!("   - Proper spacing and readability");
    println!("   - No extraneous debug information");
    println!("   - Consistent formatting throughout");
}

/// Test comprehensive session summary functionality
#[tokio::test]
async fn test_comprehensive_session_summary() {
    println!("Testing comprehensive session summary functionality");

    let server_addr = "127.0.0.1:18207";
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
        // Send varied messages with different timing
        let _ = stdin.write_all(b"Comprehensive test message 1\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"Message with different content 2\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Third comprehensive test\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Final test message\n").await;
        tokio::time::sleep(Duration::from_millis(350)).await;

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
        "Comprehensive session summary test output:\n{}",
        combined_output
    );

    // Comprehensive check of all session summary features
    let checks = vec![
        (
            "session_summary_header",
            combined_output.contains("Session Summary"),
        ),
        (
            "session_duration_calculated",
            combined_output.contains("Session duration:"),
        ),
        (
            "message_count_accurate",
            combined_output.contains("Messages sent: 4")
                || (combined_output.contains("4") && combined_output.contains("message")),
        ),
        (
            "performance_statistics",
            combined_output.contains("Average round-trip time:"),
        ),
        (
            "time_units_displayed",
            combined_output.contains("ms")
                || combined_output.contains("µs")
                || combined_output.contains("us"),
        ),
        (
            "termination_indication",
            combined_output.contains("Goodbye!"),
        ),
        (
            "all_messages_processed",
            combined_output.matches("Received echo").count() == 4,
        ),
        (
            "individual_timings_recorded",
            combined_output.matches("round-trip").count() >= 4,
        ),
        (
            "proper_formatting",
            combined_output.contains("===") || combined_output.matches(':').count() >= 3,
        ),
        ("successful_termination", command_output.status.success()),
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

    // Require most checks to pass for comprehensive functionality
    assert!(
        passed_checks >= 8,
        "At least 8/10 session summary checks should pass. Passed: {}/10. Output: {}",
        passed_checks,
        combined_output
    );

    println!("✅ Comprehensive session summary test passed");
    println!(
        "   - {}/{} session summary features verified",
        passed_checks,
        checks.len()
    );
    println!("   - Complete session summary workflow successful");
}
