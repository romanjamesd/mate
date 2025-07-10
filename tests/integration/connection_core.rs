//! Core connection establishment tests
//!
//! This module contains integration tests for the core connection functionality,
//! including connection establishment, failure handling, peer identification,
//! and proper connection cleanup.

use mate::crypto::Identity;
use mate::messages::Message;
use mate::network::{Client, Server};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Test successful connection establishment with valid server address
#[tokio::test]
async fn test_successful_connection_establishment() {
    // Create test identities
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    // Start server on ephemeral port
    let server = Server::bind("127.0.0.1:0", server_identity).await.unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    // Start server in background
    let server_handle = tokio::spawn(async move { server.run().await });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create client and attempt connection
    let client = Client::new(client_identity);
    let connection_result = client.connect(&server_addr).await;

    // Verify connection succeeded
    assert!(
        connection_result.is_ok(),
        "Connection should succeed to valid server"
    );

    let mut connection = connection_result.unwrap();

    // Verify connection state
    assert!(
        connection.is_authenticated(),
        "Connection should be authenticated after handshake"
    );
    assert!(
        connection.peer_identity().is_some(),
        "Peer identity should be available"
    );

    // Clean up
    let _ = connection.close().await;
    server_handle.abort();
}

/// Test connection failure with invalid server address
#[tokio::test]
async fn test_connection_failure_invalid_address() {
    let client_identity = Arc::new(Identity::generate().unwrap());
    let client = Client::new(client_identity);

    // Attempt connection to invalid address
    let connection_result = client.connect("invalid_address:8080").await;

    // Verify connection failed
    assert!(
        connection_result.is_err(),
        "Connection should fail with invalid address"
    );

    let error = connection_result.err().unwrap();
    let error_msg = error.to_string();

    // Error should indicate connection failure
    assert!(
        error_msg.contains("Failed to connect")
            || error_msg.contains("invalid")
            || error_msg.contains("resolve"),
        "Error message should indicate connection failure: {}",
        error_msg
    );
}

/// Test connection failure when server is not running
#[tokio::test]
async fn test_connection_failure_server_not_running() {
    let client_identity = Arc::new(Identity::generate().unwrap());
    let client = Client::new(client_identity);

    // Attempt connection to valid address format but no server running
    let connection_result = client.connect("127.0.0.1:58734").await; // Unlikely to have server on this port

    // Verify connection failed
    assert!(
        connection_result.is_err(),
        "Connection should fail when server is not running"
    );

    let error = connection_result.err().unwrap();
    let error_msg = error.to_string();

    // Error should indicate connection refused or timeout
    assert!(
        error_msg.contains("refused")
            || error_msg.contains("timeout")
            || error_msg.contains("Failed to connect"),
        "Error message should indicate server not available: {}",
        error_msg
    );
}

/// Test that peer identification information is available after successful connection
#[tokio::test]
async fn test_peer_identification_after_connection() {
    // Create test identities with known peer IDs
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    let expected_server_peer_id = server_identity.peer_id().to_string();

    // Start server
    let server = Server::bind("127.0.0.1:0", server_identity).await.unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect client
    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    // Verify peer identification is available and correct
    assert!(
        connection.peer_identity().is_some(),
        "Peer identity should be available"
    );

    let actual_peer_id = connection.peer_identity().unwrap();
    assert_eq!(
        actual_peer_id, expected_server_peer_id,
        "Peer ID should match the server's identity"
    );

    // Verify the peer ID is not empty or generic
    assert!(!actual_peer_id.is_empty(), "Peer ID should not be empty");
    assert!(
        actual_peer_id != "unknown",
        "Peer ID should not be placeholder value"
    );

    // Clean up
    let _ = connection.close().await;
    server_handle.abort();
}

/// Test that client identity is properly used for connection
#[tokio::test]
async fn test_client_identity_usage() {
    // Create specific test identities
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    let _client_peer_id = client_identity.peer_id().to_string();

    // Start server
    let server = Server::bind("127.0.0.1:0", server_identity).await.unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create client with specific identity
    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    // Test that client can send messages (proving identity is properly used)
    let test_message = Message::new_ping(42, "test_identity".to_string());
    let send_result = connection.send_message(test_message).await;

    assert!(
        send_result.is_ok(),
        "Client should be able to send messages with its identity"
    );

    // Test receiving response (proving full handshake with client identity worked)
    let receive_result = timeout(Duration::from_secs(2), connection.receive_message()).await;
    assert!(receive_result.is_ok(), "Should be able to receive response");

    let (response, _sender) = receive_result.unwrap().unwrap();
    assert_eq!(
        response.get_payload(),
        "test_identity",
        "Should receive echo of sent message"
    );

    // Clean up
    let _ = connection.close().await;
    server_handle.abort();
}

/// Test that connections are properly closed on exit
#[tokio::test]
async fn test_connection_proper_cleanup() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    // Start server
    let server = Server::bind("127.0.0.1:0", server_identity).await.unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create and establish connection
    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    // Verify connection is initially active
    assert!(
        connection.is_authenticated(),
        "Connection should be authenticated"
    );
    assert!(
        !connection.is_closed(),
        "Connection should not be closed initially"
    );

    // Explicitly close the connection
    let close_result = connection.close().await;
    assert!(close_result.is_ok(), "Connection close should succeed");

    // Verify connection state after close
    assert!(
        connection.is_closed(),
        "Connection should be marked as closed"
    );
    assert!(
        !connection.is_authenticated(),
        "Connection should not be authenticated after close"
    );

    // Verify peer identity is cleared after close
    assert!(
        connection.peer_identity().is_none(),
        "Peer identity should be cleared after close"
    );

    // Clean up server
    server_handle.abort();
}

/// Test connection behavior with multiple sequential connections
#[tokio::test]
async fn test_multiple_sequential_connections() {
    let server_identity = Arc::new(Identity::generate().unwrap());

    // Start server
    let server = Server::bind("127.0.0.1:0", server_identity).await.unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test multiple connections from same client
    let client_identity = Arc::new(Identity::generate().unwrap());
    let client = Client::new(client_identity);

    for i in 0..3 {
        let mut connection = client
            .connect(&server_addr)
            .await
            .unwrap_or_else(|_| panic!("Connection {} should succeed", i + 1));

        // Verify each connection is independent and functional
        assert!(
            connection.is_authenticated(),
            "Each connection should be authenticated"
        );
        assert!(
            connection.peer_identity().is_some(),
            "Each connection should have peer identity"
        );

        // Test message exchange on each connection
        let test_message = Message::new_ping(i, format!("test_message_{i}"));
        connection
            .send_message(test_message)
            .await
            .expect("Should send message");

        let (response, _) = timeout(Duration::from_secs(2), connection.receive_message())
            .await
            .expect("Should receive response within timeout")
            .expect("Should receive valid response");

        assert_eq!(
            response.get_payload(),
            format!("test_message_{i}"),
            "Should receive correct echo"
        );

        // Clean up connection
        let _ = connection.close().await;
    }

    // Clean up server
    server_handle.abort();
}

/// Test connection establishment with different client identities
#[tokio::test]
async fn test_multiple_client_identities() {
    let server_identity = Arc::new(Identity::generate().unwrap());

    // Start server
    let server = Server::bind("127.0.0.1:0", server_identity).await.unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test connections with different client identities
    for i in 0..3 {
        let client_identity = Arc::new(Identity::generate().unwrap());
        let _expected_peer_id = client_identity.peer_id().to_string();
        let client = Client::new(client_identity);

        let mut connection = client
            .connect(&server_addr)
            .await
            .unwrap_or_else(|_| panic!("Connection with identity {} should succeed", i + 1));

        // Verify connection uses the correct identity
        assert!(
            connection.is_authenticated(),
            "Connection should be authenticated"
        );

        // Send a message to verify the identity is properly used
        let test_message = Message::new_ping(100 + i, format!("identity_test_{i}"));
        connection
            .send_message(test_message)
            .await
            .expect("Should send message");

        let (response, _) = timeout(Duration::from_secs(2), connection.receive_message())
            .await
            .expect("Should receive response")
            .expect("Should receive valid response");

        assert_eq!(
            response.get_payload(),
            format!("identity_test_{i}"),
            "Should receive correct echo"
        );

        // Clean up
        let _ = connection.close().await;
    }

    // Clean up server
    server_handle.abort();
}
