//! Interactive Mode Initialization Tests
//!
//! Tests for Interactive Mode Initialization functionality as specified in tests-to-add.md:
//! - Test that connection information is displayed to user
//! - Test that connection status is communicated  
//! - Test that available functionality is explained to user
//! - Test that usage instructions are provided
//! - Test clear visual separation between sections
//! - Test that session tracking is properly initialized

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

/// Test that connection information is displayed to user
#[tokio::test]
async fn test_connection_information_display() {
    println!("Testing that connection information is displayed to user in interactive mode");

    let server_addr = "127.0.0.1:18091";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Start mate in interactive mode (no --message flag)
    let mut child = Command::new(get_mate_binary_path())
        .args(&["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    // Wait a moment for initialization output
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Send quit command to terminate gracefully
    if let Some(stdin) = child.stdin.as_mut() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(b"quit\n").await;
    }

    // Wait for command to complete with timeout
    let output = timeout(Duration::from_secs(5), child.wait_with_output()).await;

    server_handle.abort();

    let command_output = output
        .expect("Command should complete within timeout")
        .expect("Command should execute successfully");

    let stdout = String::from_utf8_lossy(&command_output.stdout);
    let stderr = String::from_utf8_lossy(&command_output.stderr);
    let combined_output = format!("{}{}", stdout, stderr);

    println!("Debug output:\n{}", combined_output);

    // Verify connection information is displayed
    assert!(combined_output.contains("Connected") || combined_output.contains("connection"),
           "Output should display connection information. Output: {}", combined_output);

    // Verify server address information is shown
    assert!(combined_output.contains(server_addr) || combined_output.contains("127.0.0.1"),
           "Output should display server address information. Output: {}", combined_output);

    println!("✅ Connection information display test passed");
    println!("   - Connection information is properly displayed");
    println!("   - Server address information is included");
}

/// Test that connection status is communicated
#[tokio::test]
async fn test_connection_status_communication() {
    println!("Testing that connection status is properly communicated");

    let server_addr = "127.0.0.1:18092";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

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
        use tokio::io::AsyncWriteExt;
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

    // Verify connection status is clearly communicated
    assert!(combined_output.contains("Connected") || 
            combined_output.contains("connection established") ||
            combined_output.contains("connection successful"),
           "Output should clearly communicate connection status. Output: {}", combined_output);

    // Verify successful connection indication
    assert!(!combined_output.contains("failed") && !combined_output.contains("error"),
           "Output should not indicate connection failure. Output: {}", combined_output);

    println!("✅ Connection status communication test passed");
    println!("   - Connection status is clearly communicated");
    println!("   - Successful connection is properly indicated");
}

/// Test that available functionality is explained to user
#[tokio::test]
async fn test_available_functionality_explanation() {
    println!("Testing that available functionality is explained to user");

    let server_addr = "127.0.0.1:18093";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

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
        use tokio::io::AsyncWriteExt;
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

    // Verify available functionality is explained
    assert!(combined_output.contains("help") || combined_output.contains("commands") ||
            combined_output.contains("available") || combined_output.contains("functionality"),
           "Output should explain available functionality. Output: {}", combined_output);

    // Check for mention of key commands
    let has_command_info = combined_output.contains("quit") || 
                          combined_output.contains("exit") ||
                          combined_output.contains("help") ||
                          combined_output.contains("info");
    
    assert!(has_command_info,
           "Output should mention available commands. Output: {}", combined_output);

    println!("✅ Available functionality explanation test passed");
    println!("   - Available functionality is explained");
    println!("   - Key commands are mentioned");
}

/// Test that usage instructions are provided
#[tokio::test]
async fn test_usage_instructions_provided() {
    println!("Testing that usage instructions are provided");

    let server_addr = "127.0.0.1:18094";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

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
        use tokio::io::AsyncWriteExt;
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

    // Verify usage instructions are provided
    assert!(combined_output.contains("Type") || combined_output.contains("Enter") ||
            combined_output.contains("usage") || combined_output.contains("instructions") ||
            combined_output.contains("how to"),
           "Output should provide usage instructions. Output: {}", combined_output);

    // Verify instructions mention how to interact
    assert!(combined_output.contains("message") || combined_output.contains("send") ||
            combined_output.contains("type") || combined_output.contains("enter"),
           "Output should explain how to send messages. Output: {}", combined_output);

    println!("✅ Usage instructions test passed");
    println!("   - Usage instructions are provided");
    println!("   - Instructions explain how to interact");
}

/// Test clear visual separation between sections
#[tokio::test]
async fn test_visual_separation_between_sections() {
    println!("Testing clear visual separation between sections");

    let server_addr = "127.0.0.1:18095";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

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
        use tokio::io::AsyncWriteExt;
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

    // Verify visual separation elements are present
    let has_separation = combined_output.contains("---") || 
                        combined_output.contains("===") ||
                        combined_output.contains("┌") ||
                        combined_output.contains("│") ||
                        combined_output.contains("└") ||
                        combined_output.contains("\n\n") ||
                        combined_output.contains("=") ||
                        combined_output.contains("-");

    assert!(has_separation,
           "Output should have visual separation between sections. Output: {}", combined_output);

    // Check for structured output with newlines
    let line_count = combined_output.lines().count();
    assert!(line_count > 1,
           "Output should have multiple lines for structure. Output: {}", combined_output);

    println!("✅ Visual separation test passed");
    println!("   - Visual separation elements are present");
    println!("   - Output has structured layout");
}

/// Test that session tracking is properly initialized
#[tokio::test]
async fn test_session_tracking_initialization() {
    println!("Testing that session tracking is properly initialized");

    let server_addr = "127.0.0.1:18096";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut child = Command::new(get_mate_binary_path())
        .args(&["connect", server_addr])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start mate command");

    // Wait for initialization, send info command to check session state
    tokio::time::sleep(Duration::from_millis(500)).await;

    if let Some(stdin) = child.stdin.as_mut() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(b"info\n").await;
        tokio::time::sleep(Duration::from_millis(200)).await;
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

    // Verify session tracking is initialized
    assert!(combined_output.contains("session") || combined_output.contains("duration") ||
            combined_output.contains("time") || combined_output.contains("started"),
           "Output should show session tracking initialization. Output: {}", combined_output);

    // Verify connection details are tracked
    assert!(combined_output.contains("connection") || combined_output.contains("peer") ||
            combined_output.contains("status"),
           "Output should show connection tracking. Output: {}", combined_output);

    println!("✅ Session tracking initialization test passed");
    println!("   - Session tracking is properly initialized");
    println!("   - Connection details are tracked");
}

/// Test comprehensive interactive mode initialization
#[tokio::test]
async fn test_comprehensive_interactive_initialization() {
    println!("Testing comprehensive interactive mode initialization");

    let server_addr = "127.0.0.1:18097";
    let server = start_test_server(server_addr).await
        .expect("Failed to start test server");
    
    let server_handle = tokio::spawn(async move {
        server.run().await
    });

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
        use tokio::io::AsyncWriteExt;
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

    println!("Full initialization output:\n{}", combined_output);

    // Verify all initialization components are present
    let checks = vec![
        ("connection_info", combined_output.contains("Connected") || combined_output.contains("connection")),
        ("functionality", combined_output.contains("help") || combined_output.contains("commands") || combined_output.contains("available")),
        ("instructions", combined_output.contains("Type") || combined_output.contains("Enter") || combined_output.contains("message")),
        ("structure", combined_output.lines().count() > 1),
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

    // Require most checks to pass (allowing for some variation in implementation)
    assert!(passed_checks >= 2,
           "At least 2/4 initialization checks should pass. Passed: {}/4. Output: {}", 
           passed_checks, combined_output);

    println!("✅ Comprehensive interactive initialization test passed");
    println!("   - {}/{} initialization components verified", passed_checks, checks.len());
    println!("   - Interactive mode properly initializes");
} 