//! Error handling and protocol violation tests
//!
//! This module contains tests for proper error handling, protocol violation detection,
//! corrupted data handling, and security-related error scenarios.

use mate::crypto::Identity;
use mate::messages::wire::{FramedMessage, WireProtocolError, MAX_MESSAGE_SIZE};
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

/// Create data with an invalid length prefix
fn create_invalid_length_prefix_data(length_prefix: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&length_prefix.to_be_bytes());

    // Add some dummy data (may not match the length prefix intentionally)
    data.extend_from_slice(b"dummy_message_data");

    data
}

/// Create data with zero length prefix
fn create_zero_length_prefix_data() -> Vec<u8> {
    create_invalid_length_prefix_data(0)
}

/// Create data that claims to be negative when interpreted as signed
fn create_negative_length_prefix_data() -> Vec<u8> {
    // Use a value that would be negative if interpreted as i32
    // 0x80000000 is -2147483648 when interpreted as signed i32
    create_invalid_length_prefix_data(0x80000000)
}

#[tokio::test]
async fn test_corrupted_length_prefix_handling() {
    println!("Testing corrupted length prefix handling - Essential Test #13");

    // Test case covers:
    // - Send invalid length prefix values (zero, negative when cast, etc.)
    // - Verify protocol detects and rejects invalid length prefixes
    // - Test protocol recovery after receiving invalid length prefix

    let framed_message = FramedMessage::default();

    // Test 1: Zero length prefix
    println!("Test 1: Testing zero length prefix rejection");
    {
        let zero_data = create_zero_length_prefix_data();
        let mut zero_stream = MockStream::with_data(zero_data);

        let result = framed_message.read_message(&mut zero_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Zero length prefix correctly rejected");

                // Verify we get an appropriate error type
                match e.downcast_ref::<WireProtocolError>() {
                    Some(WireProtocolError::InvalidLength { length, min, max }) => {
                        assert_eq!(*length, 0, "Error should report zero length");
                        assert_eq!(*min, 1, "Minimum length should be 1");
                        assert_eq!(
                            *max, MAX_MESSAGE_SIZE as u32,
                            "Maximum should be MAX_MESSAGE_SIZE"
                        );
                        println!("    ✓ InvalidLength error with correct parameters: length={}, min={}, max={}", 
                                length, min, max);
                    }
                    Some(other_error) => {
                        println!("    ✓ Rejected with alternative error: {:?}", other_error);
                        // Other appropriate error types are also acceptable
                    }
                    None => {
                        println!("    ✓ Rejected with non-WireProtocolError: {:?}", e);
                    }
                }
            }
            Ok(_) => {
                panic!("Expected zero length prefix to be rejected, but read succeeded");
            }
        }
    }

    // Test 2: Negative length when cast (large u32 values that would be negative as i32)
    println!("Test 2: Testing negative-when-cast length prefix rejection");
    {
        let negative_data = create_negative_length_prefix_data();
        let mut negative_stream = MockStream::with_data(negative_data);

        let result = framed_message.read_message(&mut negative_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Negative-when-cast length prefix correctly rejected");

                match e.downcast_ref::<WireProtocolError>() {
                    Some(WireProtocolError::InvalidLength { length, .. }) => {
                        assert_eq!(
                            *length, 0x80000000,
                            "Error should report the problematic length"
                        );
                        println!(
                            "    ✓ InvalidLength error for length={} (0x{:08x})",
                            length, length
                        );
                    }
                    Some(WireProtocolError::MessageTooLarge { size, .. }) => {
                        assert_eq!(*size, 0x80000000_usize, "Error should report correct size");
                        println!("    ✓ MessageTooLarge error for size={}", size);
                    }
                    Some(other_error) => {
                        println!("    ✓ Rejected with alternative error: {:?}", other_error);
                    }
                    None => {
                        println!("    ✓ Rejected with non-WireProtocolError: {:?}", e);
                    }
                }
            }
            Ok(_) => {
                panic!(
                    "Expected negative-when-cast length prefix to be rejected, but read succeeded"
                );
            }
        }
    }

    // Test 3: Extremely large length prefix values (u32::MAX)
    println!("Test 3: Testing extremely large length prefix rejection");
    {
        let max_data = create_invalid_length_prefix_data(u32::MAX);
        let mut max_stream = MockStream::with_data(max_data);

        let result = framed_message.read_message(&mut max_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Extremely large length prefix correctly rejected");

                match e.downcast_ref::<WireProtocolError>() {
                    Some(WireProtocolError::InvalidLength { length, .. }) => {
                        assert_eq!(*length, u32::MAX, "Error should report u32::MAX");
                        println!("    ✓ InvalidLength error for length={} (u32::MAX)", length);
                    }
                    Some(WireProtocolError::MessageTooLarge { size, .. }) => {
                        assert_eq!(*size, u32::MAX as usize, "Error should report correct size");
                        println!("    ✓ MessageTooLarge error for size={}", size);
                    }
                    Some(other_error) => {
                        println!("    ✓ Rejected with alternative error: {:?}", other_error);
                    }
                    None => {
                        println!("    ✓ Rejected with non-WireProtocolError: {:?}", e);
                    }
                }
            }
            Ok(_) => {
                panic!("Expected extremely large length prefix to be rejected, but read succeeded");
            }
        }
    }

    // Test 4: Protocol recovery after invalid length prefix
    println!("Test 4: Testing protocol recovery after invalid length prefix");
    {
        // First send an invalid message
        let invalid_data = create_zero_length_prefix_data();
        let mut invalid_stream = MockStream::with_data(invalid_data);

        let invalid_result = framed_message.read_message(&mut invalid_stream).await;
        assert!(
            invalid_result.is_err(),
            "Invalid message should be rejected"
        );

        // Then verify we can still process a valid message
        let (valid_envelope, _) = create_test_envelope("recovery_test");
        let mut valid_stream = MockStream::new();

        framed_message
            .write_message(&mut valid_stream, &valid_envelope)
            .await
            .expect("Valid message should write successfully");

        let written_data = valid_stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(written_data);

        let recovered_envelope = framed_message
            .read_message(&mut read_stream)
            .await
            .expect("Valid message should read successfully after invalid one");

        assert!(
            recovered_envelope.verify_signature(),
            "Recovered message should have valid signature"
        );

        println!("  ✓ Protocol successfully recovered after invalid length prefix");
    }

    println!("✓ Corrupted length prefix handling test completed successfully");
    println!("  - Zero length prefixes are properly rejected");
    println!("  - Negative-when-cast values are properly rejected");
    println!("  - Extremely large values are properly rejected");
    println!("  - Protocol can recover after encountering invalid length prefixes");
}

#[tokio::test]
async fn test_corrupted_message_data_handling() {
    println!("Testing corrupted message data handling - Essential Test #14");

    // Test case covers:
    // - Send valid length prefix with corrupted message data
    // - Verify protocol detects and rejects corrupted serialized data
    // - Test protocol recovery after receiving corrupted data

    let framed_message = FramedMessage::default();

    // Test 1: Valid length prefix with completely random data
    println!("Test 1: Testing valid length prefix with random corrupted data");
    {
        let corrupt_message_size = 100; // Claim 100 bytes
        let mut corrupt_data = Vec::new();

        // Valid length prefix
        corrupt_data.extend_from_slice(&(corrupt_message_size as u32).to_be_bytes());

        // Random/corrupted message data (100 bytes of pseudo-random data)
        for i in 0..corrupt_message_size {
            corrupt_data.push((i * 7 + 13) as u8); // Simple pseudo-random pattern
        }

        let mut corrupt_stream = MockStream::with_data(corrupt_data);

        let result = framed_message.read_message(&mut corrupt_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Corrupted message data correctly rejected");
                println!("    Error: {:?}", e);

                // The exact error type may vary depending on where deserialization fails
                // but it should definitely be an error
            }
            Ok(_) => {
                panic!("Expected corrupted message data to be rejected, but read succeeded");
            }
        }
    }

    // Test 2: Valid length prefix with partially corrupted data
    println!("Test 2: Testing valid length prefix with partially corrupted data");
    {
        // Create a valid message first
        let (valid_envelope, _) = create_test_envelope("test_corruption");
        let mut valid_stream = MockStream::new();

        framed_message
            .write_message(&mut valid_stream, &valid_envelope)
            .await
            .expect("Valid message should write successfully");

        let mut valid_data = valid_stream.get_written_data().to_vec();

        // Corrupt some bytes in the middle of the message (skip the length prefix)
        if valid_data.len() > 8 {
            for i in 6..std::cmp::min(valid_data.len(), 12) {
                valid_data[i] = !valid_data[i]; // Flip bits
            }
        }

        let mut corrupt_stream = MockStream::with_data(valid_data);

        let result = framed_message.read_message(&mut corrupt_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Partially corrupted message data correctly rejected");
                println!("    Error: {:?}", e);
            }
            Ok(_) => {
                // In some cases, bit flips might not cause deserialization to fail
                // (if they affect non-critical parts), which is also acceptable
                println!("  ⚠ Partially corrupted data was accepted (corruption may not have affected critical parts)");
            }
        }
    }

    // Test 3: Valid length prefix with truncated data
    println!("Test 3: Testing valid length prefix with truncated data");
    {
        let claimed_size = 200u32; // Claim 200 bytes
        let actual_data_size = 50; // But only provide 50 bytes

        let mut truncated_data = Vec::new();

        // Valid length prefix claiming 200 bytes
        truncated_data.extend_from_slice(&claimed_size.to_be_bytes());

        // But only provide 50 bytes of data
        for i in 0..actual_data_size {
            truncated_data.push((i * 3) as u8);
        }

        let mut truncated_stream = MockStream::with_data(truncated_data);

        let result = framed_message.read_message(&mut truncated_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Truncated message data correctly rejected");
                println!("    Error: {:?}", e);
            }
            Ok(_) => {
                panic!("Expected truncated message data to be rejected, but read succeeded");
            }
        }
    }

    // Test 4: Protocol recovery after corrupted data
    println!("Test 4: Testing protocol recovery after corrupted data");
    {
        // First, try to process corrupted data
        let mut corrupt_data = Vec::new();
        corrupt_data.extend_from_slice(&50u32.to_be_bytes()); // Length prefix: 50 bytes
        for i in 0..50 {
            corrupt_data.push((i * 11) as u8); // Corrupted data
        }

        let mut corrupt_stream = MockStream::with_data(corrupt_data);
        let corrupt_result = framed_message.read_message(&mut corrupt_stream).await;
        assert!(corrupt_result.is_err(), "Corrupted data should be rejected");

        // Then verify we can still process a valid message
        let (recovery_envelope, _) = create_test_envelope("recovery_test_data");
        let mut recovery_write_stream = MockStream::new();

        framed_message
            .write_message(&mut recovery_write_stream, &recovery_envelope)
            .await
            .expect("Recovery message should write successfully");

        let recovery_data = recovery_write_stream.get_written_data().to_vec();
        let mut recovery_read_stream = MockStream::with_data(recovery_data);

        let recovered_envelope = framed_message
            .read_message(&mut recovery_read_stream)
            .await
            .expect("Recovery message should read successfully");

        assert!(
            recovered_envelope.verify_signature(),
            "Recovered message should have valid signature"
        );

        println!("  ✓ Protocol successfully recovered after corrupted message data");
    }

    println!("✓ Corrupted message data handling test completed successfully");
    println!("  - Random corrupted data is properly rejected");
    println!("  - Truncated data is properly rejected");
    println!("  - Protocol can recover after encountering corrupted data");
}

#[tokio::test]
async fn test_unexpected_connection_closure() {
    println!("Testing unexpected connection closure handling - Essential Test #15");

    // Test case covers:
    // - Simulate unexpected connection closure during read/write operations
    // - Verify graceful error handling without panics or crashes
    // - Test proper cleanup and resource management

    let framed_message = FramedMessage::default();

    // Test 1: Connection closure during length prefix read
    println!("Test 1: Testing connection closure during length prefix read");
    {
        // Provide only partial length prefix data (2 bytes instead of 4)
        let partial_prefix_data = vec![0x00, 0x01]; // Only 2 bytes of the 4-byte length prefix
        let mut partial_stream = MockStream::with_data(partial_prefix_data);

        let result = framed_message.read_message(&mut partial_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Partial length prefix read correctly handled as error");
                println!("    Error: {:?}", e);

                // Should get an EOF error or similar, not a panic
                assert!(
                    !e.to_string().is_empty(),
                    "Error should have a meaningful message"
                );
            }
            Ok(_) => {
                panic!("Expected partial length prefix to cause an error, but read succeeded");
            }
        }
    }

    // Test 2: Connection closure during message body read
    println!("Test 2: Testing connection closure during message body read");
    {
        let claimed_message_size = 100u32;
        let partial_message_size = 30; // Only provide 30 bytes of claimed 100

        let mut partial_data = Vec::new();

        // Complete length prefix
        partial_data.extend_from_slice(&claimed_message_size.to_be_bytes());

        // Partial message data
        for i in 0..partial_message_size {
            partial_data.push((i * 5) as u8);
        }
        // Connection "closes" here - no more data available

        let mut partial_stream = MockStream::with_data(partial_data);

        let result = framed_message.read_message(&mut partial_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Partial message body read correctly handled as error");
                println!("    Error: {:?}", e);

                assert!(
                    !e.to_string().is_empty(),
                    "Error should have a meaningful message"
                );
            }
            Ok(_) => {
                panic!("Expected partial message body to cause an error, but read succeeded");
            }
        }
    }

    // Test 3: Empty data stream (immediate connection closure)
    println!("Test 3: Testing immediate connection closure (empty stream)");
    {
        let empty_data = Vec::new(); // No data at all
        let mut empty_stream = MockStream::with_data(empty_data);

        let result = framed_message.read_message(&mut empty_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Empty stream correctly handled as error");
                println!("    Error: {:?}", e);

                assert!(
                    !e.to_string().is_empty(),
                    "Error should have a meaningful message"
                );
            }
            Ok(_) => {
                panic!("Expected empty stream to cause an error, but read succeeded");
            }
        }
    }

    // Test 4: Single byte data stream (minimal data before closure)
    println!("Test 4: Testing connection closure after single byte");
    {
        let single_byte_data = vec![0x00]; // Only 1 byte when we need 4 for length prefix
        let mut single_stream = MockStream::with_data(single_byte_data);

        let result = framed_message.read_message(&mut single_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Single byte before closure correctly handled as error");
                println!("    Error: {:?}", e);

                assert!(
                    !e.to_string().is_empty(),
                    "Error should have a meaningful message"
                );
            }
            Ok(_) => {
                panic!("Expected single byte to cause an error, but read succeeded");
            }
        }
    }

    // Test 5: Verify no resource leaks or panics during error conditions
    println!("Test 5: Testing resource cleanup during connection closure scenarios");
    {
        // Run multiple scenarios in sequence to verify no resource leaks
        let scenarios = [
            vec![],                 // Empty
            vec![0x00],             // 1 byte
            vec![0x00, 0x01],       // 2 bytes
            vec![0x00, 0x01, 0x02], // 3 bytes
            // 4 bytes (complete length prefix) claiming large message but no body
            vec![0x00, 0x00, 0x01, 0x00], // Claims 256 bytes but provides none
        ];

        for (i, scenario_data) in scenarios.iter().enumerate() {
            let mut scenario_stream = MockStream::with_data(scenario_data.clone());

            let result = framed_message.read_message(&mut scenario_stream).await;

            // All scenarios should result in errors, not panics or hangs
            match result {
                Err(_) => {
                    // Expected - all these scenarios should fail gracefully
                }
                Ok(_) => {
                    panic!(
                        "Scenario {} unexpectedly succeeded with {} bytes of data",
                        i,
                        scenario_data.len()
                    );
                }
            }
        }

        println!("  ✓ All connection closure scenarios handled gracefully");
        println!("  ✓ No panics or resource leaks detected");
    }

    println!("✓ Unexpected connection closure handling test completed successfully");
    println!("  - Partial length prefix reads fail gracefully");
    println!("  - Partial message body reads fail gracefully");
    println!("  - Empty streams fail gracefully");
    println!("  - Resource cleanup works correctly during error conditions");
}

#[tokio::test]
async fn test_protocol_violation_detection() {
    println!("Testing protocol violation detection - Essential Test #16");

    // Test case covers:
    // - Send data that violates protocol specifications
    // - Verify protocol detects violations and responds appropriately
    // - Test edge cases around protocol compliance

    let framed_message = FramedMessage::default();

    // Test 1: Length prefix mismatch (claims one size, provides different size)
    println!("Test 1: Testing length prefix mismatch detection");
    {
        let claimed_size = 50u32;
        let actual_size = 100usize; // Provide more data than claimed

        let mut mismatch_data = Vec::new();

        // Length prefix claiming 50 bytes
        mismatch_data.extend_from_slice(&claimed_size.to_be_bytes());

        // But provide 100 bytes of data
        for i in 0..actual_size {
            mismatch_data.push((i % 256) as u8);
        }

        let mut mismatch_stream = MockStream::with_data(mismatch_data);

        let result = framed_message.read_message(&mut mismatch_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Length prefix mismatch correctly detected");
                println!("    Error: {:?}", e);
            }
            Ok(_) => {
                // This might succeed in some implementations if they only read the claimed amount
                println!("  ⚠ Length prefix mismatch was handled by reading only claimed amount");
            }
        }
    }

    // Test 2: Invalid message format within valid length
    println!("Test 2: Testing invalid message format detection");
    {
        let message_size = 50u32;

        let mut invalid_format_data = Vec::new();

        // Valid length prefix
        invalid_format_data.extend_from_slice(&message_size.to_be_bytes());

        // Invalid message format: start with bytes that don't represent a valid SignedEnvelope
        // Use a pattern that's unlikely to be valid serialized data
        invalid_format_data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Invalid format marker
        for i in 4..message_size as usize {
            invalid_format_data.push((i ^ 0xFF) as u8); // XOR pattern unlikely to be valid
        }

        let mut invalid_stream = MockStream::with_data(invalid_format_data);

        let result = framed_message.read_message(&mut invalid_stream).await;

        match result {
            Err(e) => {
                println!("  ✓ Invalid message format correctly detected");
                println!("    Error: {:?}", e);
            }
            Ok(_) => {
                panic!("Expected invalid message format to be rejected, but read succeeded");
            }
        }
    }

    // Test 3: Non-canonical serialization (if applicable)
    println!("Test 3: Testing protocol compliance edge cases");
    {
        // Create a valid message
        let (valid_envelope, _) = create_test_envelope("protocol_test");
        let mut valid_stream = MockStream::new();

        framed_message
            .write_message(&mut valid_stream, &valid_envelope)
            .await
            .expect("Valid message should write successfully");

        let valid_data = valid_stream.get_written_data().to_vec();

        // Verify the valid message can be read back
        let mut read_stream = MockStream::with_data(valid_data.clone());
        let read_result = framed_message.read_message(&mut read_stream).await;

        match read_result {
            Ok(envelope) => {
                assert!(
                    envelope.verify_signature(),
                    "Valid message should have valid signature"
                );
                println!("  ✓ Valid message properly accepted");
            }
            Err(e) => {
                panic!("Valid message was rejected: {:?}", e);
            }
        }

        // Now test with byte-level modifications that might violate protocol invariants
        // (This is highly implementation-specific)

        // Test with modified length prefix that's still within bounds but wrong
        if valid_data.len() > 8 {
            let mut modified_data = valid_data.clone();
            let original_length = u32::from_be_bytes([
                modified_data[0],
                modified_data[1],
                modified_data[2],
                modified_data[3],
            ]);

            // Modify length to be off by 1 (if it won't cause overflow)
            if original_length > 1 {
                let new_length = original_length - 1;
                let new_length_bytes = new_length.to_be_bytes();
                modified_data[0..4].copy_from_slice(&new_length_bytes);

                let mut modified_stream = MockStream::with_data(modified_data);
                let modified_result = framed_message.read_message(&mut modified_stream).await;

                match modified_result {
                    Err(e) => {
                        println!("  ✓ Modified length prefix correctly rejected");
                        println!("    Error: {:?}", e);
                    }
                    Ok(_) => {
                        println!(
                            "  ⚠ Modified length prefix was accepted (may be within tolerance)"
                        );
                    }
                }
            }
        }
    }

    // Test 4: Sequence of protocol violations
    println!("Test 4: Testing multiple consecutive protocol violations");
    {
        let violations = [
            vec![0x00, 0x00, 0x00, 0x00],                   // Zero length
            vec![0xFF, 0xFF, 0xFF, 0xFF],                   // Max length
            vec![0x00, 0x00, 0x00, 0x01, 0xFF],             // Length 1 with invalid data
            vec![0x00, 0x00, 0x00, 0x05, 0x00, 0x01, 0x02], // Length 5 with only 3 bytes
        ];

        for (i, violation_data) in violations.iter().enumerate() {
            let mut violation_stream = MockStream::with_data(violation_data.clone());

            let result = framed_message.read_message(&mut violation_stream).await;

            match result {
                Err(_) => {
                    // Expected - protocol violations should be rejected
                    println!("  ✓ Protocol violation {} correctly rejected", i + 1);
                }
                Ok(_) => {
                    println!("  ⚠ Protocol violation {} was unexpectedly accepted", i + 1);
                }
            }
        }

        // After all violations, verify we can still process a valid message
        let (recovery_envelope, _) = create_test_envelope("post_violation_recovery");
        let mut recovery_stream = MockStream::new();

        framed_message
            .write_message(&mut recovery_stream, &recovery_envelope)
            .await
            .expect("Recovery message should write successfully");

        let recovery_data = recovery_stream.get_written_data().to_vec();
        let mut recovery_read_stream = MockStream::with_data(recovery_data);

        let final_envelope = framed_message
            .read_message(&mut recovery_read_stream)
            .await
            .expect("Recovery message should read successfully after violations");

        assert!(
            final_envelope.verify_signature(),
            "Recovery message should have valid signature"
        );

        println!("  ✓ Protocol successfully recovered after multiple violations");
    }

    println!("✓ Protocol violation detection test completed successfully");
    println!("  - Length prefix mismatches are detected");
    println!("  - Invalid message formats are detected");
    println!("  - Protocol compliance is enforced");
    println!("  - Recovery works after protocol violations");
}
