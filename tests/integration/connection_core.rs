//! Core connection establishment tests
//!
//! This module contains integration tests for the core connection functionality,
//! including connection establishment, failure handling, peer identification,
//! and proper connection cleanup.

use crate::common::test_helpers::test_server_database;
use mate::crypto::Identity;
use mate::messages::wire::{FramedMessage, WireConfig};
use mate::messages::{Message, SignedEnvelope};
use mate::network::{Client, Connection, ConnectionError, Server};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

/// Keep the peer's raw stream so tests can change signers after a real handshake.
/// Both halves run together without detached tasks or timing assumptions.
async fn authenticated_raw_peer(inbound: bool, peer: &Identity) -> (Connection, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (client, server) = tokio::join!(
        TcpStream::connect(listener.local_addr().unwrap()),
        listener.accept()
    );
    let (local, mut raw) = if inbound {
        (server.unwrap().0, client.unwrap())
    } else {
        (client.unwrap(), server.unwrap().0)
    };
    let mut connection = Connection::new(local, Arc::new(Identity::generate().unwrap())).await;
    assert_eq!(connection.peer_identity(), None);
    let framing = FramedMessage::new(WireConfig::default());
    let handshake = async {
        if inbound {
            connection.handle_handshake_request().await
        } else {
            connection.handshake().await
        }
    };
    let raw_handshake = async {
        if inbound {
            let request = Message::new_ping(42, format!("HANDSHAKE_REQUEST:{}", peer.peer_id()));
            let envelope = SignedEnvelope::create(&request, peer, None).unwrap();
            framing.write_message(&mut raw, &envelope).await.unwrap();
            let response = framing.read_message(&mut raw).await.unwrap();
            assert!(response.verify_signature());
            assert_eq!(response.get_message().unwrap().get_nonce(), 42);
        } else {
            let request = framing.read_message(&mut raw).await.unwrap();
            assert!(request.verify_signature());
            let response = Message::new_pong(
                request.get_message().unwrap().get_nonce(),
                format!("HANDSHAKE_RESPONSE:{}", peer.peer_id()),
            );
            let envelope = SignedEnvelope::create(&response, peer, None).unwrap();
            framing.write_message(&mut raw, &envelope).await.unwrap();
        }
    };
    let (result, ()) = tokio::join!(handshake, raw_handshake);
    assert_eq!(result.unwrap(), peer.peer_id().as_str());
    assert_eq!(connection.peer_identity(), Some(peer.peer_id().as_str()));
    (connection, raw)
}

#[tokio::test]
async fn authenticated_connection_binds_verified_signer_in_both_directions() {
    timeout(Duration::from_secs(10), async {
        for inbound in [true, false] {
            let peer = Identity::generate().unwrap();
            let other = Identity::generate().unwrap();
            let (mut connection, mut raw) = authenticated_raw_peer(inbound, &peer).await;
            let framing = FramedMessage::new(WireConfig::default());
            let message = Message::new_ping(123, "authenticated message".to_string());
            let envelope = SignedEnvelope::create(&message, &peer, None).unwrap();
            framing.write_message(&mut raw, &envelope).await.unwrap();
            let (received, sender) = connection.receive_message().await.unwrap();
            assert!(received.is_ping());
            assert_eq!(received.get_nonce(), message.get_nonce());
            assert_eq!(received.get_payload(), message.get_payload());
            assert_eq!(sender, peer.peer_id().as_str());

            let envelope = SignedEnvelope::create(&message, &other, None).unwrap();
            framing.write_message(&mut raw, &envelope).await.unwrap();
            match connection.receive_message().await.unwrap_err() {
                ConnectionError::PeerIdentityMismatch { expected, actual } => {
                    assert_eq!(expected, peer.peer_id().as_str());
                    assert_eq!(actual, other.peer_id().as_str());
                }
                error => panic!("expected signer mismatch, got {error:?}"),
            }
            assert_eq!(connection.peer_identity(), Some(peer.peer_id().as_str()));
            assert!(connection.is_authenticated());
        }
    })
    .await
    .expect("signer binding test timed out");
}

#[tokio::test]
async fn untrusted_envelopes_retain_errors_before_signer_binding() {
    timeout(Duration::from_secs(10), async {
        for inbound in [true, false] {
            for stale in [false, true] {
                let peer = Identity::generate().unwrap();
                let other = Identity::generate().unwrap();
                let (mut connection, mut raw) = authenticated_raw_peer(inbound, &peer).await;
                let message = Message::new_ping(123, "untrusted message".to_string());
                let mut envelope = SignedEnvelope::create(
                    &message,
                    &other,
                    if stale { Some(1_000) } else { None },
                )
                .unwrap();
                if !stale {
                    envelope.signature[0] ^= 0xff;
                }
                FramedMessage::new(WireConfig::default())
                    .write_message(&mut raw, &envelope)
                    .await
                    .unwrap();
                let error = connection.receive_message().await.unwrap_err();
                if stale {
                    assert!(
                        matches!(error, ConnectionError::InvalidTimestamp),
                        "{error:?}"
                    );
                } else {
                    assert!(
                        matches!(error, ConnectionError::InvalidSignature),
                        "{error:?}"
                    );
                }
                assert_eq!(connection.peer_identity(), Some(peer.peer_id().as_str()));
            }
        }
    })
    .await
    .expect("envelope validation test timed out");
}

/// Test successful connection establishment with valid server address
#[tokio::test]
async fn test_successful_connection_establishment() {
    // Create test identities
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    // Start server on ephemeral port
    let database = test_server_database(server_identity.peer_id().as_str());
    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    // Exercise hostname resolution and all resolved addresses (IPv4/IPv6 localhost).
    let server_addr = format!("localhost:{}", server.local_addr().unwrap().port());

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
    let database = test_server_database(server_identity.peer_id().as_str());
    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
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
    let database = test_server_database(server_identity.peer_id().as_str());
    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
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
    let database = test_server_database(server_identity.peer_id().as_str());
    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
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
    let database = test_server_database(server_identity.peer_id().as_str());
    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
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
    let database = test_server_database(server_identity.peer_id().as_str());
    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
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
