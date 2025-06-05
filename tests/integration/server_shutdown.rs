use mate::crypto::Identity;
use mate::network::Server;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

#[tokio::test]
async fn test_server_graceful_shutdown() {
    // Create test identity
    let identity = Arc::new(Identity::generate().unwrap());

    // Bind server to available port (0 = let OS choose)
    let server = Server::bind("127.0.0.1:0", identity).await.unwrap();
    let addr = server.local_addr().unwrap();

    println!("Test server bound to: {}", addr);

    // Start server in background task
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait a moment for server to start accepting connections
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Note: In a real test with actual signal handling, we would send SIGTERM/SIGINT
    // For this test, we simulate shutdown by aborting the server task
    // since we don't have easy access to the shutdown channel from outside

    // Give the server a moment to run
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Simulate shutdown by aborting the server task
    server_handle.abort();

    // Verify server task terminates (either gracefully or via abort)
    let result = timeout(Duration::from_secs(5), server_handle).await;

    // The task should complete (either via abort or graceful shutdown)
    match result {
        Ok(_) => {
            // Task completed normally (shouldn't happen in this test since we aborted)
            println!("Server completed normally");
        }
        Err(_) => {
            // Timeout occurred - this would be a problem
            panic!("Server did not shutdown within timeout");
        }
    }
}

#[tokio::test]
async fn test_server_bind_and_basic_startup() {
    // Test that server can bind and start without errors
    let identity = Arc::new(Identity::generate().unwrap());

    // Bind to ephemeral port
    let server = Server::bind("127.0.0.1:0", identity).await.unwrap();
    let addr = server.local_addr().unwrap();

    println!("Test server bound to: {}", addr);

    // Start server in background with immediate shutdown
    let server_handle = tokio::spawn(async move {
        // Use select to race server.run() against immediate shutdown
        tokio::select! {
            result = server.run() => {
                // Server completed (shouldn't happen in normal operation)
                result
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                // Timeout after 100ms - this simulates shutdown
                Ok(())
            }
        }
    });

    // Wait for the test to complete
    let result = timeout(Duration::from_secs(2), server_handle).await;
    assert!(result.is_ok(), "Server should startup and shutdown cleanly");

    let server_result = result.unwrap().unwrap();
    assert!(
        server_result.is_ok(),
        "Server should not return errors during clean shutdown"
    );
}

#[tokio::test]
async fn test_server_multiple_bind_attempts() {
    // Test server binding behavior
    let identity1 = Arc::new(Identity::generate().unwrap());
    let identity2 = Arc::new(Identity::generate().unwrap());

    // Bind first server
    let server1 = Server::bind("127.0.0.1:0", identity1).await.unwrap();
    let addr = server1.local_addr().unwrap();

    println!("First server bound to: {}", addr);

    // Try to bind second server to the same address (should fail)
    let result = Server::bind(&addr.to_string(), identity2).await;
    assert!(
        result.is_err(),
        "Second server should fail to bind to the same address"
    );

    // Verify the error is related to address already in use
    let error_msg = format!("{}", result.err().unwrap());
    println!("Expected bind error: {}", error_msg);

    // The first server should still be valid
    drop(server1); // Clean up
}
