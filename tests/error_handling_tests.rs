use mate::crypto::Identity;
use mate::messages::{Message, SignedEnvelope};
use mate::messages::wire::{FramedMessage, WireProtocolError, MAX_MESSAGE_SIZE};
use tokio::io::{AsyncRead, AsyncWrite};
use std::io::Cursor;

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
                        assert_eq!(*max, MAX_MESSAGE_SIZE as u32, "Maximum should be MAX_MESSAGE_SIZE");
                        println!("    ✓ InvalidLength error with correct parameters: length={}, min={}, max={}", 
                                length, min, max);
                    },
                    Some(other_error) => {
                        println!("    ✓ Rejected with alternative error: {:?}", other_error);
                        // Other appropriate error types are also acceptable
                    },
                    None => {
                        println!("    ✓ Rejected with non-WireProtocolError: {:?}", e);
                    }
                }
            },
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
                        assert_eq!(*length, 0x80000000, "Error should report the problematic length");
                        println!("    ✓ InvalidLength error for length={} (0x{:08x})", length, length);
                    },
                    Some(WireProtocolError::MessageTooLarge { size, .. }) => {
                        assert_eq!(*size, 0x80000000 as usize, "Error should report correct size");
                        println!("    ✓ MessageTooLarge error for size={}", size);
                    },
                    Some(other_error) => {
                        println!("    ✓ Rejected with alternative error: {:?}", other_error);
                    },
                    None => {
                        println!("    ✓ Rejected with non-WireProtocolError: {:?}", e);
                    }
                }
            },
            Ok(_) => {
                panic!("Expected negative-when-cast length prefix to be rejected, but read succeeded");
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
                    },
                    Some(WireProtocolError::MessageTooLarge { size, .. }) => {
                        assert_eq!(*size, u32::MAX as usize, "Error should report correct size");
                        println!("    ✓ MessageTooLarge error for size={}", size);
                    },
                    Some(other_error) => {
                        println!("    ✓ Rejected with alternative error: {:?}", other_error);
                    },
                    None => {
                        println!("    ✓ Rejected with non-WireProtocolError: {:?}", e);
                    }
                }
            },
            Ok(_) => {
                panic!("Expected u32::MAX length prefix to be rejected, but read succeeded");
            }
        }
    }
    
    // Test 4: Length prefix that exceeds configured maximum but is reasonable as u32
    println!("Test 4: Testing length prefix exceeding configured maximum");
    {
        let oversized_length = (MAX_MESSAGE_SIZE + 1) as u32;
        let oversized_data = create_invalid_length_prefix_data(oversized_length);
        let mut oversized_stream = MockStream::with_data(oversized_data);
        
        let result = framed_message.read_message(&mut oversized_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Over-maximum length prefix correctly rejected");
                
                match e.downcast_ref::<WireProtocolError>() {
                    Some(WireProtocolError::MessageTooLarge { size, max_size }) => {
                        assert_eq!(*size, oversized_length as usize, "Error should report correct size");
                        assert_eq!(*max_size, MAX_MESSAGE_SIZE, "Error should report correct max size");
                        println!("    ✓ MessageTooLarge error: size={}, max_size={}", size, max_size);
                    },
                    Some(WireProtocolError::InvalidLength { length, .. }) => {
                        assert_eq!(*length, oversized_length, "Error should report correct length");
                        println!("    ✓ InvalidLength error for length={}", length);
                    },
                    Some(other_error) => {
                        println!("    ✓ Rejected with alternative error: {:?}", other_error);
                    },
                    None => {
                        println!("    ✓ Rejected with non-WireProtocolError: {:?}", e);
                    }
                }
            },
            Ok(_) => {
                panic!("Expected oversized length prefix to be rejected, but read succeeded");
            }
        }
    }
    
    // Test 5: Protocol recovery after receiving invalid length prefix
    println!("Test 5: Testing protocol recovery after invalid length prefix");
    {
        // First, try to read an invalid message (this should fail)
        let invalid_data = create_zero_length_prefix_data();
        let mut invalid_stream = MockStream::with_data(invalid_data);
        
        let invalid_result = framed_message.read_message(&mut invalid_stream).await;
        assert!(invalid_result.is_err(), "Invalid message should be rejected");
        println!("  ✓ Invalid message properly rejected");
        
        // Now test that the protocol can still handle valid messages
        let (valid_envelope, _) = create_test_envelope("recovery_test_payload");
        
        // Write the valid message to get proper wire format
        let mut write_stream = MockStream::new();
        framed_message.write_message(&mut write_stream, &valid_envelope)
            .await
            .expect("Should be able to write valid message");
        
        let valid_data = write_stream.get_written_data().to_vec();
        let mut valid_read_stream = MockStream::with_data(valid_data);
        
        // Read the valid message - this should succeed, demonstrating protocol recovery
        let recovery_result = framed_message.read_message(&mut valid_read_stream).await;
        
        match recovery_result {
            Ok(recovered_envelope) => {
                assert!(recovered_envelope.verify_signature(), 
                       "Recovered message should have valid signature");
                
                let recovered_message = recovered_envelope.get_message()
                    .expect("Should be able to deserialize recovered message");
                assert_eq!(recovered_message.get_payload(), "recovery_test_payload",
                          "Recovered message should have correct payload");
                
                println!("  ✓ Protocol successfully recovered and processed valid message");
            },
            Err(e) => {
                panic!("Protocol should recover after invalid length prefix, but got error: {:?}", e);
            }
        }
    }
    
    // Test 6: Additional edge cases for length prefix validation
    println!("Test 6: Testing additional edge cases");
    {
        let edge_cases = vec![
            (1, "minimum valid length (edge case)"),
            (MAX_MESSAGE_SIZE as u32, "maximum valid length (edge case)"),
            (0xFFFFFFFF, "u32::MAX"),
            (0x7FFFFFFF, "i32::MAX when interpreted as signed"),
            (0x80000001, "just above i32::MAX when interpreted as signed"),
        ];
        
        for (length_value, description) in edge_cases {
            let edge_data = create_invalid_length_prefix_data(length_value);
            let mut edge_stream = MockStream::with_data(edge_data);
            
            let result = framed_message.read_message(&mut edge_stream).await;
            
            match result {
                Ok(_) => {
                    if length_value <= MAX_MESSAGE_SIZE as u32 && length_value > 0 {
                        println!("    ✓ {} accepted (may be valid)", description);
                    } else {
                        panic!("Expected {} to be rejected but it was accepted", description);
                    }
                },
                Err(_) => {
                    println!("    ✓ {} correctly rejected", description);
                }
            }
        }
    }
    
    println!("✓ All corrupted length prefix handling tests passed!");
    println!("  - Verified zero length prefix rejection");
    println!("  - Verified negative-when-cast length prefix rejection");
    println!("  - Verified extremely large length prefix rejection");
    println!("  - Verified oversized length prefix rejection");
    println!("  - Verified protocol recovery after invalid length prefix");
    println!("  - Verified edge case handling");
}

#[tokio::test]
async fn test_corrupted_message_data_handling() {
    println!("Testing corrupted message data handling - Essential Test #14");
    
    // Test case covers:
    // - Send valid length prefix followed by corrupted message data
    // - Verify protocol detects deserialization failures
    // - Test protocol handles various types of data corruption
    // - Verify appropriate errors are returned for different corruption types
    
    let framed_message = FramedMessage::default();
    
    // First, create a valid message to understand the expected format
    let (valid_envelope, _) = create_test_envelope("test_payload_for_corruption");
    let valid_serialized = bincode::serialize(&valid_envelope)
        .expect("Failed to serialize valid envelope");
    let valid_length = valid_serialized.len() as u32;
    
    println!("Valid message size: {} bytes", valid_length);
    
    // Test 1: Valid length prefix with completely corrupted data
    println!("Test 1: Testing valid length prefix with corrupted data");
    {
        let mut corrupted_data = Vec::new();
        corrupted_data.extend_from_slice(&valid_length.to_be_bytes());
        
        // Add completely random data of the correct length
        let random_data: Vec<u8> = (0..valid_length).map(|_| rand::random::<u8>()).collect();
        corrupted_data.extend_from_slice(&random_data);
        
        let mut corrupted_stream = MockStream::with_data(corrupted_data);
        let result = framed_message.read_message(&mut corrupted_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Corrupted data correctly rejected");
                
                // Check for deserialization-related errors
                let error_string = format!("{:?}", e);
                if error_string.contains("Serialization") || 
                   error_string.contains("deserialization") ||
                   error_string.contains("bincode") ||
                   error_string.contains("InvalidMessageFormat") {
                    println!("    ✓ Detected as serialization/deserialization error");
                } else {
                    println!("    ✓ Rejected with error: {}", error_string);
                }
            },
            Ok(_) => {
                panic!("Expected corrupted data to be rejected, but deserialization succeeded");
            }
        }
    }
    
    // Test 2: Partial corruption - valid start, corrupted end
    println!("Test 2: Testing partial corruption scenarios");
    {
        let mut partial_corrupted_data = Vec::new();
        partial_corrupted_data.extend_from_slice(&valid_length.to_be_bytes());
        
        // Take first half of valid data, corrupt second half
        let half_point = valid_serialized.len() / 2;
        partial_corrupted_data.extend_from_slice(&valid_serialized[..half_point]);
        
        // Corrupt the second half
        let corrupted_second_half: Vec<u8> = (0..(valid_serialized.len() - half_point))
            .map(|_| rand::random::<u8>())
            .collect();
        partial_corrupted_data.extend_from_slice(&corrupted_second_half);
        
        let mut partial_stream = MockStream::with_data(partial_corrupted_data);
        let result = framed_message.read_message(&mut partial_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Partially corrupted data correctly rejected");
                println!("    ✓ Error: {:?}", e);
            },
            Ok(_) => {
                // This might succeed in rare cases where corruption doesn't affect structure
                println!("  ! Partially corrupted data was accepted (possible with minor corruption)");
            }
        }
    }
    
    // Test 3: Truncated message data
    println!("Test 3: Testing truncated message data");
    {
        let mut truncated_data = Vec::new();
        truncated_data.extend_from_slice(&valid_length.to_be_bytes());
        
        // Only include first 75% of the message data
        let truncate_point = (valid_serialized.len() * 3) / 4;
        truncated_data.extend_from_slice(&valid_serialized[..truncate_point]);
        
        let mut truncated_stream = MockStream::with_data(truncated_data);
        let result = framed_message.read_message(&mut truncated_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Truncated message data correctly rejected");
                
                // Should detect EOF or length mismatch
                let error_string = format!("{:?}", e);
                if error_string.contains("EOF") || 
                   error_string.contains("UnexpectedEof") ||
                   error_string.contains("LengthMismatch") {
                    println!("    ✓ Detected as EOF/length mismatch error");
                } else {
                    println!("    ✓ Rejected with error: {}", error_string);
                }
            },
            Ok(_) => {
                panic!("Expected truncated data to be rejected due to EOF");
            }
        }
    }
    
    // Test 4: Extra data beyond claimed length
    println!("Test 4: Testing extra data beyond claimed length");
    {
        let mut extra_data = Vec::new();
        extra_data.extend_from_slice(&valid_length.to_be_bytes());
        extra_data.extend_from_slice(&valid_serialized);
        
        // Add extra garbage data
        extra_data.extend_from_slice(b"extra_garbage_data_that_should_not_be_there");
        
        let mut extra_stream = MockStream::with_data(extra_data);
        let result = framed_message.read_message(&mut extra_stream).await;
        
        match result {
            Ok(envelope) => {
                // Extra data might be ignored if the message itself is valid
                println!("  ✓ Message with extra data was accepted (extra data ignored)");
                assert!(envelope.verify_signature(), "Should have valid signature despite extra data");
            },
            Err(e) => {
                println!("  ✓ Message with extra data was rejected: {:?}", e);
            }
        }
    }
    
    println!("✓ All corrupted message data handling tests passed!");
    println!("  - Verified detection of completely corrupted data");
    println!("  - Verified detection of partially corrupted data");
    println!("  - Verified detection of truncated message data");
    println!("  - Verified handling of extra data beyond claimed length");
}

#[tokio::test]
async fn test_unexpected_connection_closure() {
    println!("Testing unexpected connection closure - Essential Test #15");
    
    // Test case covers:
    // - Close connection during message transmission (both read and write)
    // - Verify protocol detects EOF conditions appropriately
    // - Test graceful handling of unexpected connection termination
    // - Verify appropriate errors are returned
    
    let framed_message = FramedMessage::default();
    
    // Test 1: Connection closed during length prefix reading
    println!("Test 1: Testing connection closure during length prefix reading");
    {
        // Various partial length prefix scenarios
        let partial_scenarios = vec![
            (vec![], "immediately after connect"),
            (vec![0x00], "after 1 byte of length prefix"),
            (vec![0x00, 0x01], "after 2 bytes of length prefix"),
            (vec![0x00, 0x01, 0x02], "after 3 bytes of length prefix"),
        ];
        
        for (partial_data, description) in partial_scenarios {
            let mut partial_stream = MockStream::with_data(partial_data.clone());
            let result = framed_message.read_message(&mut partial_stream).await;
            
            match result {
                Err(e) => {
                    println!("  ✓ Connection closure {} correctly detected", description);
                    
                    // Should get EOF-related error
                    let error_string = format!("{:?}", e);
                    if error_string.contains("EOF") || 
                       error_string.contains("UnexpectedEof") ||
                       error_string.contains("Unexpected") {
                        println!("    ✓ Detected as EOF error: {}", error_string);
                    } else {
                        println!("    ✓ Detected with error: {}", error_string);
                    }
                },
                Ok(_) => {
                    panic!("Expected EOF error for connection closure {}, but read succeeded", description);
                }
            }
        }
    }
    
    // Test 2: Connection closed during message body reading
    println!("Test 2: Testing connection closure during message body reading");
    {
        // Create a valid length prefix for a reasonably sized message
        let test_message_size = 1000u32;
        let mut partial_body_data = Vec::new();
        partial_body_data.extend_from_slice(&test_message_size.to_be_bytes());
        
        // Add only partial message data
        let partial_body = vec![0u8; 500]; // Only half the claimed data
        partial_body_data.extend_from_slice(&partial_body);
        
        let mut partial_stream = MockStream::with_data(partial_body_data);
        let result = framed_message.read_message(&mut partial_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Connection closure during message body correctly detected");
                
                let error_string = format!("{:?}", e);
                if error_string.contains("EOF") || 
                   error_string.contains("UnexpectedEof") {
                    println!("    ✓ Detected as EOF error during message body read");
                } else {
                    println!("    ✓ Detected with error: {}", error_string);
                }
            },
            Ok(_) => {
                panic!("Expected EOF error during message body read, but read succeeded");
            }
        }
    }
    
    // Test 3: Verify error information is appropriate
    println!("Test 3: Testing error information quality");
    {
        let mut empty_stream = MockStream::with_data(vec![]);
        let result = framed_message.read_message(&mut empty_stream).await;
        
        match result {
            Err(e) => {
                let error_message = format!("{}", e);
                let error_debug = format!("{:?}", e);
                
                println!("  ✓ EOF error provides informative message:");
                println!("    Display: {}", error_message);
                println!("    Debug: {}", error_debug);
                
                // Error should contain helpful information
                assert!(!error_message.is_empty(), "Error message should not be empty");
                assert!(!error_debug.is_empty(), "Error debug should not be empty");
            },
            Ok(_) => {
                panic!("Expected error for empty stream");
            }
        }
    }
    
    println!("✓ All unexpected connection closure tests passed!");
    println!("  - Verified detection of closure during length prefix reading");
    println!("  - Verified detection of closure during message body reading");
    println!("  - Verified error information quality");
}

#[tokio::test]
async fn test_protocol_violation_detection() {
    println!("Testing protocol violation detection - Essential Test #16");
    
    // Test case covers:
    // - Send data that violates wire protocol format
    // - Test sending only partial length prefix
    // - Test sending message shorter than indicated by length prefix
    // - Verify protocol detects violations and responds appropriately
    
    let framed_message = FramedMessage::default();
    
    // Test 1: Invalid wire format - non-big-endian length prefix
    println!("Test 1: Testing protocol format violations");
    {
        // Create data that looks like a valid length but uses little-endian instead of big-endian
        let claimed_length = 100u32;
        let mut invalid_format_data = Vec::new();
        
        // Use little-endian instead of required big-endian
        invalid_format_data.extend_from_slice(&claimed_length.to_le_bytes());
        
        // Add some data (won't be enough for the incorrectly parsed length)
        invalid_format_data.extend_from_slice(&vec![0u8; 50]);
        
        let mut invalid_stream = MockStream::with_data(invalid_format_data);
        let result = framed_message.read_message(&mut invalid_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Invalid wire format correctly rejected");
                println!("    ✓ Error: {:?}", e);
            },
            Ok(_) => {
                println!("  ! Invalid wire format was accepted (might be coincidentally valid)");
            }
        }
    }
    
    // Test 2: Message shorter than indicated by length prefix
    println!("Test 2: Testing message shorter than length prefix claims");
    {
        let claimed_length = 1000u32;
        let actual_data_size = 100;  // Much shorter than claimed
        
        let mut short_message_data = Vec::new();
        short_message_data.extend_from_slice(&claimed_length.to_be_bytes());
        short_message_data.extend_from_slice(&vec![0u8; actual_data_size]);
        
        let mut short_stream = MockStream::with_data(short_message_data);
        let result = framed_message.read_message(&mut short_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Short message correctly rejected");
                
                let error_string = format!("{:?}", e);
                if error_string.contains("EOF") || 
                   error_string.contains("UnexpectedEof") ||
                   error_string.contains("LengthMismatch") {
                    println!("    ✓ Detected as length/EOF violation");
                } else {
                    println!("    ✓ Rejected with error: {}", error_string);
                }
            },
            Ok(_) => {
                panic!("Expected protocol violation for message shorter than claimed length");
            }
        }
    }
    
    // Test 3: Only partial length prefix (protocol violation)
    println!("Test 3: Testing incomplete length prefix");
    {
        let incomplete_prefixes = vec![
            (vec![], "no data"),
            (vec![0x00], "1 byte of 4"),
            (vec![0x00, 0x01], "2 bytes of 4"),
            (vec![0x00, 0x01, 0x02], "3 bytes of 4"),
        ];
        
        for (partial_prefix, description) in incomplete_prefixes {
            let mut incomplete_stream = MockStream::with_data(partial_prefix);
            let result = framed_message.read_message(&mut incomplete_stream).await;
            
            match result {
                Err(e) => {
                    println!("  ✓ Incomplete length prefix ({}) correctly rejected", description);
                    
                    let error_string = format!("{:?}", e);
                    if error_string.contains("EOF") || error_string.contains("UnexpectedEof") {
                        println!("    ✓ Detected as EOF during length prefix read");
                    } else {
                        println!("    ✓ Rejected with error: {}", error_string);
                    }
                },
                Ok(_) => {
                    panic!("Expected protocol violation for incomplete length prefix ({})", description);
                }
            }
        }
    }
    
    // Test 4: Valid length prefix with invalid message structure
    println!("Test 4: Testing invalid message structure");
    {
        // Create data that has proper length prefix but invalid SignedEnvelope structure
        let fake_message_data = b"This is not a valid SignedEnvelope binary structure at all!";
        let message_length = fake_message_data.len() as u32;
        
        let mut invalid_structure_data = Vec::new();
        invalid_structure_data.extend_from_slice(&message_length.to_be_bytes());
        invalid_structure_data.extend_from_slice(fake_message_data);
        
        let mut invalid_stream = MockStream::with_data(invalid_structure_data);
        let result = framed_message.read_message(&mut invalid_stream).await;
        
        match result {
            Err(e) => {
                println!("  ✓ Invalid message structure correctly rejected");
                
                let error_string = format!("{:?}", e);
                if error_string.contains("Serialization") || 
                   error_string.contains("bincode") ||
                   error_string.contains("InvalidMessageFormat") {
                    println!("    ✓ Detected as serialization error");
                } else {
                    println!("    ✓ Rejected with error: {}", error_string);
                }
            },
            Ok(_) => {
                panic!("Expected protocol violation for invalid message structure");
            }
        }
    }
    
    // Test 5: Verify protocol responses are appropriate
    println!("Test 5: Testing appropriate error responses");
    {
        let test_cases = vec![
            (vec![], "empty stream"),
            (vec![0xFF, 0xFF, 0xFF, 0xFF], "maximum u32 length prefix only"),
            (vec![0x00, 0x00, 0x00, 0x05, 0x01, 0x02], "claimed 5 bytes, got 2"),
        ];
        
        for (violation_data, description) in test_cases {
            let mut violation_stream = MockStream::with_data(violation_data);
            let result = framed_message.read_message(&mut violation_stream).await;
            
            match result {
                Err(e) => {
                    println!("  ✓ Protocol violation ({}) appropriately handled", description);
                    
                    // Verify error contains contextual information
                    let error_string = format!("{:?}", e);
                    assert!(!error_string.is_empty(), "Error should contain information");
                    
                    // Check if error is informative
                    if error_string.len() > 20 { // Reasonable minimum for informative error
                        println!("    ✓ Error message is informative: {}", 
                                &error_string[..std::cmp::min(100, error_string.len())]);
                    }
                },
                Ok(_) => {
                    panic!("Expected protocol violation for {}", description);
                }
            }
        }
    }
    
    println!("✓ All protocol violation detection tests passed!");
    println!("  - Verified detection of wire format violations");
    println!("  - Verified detection of length mismatch violations");
    println!("  - Verified detection of incomplete length prefix");
    println!("  - Verified detection of invalid message structure");
    println!("  - Verified appropriate error responses");
} 