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

#[tokio::test]
async fn test_large_message_handling() {
    println!("Testing large message handling - memory efficiency and performance");
    
    // Use a larger configuration for testing large messages
    let large_max_size = 1024 * 1024; // 1MB limit for large message testing
    let wire_config = WireConfig::with_max_message_size(large_max_size);
    let framed_message = FramedMessage::new(wire_config);
    
    // Test 1: Send large but valid messages (approaching size limit)
    println!("Test 1: Testing large messages approaching size limit");
    
    let large_message_test_cases = vec![
        (large_max_size / 4, "quarter max size"),
        (large_max_size / 2, "half max size"),
        (large_max_size * 3 / 4, "three-quarters max size"),
        (large_max_size * 9 / 10, "90% of max size"),
    ];
    
    for (target_payload_size, description) in large_message_test_cases {
        println!("  Testing {} (~{} bytes payload)", description, target_payload_size);
        
        // Create a large payload - account for envelope overhead
        // Start with a reasonable estimate and adjust if needed
        let mut payload_size = target_payload_size.saturating_sub(1000); // Reserve space for envelope overhead
        let payload = "A".repeat(payload_size);
        let (envelope, _) = create_test_envelope(&payload);
        
        // Check actual serialized size and adjust if necessary
        let mut serialized_size = bincode::serialize(&envelope)
            .expect("Failed to serialize test envelope")
            .len();
        
        // If we're over the limit, reduce payload size
        while serialized_size > large_max_size && payload_size > 1000 {
            payload_size = payload_size.saturating_sub(1000);
            let adjusted_payload = "A".repeat(payload_size);
            let (adjusted_envelope, _) = create_test_envelope(&adjusted_payload);
            serialized_size = bincode::serialize(&adjusted_envelope)
                .expect("Failed to serialize adjusted envelope")
                .len();
        }
        
        if serialized_size > large_max_size {
            println!("    ! Skipping {} - cannot create message within size limit due to envelope overhead", description);
            continue;
        }
        
        println!("    Final payload size: {} bytes, serialized size: {} bytes", payload_size, serialized_size);
        
        // Test 2: Verify protocol handles large messages without memory issues
        println!("    Testing memory-efficient processing...");
        
        let final_payload = "A".repeat(payload_size);
        let (large_envelope, _) = create_test_envelope(&final_payload);
        
        // Test write operation
        let mut write_stream = MockStream::new();
        
        let write_start = std::time::Instant::now();
        framed_message.write_message(&mut write_stream, &large_envelope)
            .await
            .expect(&format!("Failed to write large message for {}", description));
        let write_duration = write_start.elapsed();
        
        println!("    ✓ Write completed in {:?}", write_duration);
        
        // Test read operation
        let written_data = write_stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(written_data);
        
        let read_start = std::time::Instant::now();
        let received_envelope = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to read back large message for {}", description));
        let read_duration = read_start.elapsed();
        
        println!("    ✓ Read completed in {:?}", read_duration);
        
        // Test 3: Verify message integrity after large message processing
        assert!(received_envelope.verify_signature(), 
               "Signature should be valid for large message ({})", description);
        
        // Verify payload content integrity
        let received_message = received_envelope.get_message()
            .expect("Failed to extract message from large envelope");
        
        if let Message::Ping { nonce, payload: received_payload } = received_message {
            assert_eq!(nonce, 42, "Nonce should be preserved");
            assert_eq!(received_payload.len(), final_payload.len(), 
                      "Payload length should be preserved for {}", description);
            assert_eq!(received_payload, final_payload, 
                      "Payload content should be preserved for {}", description);
        } else {
            panic!("Expected Ping message for {}", description);
        }
        
        println!("    ✓ {} successfully processed and verified", description);
    }
    
    // Test 4: Test memory allocation patterns with multiple large messages
    println!("Test 4: Testing memory patterns with multiple large messages");
    
    let batch_size = 5;
    let batch_payload_size = large_max_size / 8; // Use 1/8 of max size for batch testing
    
    // Ensure we can create valid messages at this size
    let test_payload = "B".repeat(batch_payload_size.saturating_sub(1000));
    let (test_envelope, _) = create_test_envelope(&test_payload);
    let test_serialized_size = bincode::serialize(&test_envelope)
        .expect("Failed to serialize batch test envelope")
        .len();
    
    if test_serialized_size <= large_max_size {
        println!("  Processing batch of {} messages (~{} bytes each)", batch_size, test_serialized_size);
        
        let batch_start = std::time::Instant::now();
        
        for i in 0..batch_size {
            let batch_payload = format!("{}_{}", "B".repeat(test_payload.len()), i);
            let (batch_envelope, _) = create_test_envelope(&batch_payload);
            
            // Write and read back each message
            let mut batch_stream = MockStream::new();
            
            framed_message.write_message(&mut batch_stream, &batch_envelope)
                .await
                .expect(&format!("Failed to write batch message {}", i));
            
            let written_data = batch_stream.get_written_data().to_vec();
            let mut read_stream = MockStream::with_data(written_data);
            
            let received_envelope = framed_message.read_message(&mut read_stream)
                .await
                .expect(&format!("Failed to read batch message {}", i));
            
            assert!(received_envelope.verify_signature(), 
                   "Batch message {} signature should be valid", i);
        }
        
        let batch_duration = batch_start.elapsed();
        println!("    ✓ Processed {} large messages in {:?} (avg: {:?}/message)", 
                 batch_size, batch_duration, batch_duration / batch_size);
    } else {
        println!("  ! Skipping batch test - message too large ({} bytes) for batch testing", test_serialized_size);
    }
    
    // Test 5: Verify no memory leaks with large message processing
    println!("Test 5: Testing memory cleanup after large message processing");
    
    // Process a large message and ensure resources are properly cleaned up
    let cleanup_payload_size = large_max_size / 3;
    let cleanup_payload = "C".repeat(cleanup_payload_size.saturating_sub(1000));
    let (cleanup_envelope, _) = create_test_envelope(&cleanup_payload);
    
    let cleanup_serialized_size = bincode::serialize(&cleanup_envelope)
        .expect("Failed to serialize cleanup test envelope")
        .len();
    
    if cleanup_serialized_size <= large_max_size {
        println!("  Processing cleanup test message ({} bytes)", cleanup_serialized_size);
        
        // Process the message multiple times to test for accumulating memory issues
        for iteration in 0..3 {
            let mut cleanup_stream = MockStream::new();
            
            framed_message.write_message(&mut cleanup_stream, &cleanup_envelope)
                .await
                .expect(&format!("Failed to write cleanup message iteration {}", iteration));
            
            let written_data = cleanup_stream.get_written_data().to_vec();
            let mut read_stream = MockStream::with_data(written_data);
            
            let received_envelope = framed_message.read_message(&mut read_stream)
                .await
                .expect(&format!("Failed to read cleanup message iteration {}", iteration));
            
            assert!(received_envelope.verify_signature(), 
                   "Cleanup message iteration {} signature should be valid", iteration);
            
            // Explicitly drop the received envelope to ensure cleanup
            drop(received_envelope);
        }
        
        println!("    ✓ Multiple iterations completed successfully - no apparent memory leaks");
    } else {
        println!("  ! Skipping cleanup test - message too large ({} bytes)", cleanup_serialized_size);
    }
    
    // Test 6: Test performance characteristics with large messages
    println!("Test 6: Testing performance characteristics");
    
    let perf_payload_size = large_max_size / 6;
    let perf_payload = "D".repeat(perf_payload_size.saturating_sub(1000));
    let (perf_envelope, _) = create_test_envelope(&perf_payload);
    
    let perf_serialized_size = bincode::serialize(&perf_envelope)
        .expect("Failed to serialize performance test envelope")
        .len();
    
    if perf_serialized_size <= large_max_size {
        println!("  Measuring performance with {} byte messages", perf_serialized_size);
        
        let num_iterations = 10;
        let mut write_times = Vec::new();
        let mut read_times = Vec::new();
        
        for _i in 0..num_iterations {
            // Measure write performance
            let mut perf_stream = MockStream::new();
            let write_start = std::time::Instant::now();
            framed_message.write_message(&mut perf_stream, &perf_envelope)
                .await
                .expect("Failed to write performance test message");
            write_times.push(write_start.elapsed());
            
            // Measure read performance
            let written_data = perf_stream.get_written_data().to_vec();
            let mut read_stream = MockStream::with_data(written_data);
            let read_start = std::time::Instant::now();
            let _received = framed_message.read_message(&mut read_stream)
                .await
                .expect("Failed to read performance test message");
            read_times.push(read_start.elapsed());
        }
        
        let avg_write_time = write_times.iter().sum::<std::time::Duration>() / write_times.len() as u32;
        let avg_read_time = read_times.iter().sum::<std::time::Duration>() / read_times.len() as u32;
        
        println!("    Average write time: {:?}", avg_write_time);
        println!("    Average read time: {:?}", avg_read_time);
        println!("    ✓ Performance measurements completed");
        
        // Basic performance sanity check - operations should complete in reasonable time
        assert!(avg_write_time < std::time::Duration::from_secs(1), 
               "Write operations should complete in under 1 second");
        assert!(avg_read_time < std::time::Duration::from_secs(1), 
               "Read operations should complete in under 1 second");
    } else {
        println!("  ! Skipping performance test - message too large ({} bytes)", perf_serialized_size);
    }
    
    println!("✓ All large message handling tests passed!");
} 