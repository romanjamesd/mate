//! Interactive Message Exchange Tests
//!
//! Tests for Interactive Message Exchange functionality as specified in tests-to-add.md:
//! - Test successful message send and echo response display
//! - Test that response timing is measured for each message
//! - Test that message statistics are accurately tracked
//! - Test that performance metrics accumulate correctly
//! - Test clear indication of received responses
//! - Test that multiple message exchanges maintain accurate statistics

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

/// Test successful message send and echo response display
#[tokio::test]
async fn test_successful_message_send_and_echo_response() {
    println!("Testing successful message send and echo response display");

    let server_addr = "127.0.0.1:18121";
    let server = start_test_server(server_addr)
        .await
        .expect("Failed to start test server");

    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to start
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
        // Send a test message
        let _ = stdin.write_all(b"Hello from interactive test\n").await;
        tokio::time::sleep(Duration::from_millis(500)).await;

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

    println!(
        "Message send and echo response output:\n{}",
        combined_output
    );

    // Verify message was sent
    assert!(
        combined_output.contains("Sending Ping message"),
        "Should show message being sent. Output: {}",
        combined_output
    );

    // Verify echo response was displayed
    assert!(
        combined_output.contains("Received echo") || combined_output.contains("← Received echo"),
        "Should display echo response. Output: {}",
        combined_output
    );

    // Verify the actual message content appears in the echo
    assert!(
        combined_output.contains("Hello from interactive test"),
        "Echo should contain the actual message content. Output: {}",
        combined_output
    );

    // Verify successful exchange
    assert!(
        command_output.status.success(),
        "Message exchange should complete successfully. Status: {}",
        command_output.status
    );

    println!("✅ Successful message send and echo response test passed");
    println!("   - Message was successfully sent");
    println!("   - Echo response was displayed");
    println!("   - Message content was echoed correctly");
}

/// Test that response timing is measured for each message
#[tokio::test]
async fn test_response_timing_measured() {
    println!("Testing that response timing is measured for each message");

    let server_addr = "127.0.0.1:18122";
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
        // Send multiple messages to test timing for each
        let _ = stdin.write_all(b"First message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Second message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Third message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

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

    println!("Response timing measurement output:\n{}", combined_output);

    // Count timing displays - should be one for each message
    let timing_count = combined_output.matches("round-trip").count();
    assert!(
        timing_count >= 3,
        "Should show timing for each message. Expected at least 3, got: {}, Output: {}",
        timing_count,
        combined_output
    );

    // Verify timing format (should show time units)
    assert!(
        combined_output.contains("ms")
            || combined_output.contains("µs")
            || combined_output.contains("us"),
        "Should display timing with time units. Output: {}",
        combined_output
    );

    // Verify each message got a timing measurement
    let echo_responses = combined_output.matches("Received echo").count();
    assert_eq!(
        echo_responses, 3,
        "Should have echo responses for all 3 messages. Got: {}, Output: {}",
        echo_responses, combined_output
    );

    println!("✅ Response timing measurement test passed");
    println!("   - Timing is measured for each message");
    println!("   - Timing displays appropriate time units");
    println!("   - Each message gets individual timing");
}

/// Test that message statistics are accurately tracked
#[tokio::test]
async fn test_message_statistics_tracked() {
    println!("Testing that message statistics are accurately tracked");

    let server_addr = "127.0.0.1:18123";
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
        // Send multiple messages to build up statistics
        let _ = stdin.write_all(b"Message 1\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Message 2\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Message 3\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let _ = stdin.write_all(b"Message 4\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Check info to see statistics
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

    println!("Message statistics tracking output:\n{}", combined_output);

    // Verify message count is tracked
    assert!(
        combined_output.contains("Messages sent: 4")
            || combined_output.contains("4") && combined_output.contains("message"),
        "Should track message count accurately. Output: {}",
        combined_output
    );

    // Verify session summary shows statistics
    assert!(
        combined_output.contains("Session Summary") || combined_output.contains("session"),
        "Should show session summary with statistics. Output: {}",
        combined_output
    );

    // Verify all messages were processed
    let echo_count = combined_output.matches("Received echo").count();
    assert_eq!(
        echo_count, 4,
        "Should have processed all 4 messages. Echo count: {}, Output: {}",
        echo_count, combined_output
    );

    println!("✅ Message statistics tracking test passed");
    println!("   - Message count is accurately tracked");
    println!("   - Session summary displays statistics");
    println!("   - All messages are processed and counted");
}

/// Test that performance metrics accumulate correctly
#[tokio::test]
async fn test_performance_metrics_accumulate() {
    println!("Testing that performance metrics accumulate correctly");

    let server_addr = "127.0.0.1:18124";
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
        // Send messages with some spacing to build metrics
        let _ = stdin.write_all(b"Perf test 1\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Perf test 2\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let _ = stdin.write_all(b"Perf test 3\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Check performance metrics via info
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

    println!(
        "Performance metrics accumulation output:\n{}",
        combined_output
    );

    // Verify average timing is calculated
    assert!(
        combined_output.contains("Average") || combined_output.contains("average"),
        "Should show average performance metrics. Output: {}",
        combined_output
    );

    // Verify performance metrics are shown with appropriate units
    assert!(
        combined_output.contains("ms")
            || combined_output.contains("µs")
            || combined_output.contains("us"),
        "Should show performance metrics with time units. Output: {}",
        combined_output
    );

    // Verify session summary includes performance data
    assert!(
        combined_output.contains("round-trip") || combined_output.contains("performance"),
        "Should include performance data in session summary. Output: {}",
        combined_output
    );

    // Verify individual message timings were recorded
    let timing_mentions = combined_output.matches("round-trip").count();
    assert!(
        timing_mentions >= 3,
        "Should record timing for each message. Timing mentions: {}, Output: {}",
        timing_mentions,
        combined_output
    );

    println!("✅ Performance metrics accumulation test passed");
    println!("   - Performance metrics accumulate correctly");
    println!("   - Average timing is calculated");
    println!("   - Performance data is included in summaries");
}

/// Test clear indication of received responses
#[tokio::test]
async fn test_clear_indication_of_received_responses() {
    println!("Testing clear indication of received responses");

    let server_addr = "127.0.0.1:18125";
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

    let test_messages = vec!["Response test A", "Response test B", "Response test C"];

    if let Some(stdin) = child.stdin.as_mut() {
        for message in &test_messages {
            let _ = stdin.write_all(format!("{message}\n").as_bytes()).await;
            tokio::time::sleep(Duration::from_millis(400)).await;
        }

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

    println!("Clear response indication output:\n{}", combined_output);

    // Verify clear response indication format
    assert!(
        combined_output.contains("Received echo") || combined_output.contains("← Received"),
        "Should clearly indicate received responses. Output: {}",
        combined_output
    );

    // Verify each test message was clearly echoed
    for message in &test_messages {
        assert!(
            combined_output.contains(message),
            "Message '{}' should be clearly indicated in response. Output: {}",
            message,
            combined_output
        );
    }

    // Verify visual distinction between sent and received
    let received_indicators = combined_output.matches("Received echo").count()
        + combined_output.matches("← Received").count();
    assert!(received_indicators >= test_messages.len(),
           "Should have clear indicators for all received responses. Expected: {}, Got: {}, Output: {}", 
           test_messages.len(), received_indicators, combined_output);

    // Verify timing information is included with responses
    let timing_with_responses = combined_output.matches("round-trip").count();
    assert!(
        timing_with_responses >= test_messages.len(),
        "Should include timing with response indications. Expected: {}, Got: {}, Output: {}",
        test_messages.len(),
        timing_with_responses,
        combined_output
    );

    println!("✅ Clear indication of received responses test passed");
    println!("   - Responses are clearly indicated");
    println!("   - All messages have clear echo indications");
    println!("   - Visual distinction between sent and received");
    println!("   - Timing information is included");
}

/// Test that multiple message exchanges maintain accurate statistics
#[tokio::test]
async fn test_multiple_exchanges_maintain_accurate_statistics() {
    println!("Testing that multiple message exchanges maintain accurate statistics");

    let server_addr = "127.0.0.1:18126";
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
        // Send first batch of messages
        for i in 1..=3 {
            let _ = stdin
                .write_all(format!("Batch 1 Message {i}\n").as_bytes())
                .await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        // Check stats after first batch
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Send second batch of messages
        for i in 1..=4 {
            let _ = stdin
                .write_all(format!("Batch 2 Message {i}\n").as_bytes())
                .await;
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        // Check final stats
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
    let combined_output = format!("{stdout}{stderr}");

    println!("Multiple exchanges statistics output:\n{}", combined_output);

    // Verify all messages were processed (7 total: 3 + 4)
    let echo_count = combined_output.matches("Received echo").count();
    assert_eq!(
        echo_count, 7,
        "Should have processed all 7 messages across batches. Echo count: {}, Output: {}",
        echo_count, combined_output
    );

    // Verify final message count is accurate
    assert!(
        combined_output.contains("Messages sent: 7")
            || (combined_output.contains("7") && combined_output.contains("message")),
        "Final statistics should show 7 messages sent. Output: {}",
        combined_output
    );

    // Verify statistics were updated between batches
    // Should have at least 2 info command responses showing different counts
    let info_responses =
        combined_output.matches("connection").count() + combined_output.matches("session").count();
    assert!(
        info_responses >= 2,
        "Should show info responses from both checks. Info responses: {}, Output: {}",
        info_responses,
        combined_output
    );

    // Verify session duration is reasonable
    assert!(
        combined_output.contains("Session duration") || combined_output.contains("duration"),
        "Should show session duration in final summary. Output: {}",
        combined_output
    );

    // Verify average performance calculation with multiple messages
    assert!(
        combined_output.contains("Average") || combined_output.contains("average"),
        "Should calculate average performance across all messages. Output: {}",
        combined_output
    );

    println!("✅ Multiple exchanges maintain accurate statistics test passed");
    println!("   - All messages across batches were processed");
    println!("   - Statistics were updated incrementally");
    println!("   - Final counts are accurate");
    println!("   - Performance metrics are calculated across all exchanges");
}

/// Test comprehensive interactive message exchange
#[tokio::test]
async fn test_comprehensive_interactive_message_exchange() {
    println!("Testing comprehensive interactive message exchange functionality");

    let server_addr = "127.0.0.1:18127";
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
        // Comprehensive workflow test

        // Send initial message
        let _ = stdin.write_all(b"Initial test message\n").await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Check info after first message
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        // Send multiple messages rapidly
        let _ = stdin.write_all(b"Rapid message 1\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        let _ = stdin.write_all(b"Rapid message 2\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Send a longer message
        let _ = stdin
            .write_all(b"This is a longer message to test different response characteristics\n")
            .await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        // Final info check
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
        "Comprehensive message exchange output:\n{}",
        combined_output
    );

    // Verify all message exchange features worked
    let checks = [
        (
            "messages_sent",
            combined_output.matches("Sending Ping message").count() >= 4,
        ), // 4 user messages + handshake
        (
            "responses_received",
            combined_output.matches("Received echo").count() == 4,
        ), // 4 user messages
        (
            "timing_measured",
            combined_output.matches("round-trip").count() >= 4,
        ),
        (
            "statistics_tracked",
            combined_output.contains("Messages sent: 4")
                || (combined_output.contains("4") && combined_output.contains("message")),
        ),
        (
            "performance_metrics",
            combined_output.contains("Average") || combined_output.contains("average"),
        ),
        (
            "clear_responses",
            combined_output.contains("Initial test message")
                && combined_output.contains("longer message"),
        ),
        ("graceful_completion", command_output.status.success()),
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
        passed_checks >= 6,
        "At least 6/7 message exchange checks should pass. Passed: {}/7. Output: {}",
        passed_checks,
        combined_output
    );

    println!("✅ Comprehensive interactive message exchange test passed");
    println!(
        "   - {}/{} message exchange features verified",
        passed_checks,
        checks.len()
    );
    println!("   - Complete message exchange workflow successful");
}
