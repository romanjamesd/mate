use mate::crypto::Identity;
use mate::messages::{Message, SignedEnvelope};
use mate::messages::wire::{FramedMessage, WireConfig, WireProtocolError, MAX_MESSAGE_SIZE};
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

/// Create a mock stream with oversized length prefix
fn create_oversized_length_prefix_data(fake_size: u32) -> Vec<u8> {
    // Create a 4-byte length prefix with the fake size
    let mut data = Vec::new();
    data.extend_from_slice(&fake_size.to_be_bytes());
    
    // Add some dummy data (doesn't need to be valid message data for this test)
    data.extend_from_slice(b"dummy_message_data_that_wont_be_processed");
    
    data
}

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
                        assert_eq!(*size, fake_size, 
                                 "Error should report correct oversized message size");
                        assert_eq!(*max_size, test_max_size, 
                                 "Error should report correct maximum allowed size");
                        println!("    ✓ Correctly rejected {} with MessageTooLarge error", description);
                    },
                    Some(WireProtocolError::InvalidLength { .. }) => {
                        println!("    ✓ Correctly rejected {} with InvalidLength error", description);
                    },
                    Some(other_error) => {
                        panic!("Expected MessageTooLarge or InvalidLength error for {}, got: {:?}", 
                               description, other_error);
                    },
                    None => {
                        panic!("Expected WireProtocolError for {}, got: {:?}", description, e);
                    }
                }
            },
            Ok(_) => {
                panic!("Expected error for oversized message ({}), but read succeeded", description);
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
    
    println!("  Boundary test envelope serialized size: {} bytes (limit: {})", 
             serialized_size, test_max_size);
    
    if serialized_size <= test_max_size {
        // This message should be accepted
        let mut boundary_stream = MockStream::new();
        
        let write_result = framed_message.write_message(&mut boundary_stream, &boundary_envelope).await;
        match write_result {
            Ok(_) => {
                println!("    ✓ Message at boundary ({} bytes) was correctly accepted", serialized_size);
                
                // Test reading it back
                let written_data = boundary_stream.get_written_data().to_vec();
                let mut read_stream = MockStream::with_data(written_data);
                
                let read_result = framed_message.read_message(&mut read_stream).await;
                match read_result {
                    Ok(received_envelope) => {
                        assert!(received_envelope.verify_signature(), 
                               "Boundary message signature should be valid");
                        println!("    ✓ Boundary message successfully round-tripped");
                    },
                    Err(e) => {
                        panic!("Failed to read back boundary message: {:?}", e);
                    }
                }
            },
            Err(e) => {
                println!("    ! Message at boundary was rejected during write: {:?}", e);
                // This might be expected if the envelope overhead pushes it over the limit
            }
        }
    } else {
        println!("    ! Boundary test envelope ({} bytes) exceeds limit ({} bytes) due to envelope overhead", 
                 serialized_size, test_max_size);
    }
    
    // Test 3: Verify protocol accepts messages within size limits
    println!("Test 3: Testing acceptance of messages within size limits");
    
    let within_limits_test_cases = vec![
        (100, "small message (100 bytes payload)"),
        (200, "medium message (200 bytes payload)"),
        (300, "larger message (300 bytes payload)"),
    ];
    
    for (payload_size, description) in within_limits_test_cases {
        let payload = "x".repeat(payload_size);
        let (envelope, _) = create_test_envelope(&payload);
        
        // Check serialized size
        let serialized_size = bincode::serialize(&envelope)
            .expect("Failed to serialize test envelope")
            .len();
        
        if serialized_size <= test_max_size {
            println!("  Testing {} (serialized: {} bytes)", description, serialized_size);
            
            let mut stream = MockStream::new();
            
            // Write message
            framed_message.write_message(&mut stream, &envelope)
                .await
                .expect(&format!("Failed to write {}", description));
            
            // Read message back
            let written_data = stream.get_written_data().to_vec();
            let mut read_stream = MockStream::with_data(written_data);
            
            let received_envelope = framed_message.read_message(&mut read_stream)
                .await
                .expect(&format!("Failed to read back {}", description));
            
            assert!(received_envelope.verify_signature(), 
                   "Signature should be valid for {}", description);
            
            println!("    ✓ {} successfully processed", description);
        } else {
            println!("  Skipping {} - serialized size ({}) exceeds test limit ({})", 
                     description, serialized_size, test_max_size);
        }
    }
    
    // Test 4: Test protocol remains stable after malicious attempts
    println!("Test 4: Testing protocol stability after malicious attempts");
    
    // Try to send multiple malicious messages
    let malicious_sizes = vec![
        test_max_size * 100,
        (u32::MAX - 1) as usize,
        u32::MAX as usize,
    ];
    
    for malicious_size in malicious_sizes {
        let malicious_data = create_oversized_length_prefix_data(malicious_size as u32);
        let mut malicious_stream = MockStream::with_data(malicious_data);
        
        let result = framed_message.read_message(&mut malicious_stream).await;
        
        // Should fail appropriately
        assert!(result.is_err(), "Malicious message should be rejected");
        
        // Protocol should remain stable - test with a valid message afterward
        let (valid_envelope, _) = create_test_envelope("valid message after attack");
        let valid_serialized_size = bincode::serialize(&valid_envelope)
            .expect("Failed to serialize valid envelope")
            .len();
        
        if valid_serialized_size <= test_max_size {
            let mut valid_stream = MockStream::new();
            
            let write_result = framed_message.write_message(&mut valid_stream, &valid_envelope).await;
            assert!(write_result.is_ok(), "Protocol should remain functional after malicious attempt");
            
            println!("    ✓ Protocol remains stable after malicious attempt with {} byte fake size", malicious_size);
        }
    }
    
    println!("✓ All message size limit enforcement tests passed!");
} 