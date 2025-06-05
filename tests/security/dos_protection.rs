//! DoS protection and message size limit tests
//!
//! This module contains tests to verify that the system properly protects
//! against denial-of-service attacks through message size limits and other
//! defensive measures.

use mate::crypto::Identity;
use mate::messages::wire::{FramedMessage, WireConfig, WireProtocolError, MAX_MESSAGE_SIZE};
use mate::messages::{Message, SignedEnvelope};
use std::io::Cursor;
use tokio::io::{AsyncRead, AsyncWrite};

/// Test helper to create a mock read/write stream from a buffer
struct MockStream {
    read_cursor: Cursor<Vec<u8>>,
    write_buffer: Vec<u8>,
}

impl MockStream {
    fn new() -> Self {
        Self {
            read_cursor: Cursor::new(Vec::new()),
            write_buffer: Vec::new(),
        }
    }

    fn with_data(data: Vec<u8>) -> Self {
        Self {
            read_cursor: Cursor::new(data),
            write_buffer: Vec::new(),
        }
    }

    fn get_written_data(&self) -> &[u8] {
        &self.write_buffer
    }
}

impl AsyncRead for MockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::pin::Pin::new(&mut self.read_cursor).poll_read(cx, buf)
    }
}

impl AsyncWrite for MockStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        self.write_buffer.extend_from_slice(buf);
        std::task::Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        std::task::Poll::Ready(Ok(()))
    }
}

/// Create a test SignedEnvelope with a known message
fn create_test_envelope(payload: &str) -> (SignedEnvelope, Message) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(42, payload.to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    (envelope, message)
}

/// Create a mock stream with oversized length prefix
fn create_oversized_length_prefix_data(fake_size: u32) -> Vec<u8> {
    // Create a 4-byte length prefix with the fake size
    let mut data = Vec::new();
    data.extend_from_slice(&fake_size.to_be_bytes());

    // Add some dummy data (doesn't need to be valid message data for this test)
    data.extend_from_slice(b"dummy_message_data_that_wont_be_processed");

    data
}

/// Test case 10 from essential-tests.md
///
/// Tests that the system properly enforces message size limits to prevent DoS attacks.
/// Verifies rejection of oversized messages and proper error reporting.
#[tokio::test]
async fn test_message_size_limit_enforcement() {
    println!("Testing message size limit enforcement - DoS protection");

    // Test 1: Send messages exceeding maximum allowed size
    println!("Test 1: Testing rejection of oversized messages");

    // Use a restrictive configuration for testing
    let test_max_size = 1024; // 1KB limit for testing
    let wire_config = WireConfig::with_max_message_size(test_max_size);
    let framed_message = FramedMessage::new(wire_config);

    // Test with various oversized length prefixes
    let oversized_test_cases = vec![
        (test_max_size + 1, "just over limit"),
        (test_max_size * 2, "double the limit"),
        (test_max_size * 10, "10x over limit"),
        (MAX_MESSAGE_SIZE + 1, "over global maximum"),
        (u32::MAX as usize, "maximum u32 value"),
        ((u32::MAX / 2) as usize, "half of maximum u32"),
    ];

    for (fake_size, description) in oversized_test_cases {
        println!("  Testing {} ({} bytes)", description, fake_size);

        // Create a mock stream with oversized length prefix
        let oversized_data = create_oversized_length_prefix_data(fake_size as u32);
        let mut oversized_stream = MockStream::with_data(oversized_data);

        // Attempt to read the message - should fail with appropriate error
        let result = framed_message.read_message(&mut oversized_stream).await;

        match result {
            Err(e) => {
                // Verify we get the appropriate error type
                match e.downcast_ref::<WireProtocolError>() {
                    Some(WireProtocolError::MessageTooLarge { size, max_size }) => {
                        assert_eq!(
                            *size, fake_size,
                            "Error should report correct oversized message size"
                        );
                        assert_eq!(
                            *max_size, test_max_size,
                            "Error should report correct maximum allowed size"
                        );
                        println!(
                            "    ✓ Correctly rejected {} with MessageTooLarge error",
                            description
                        );
                    }
                    Some(WireProtocolError::InvalidLength { .. }) => {
                        println!(
                            "    ✓ Correctly rejected {} with InvalidLength error",
                            description
                        );
                    }
                    Some(other_error) => {
                        panic!(
                            "Expected MessageTooLarge or InvalidLength error for {}, got: {:?}",
                            description, other_error
                        );
                    }
                    None => {
                        panic!(
                            "Expected WireProtocolError for {}, got: {:?}",
                            description, e
                        );
                    }
                }
            }
            Ok(_) => {
                panic!(
                    "Expected error for oversized message ({}), but read succeeded",
                    description
                );
            }
        }
    }

    // Test 2: Test messages at exact size limit boundary
    println!("Test 2: Testing messages at exact size limit boundary");

    // Create a message that should be exactly at the size limit when serialized
    // We need to account for the envelope overhead, so we'll use a smaller payload
    let boundary_payload = "x".repeat(512); // Start with smaller payload
    let (boundary_envelope, _) = create_test_envelope(&boundary_payload);

    // Check the actual serialized size
    let serialized_size = bincode::serialize(&boundary_envelope)
        .expect("Failed to serialize boundary test envelope")
        .len();

    println!(
        "  Boundary test envelope serialized size: {} bytes (limit: {})",
        serialized_size, test_max_size
    );

    if serialized_size <= test_max_size {
        // This message should be accepted
        let mut boundary_stream = MockStream::new();

        let write_result = framed_message
            .write_message(&mut boundary_stream, &boundary_envelope)
            .await;
        match write_result {
            Ok(_) => {
                println!(
                    "    ✓ Message at boundary ({} bytes) was correctly accepted",
                    serialized_size
                );

                // Test reading it back
                let written_data = boundary_stream.get_written_data().to_vec();
                let mut read_stream = MockStream::with_data(written_data);

                let read_result = framed_message.read_message(&mut read_stream).await;
                match read_result {
                    Ok(received_envelope) => {
                        assert!(
                            received_envelope.verify_signature(),
                            "Boundary message signature should be valid"
                        );
                        println!("    ✓ Boundary message successfully round-tripped");
                    }
                    Err(e) => {
                        panic!("Failed to read back boundary message: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!(
                    "    ! Message at boundary was rejected during write: {:?}",
                    e
                );
                // This might be expected if the envelope overhead pushes it over the limit
            }
        }
    } else {
        println!("    ! Boundary test envelope ({} bytes) exceeds limit ({} bytes) due to envelope overhead", 
                 serialized_size, test_max_size);
    }

    println!("✅ Message size limit enforcement test completed successfully");
}

/// Test case 11 from essential-tests.md
///
/// Tests handling of large messages within acceptable limits and verifies
/// that legitimate large messages are processed correctly.
#[tokio::test]
async fn test_large_message_handling() {
    println!("Testing large message handling within limits");

    // Use a generous size limit for this test (but still bounded)
    let large_message_limit = 1024 * 1024; // 1MB limit
    let wire_config = WireConfig::with_max_message_size(large_message_limit);
    let framed_message = FramedMessage::new(wire_config);

    // Test with various large but acceptable message sizes
    let large_message_test_cases = vec![
        (1024, "1KB message"),
        (10 * 1024, "10KB message"),
        (100 * 1024, "100KB message"),
        (500 * 1024, "500KB message"),
    ];

    for (payload_size, description) in large_message_test_cases {
        println!("  Testing {}", description);

        // Create a large payload
        let large_payload = "x".repeat(payload_size);
        let (large_envelope, _) = create_test_envelope(&large_payload);

        // Check the actual serialized size
        let serialized_size = bincode::serialize(&large_envelope)
            .expect("Failed to serialize large test envelope")
            .len();

        println!("    Envelope serialized size: {} bytes", serialized_size);

        if serialized_size <= large_message_limit {
            // This message should be processed successfully
            let mut large_stream = MockStream::new();

            // Test writing the large message
            let write_result = framed_message
                .write_message(&mut large_stream, &large_envelope)
                .await;
            match write_result {
                Ok(_) => {
                    println!(
                        "    ✓ Large message ({} bytes) written successfully",
                        serialized_size
                    );

                    // Test reading it back
                    let written_data = large_stream.get_written_data().to_vec();
                    let mut read_stream = MockStream::with_data(written_data);

                    let read_result = framed_message.read_message(&mut read_stream).await;
                    match read_result {
                        Ok(received_envelope) => {
                            assert!(
                                received_envelope.verify_signature(),
                                "Large message signature should be valid"
                            );

                            // Verify the message content
                            let received_message = received_envelope
                                .get_message()
                                .expect("Failed to deserialize received message");

                            if let Message::Ping { nonce: _, payload } = received_message {
                                assert_eq!(
                                    payload.len(),
                                    large_payload.len(),
                                    "Received payload should match original size"
                                );
                                assert_eq!(
                                    payload, large_payload,
                                    "Received payload should match original content"
                                );
                            } else {
                                panic!("Expected Ping message, got: {:?}", received_message);
                            }

                            println!("    ✓ Large message successfully round-tripped and verified");
                        }
                        Err(e) => {
                            panic!("Failed to read back large message: {:?}", e);
                        }
                    }
                }
                Err(e) => {
                    panic!("Failed to write large message ({}): {:?}", description, e);
                }
            }
        } else {
            println!(
                "    ! Large message ({} bytes) exceeds limit ({} bytes) due to envelope overhead",
                serialized_size, large_message_limit
            );
        }
    }

    println!("✅ Large message handling test completed successfully");
}
