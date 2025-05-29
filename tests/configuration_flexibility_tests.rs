use mate::crypto::Identity;
use mate::messages::{Message, SignedEnvelope};
use mate::messages::wire::{FramedMessage, WireConfig, MAX_MESSAGE_SIZE, DEFAULT_READ_TIMEOUT, DEFAULT_WRITE_TIMEOUT};
use tokio::io::{AsyncRead, AsyncWrite};
use std::io::Cursor;
use std::time::Duration;

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

/// Test wire protocol with different maximum message size configurations
/// Verify configuration changes are properly enforced
/// Test protocol behavior with very restrictive and very permissive limits
#[tokio::test]
async fn test_configurable_size_limits() {
    println!("Testing configurable size limits - Configuration and Flexibility");
    
    // Test 1: Test wire protocol with different maximum message size configurations
    println!("Test 1: Testing various maximum message size configurations");
    
    let size_config_test_cases = vec![
        (512, "very restrictive (512 bytes)"),
        (1024, "restrictive (1KB)"),
        (64 * 1024, "moderate (64KB)"),
        (512 * 1024, "permissive (512KB)"),
        (1024 * 1024, "very permissive (1MB)"),
        (8 * 1024 * 1024, "extremely permissive (8MB)"),
    ];
    
    for (max_size, description) in size_config_test_cases {
        println!("  Testing configuration: {}", description);
        
        // Create wire config with specific max message size
        let wire_config = WireConfig::with_max_message_size(max_size);
        let framed_message = FramedMessage::new(wire_config);
        
        // Test messages at different size relationships to the limit
        let payload_test_cases = vec![
            (50, "small payload"),
            (max_size / 4, "quarter limit"),
            (max_size / 2, "half limit"),
            ((max_size * 3) / 4, "three-quarter limit"),
        ];
        
        for (target_payload_size, payload_description) in payload_test_cases {
            if target_payload_size > 0 {
                println!("    Testing {} with {}", payload_description, description);
                
                // Create a payload that should result in a message close to target size
                // Account for envelope overhead by using slightly smaller payload
                let actual_payload_size = target_payload_size.saturating_sub(500);
                let test_payload = "X".repeat(actual_payload_size);
                let (test_envelope, _) = create_test_envelope(&test_payload);
                
                // Check actual serialized size
                let serialized_size = bincode::serialize(&test_envelope)
                    .expect("Failed to serialize test envelope")
                    .len();
                
                println!("      Payload size: {}, Serialized size: {}, Limit: {}", 
                         actual_payload_size, serialized_size, max_size);
                
                if serialized_size <= max_size {
                    // Message should be accepted
                    let mut stream = MockStream::new();
                    
                    let write_result = framed_message.write_message(&mut stream, &test_envelope).await;
                    match write_result {
                        Ok(_) => {
                            println!("      ✓ Message within limit correctly accepted");
                            
                            // Test reading it back
                            let written_data = stream.get_written_data().to_vec();
                            let mut read_stream = MockStream::with_data(written_data);
                            
                            let read_result = framed_message.read_message(&mut read_stream).await;
                            match read_result {
                                Ok(received_envelope) => {
                                    assert!(received_envelope.verify_signature(), 
                                           "Received message signature should be valid");
                                    println!("      ✓ Message successfully round-tripped");
                                },
                                Err(e) => {
                                    panic!("Failed to read back message within limits: {:?}", e);
                                }
                            }
                        },
                        Err(e) => {
                            panic!("Message within limits should not be rejected: {:?}", e);
                        }
                    }
                } else {
                    println!("      ! Skipping - serialized size ({}) exceeds limit ({})", 
                             serialized_size, max_size);
                }
            }
        }
        
        // Test oversized message rejection
        println!("    Testing oversized message rejection with {}", description);
        
        // Create a message that definitely exceeds the limit
        let oversized_payload_size = max_size + 1000;
        let oversized_payload = "Y".repeat(oversized_payload_size);
        let (oversized_envelope, _) = create_test_envelope(&oversized_payload);
        
        let oversized_serialized_size = bincode::serialize(&oversized_envelope)
            .expect("Failed to serialize oversized envelope")
            .len();
        
        if oversized_serialized_size > max_size {
            let mut oversized_stream = MockStream::new();
            
            let oversized_result = framed_message.write_message(&mut oversized_stream, &oversized_envelope).await;
            match oversized_result {
                Err(_) => {
                    println!("      ✓ Oversized message correctly rejected (size: {}, limit: {})", 
                             oversized_serialized_size, max_size);
                },
                Ok(_) => {
                    panic!("Oversized message should have been rejected (size: {}, limit: {})", 
                           oversized_serialized_size, max_size);
                }
            }
        }
    }
    
    // Test 2: Verify configuration changes are properly enforced
    println!("\nTest 2: Testing dynamic configuration enforcement");
    
    // Create messages of known sizes
    let small_payload = "small message".to_string();
    let medium_payload = "medium sized message payload that should be larger".repeat(20);
    let large_payload = "large message payload for testing".repeat(100);
    
    let (small_envelope, _) = create_test_envelope(&small_payload);
    let (medium_envelope, _) = create_test_envelope(&medium_payload);
    let (large_envelope, _) = create_test_envelope(&large_payload);
    
    let small_size = bincode::serialize(&small_envelope).unwrap().len();
    let medium_size = bincode::serialize(&medium_envelope).unwrap().len();
    let large_size = bincode::serialize(&large_envelope).unwrap().len();
    
    println!("  Message sizes - Small: {}, Medium: {}, Large: {}", small_size, medium_size, large_size);
    
    // Test with progressive size limits
    let progressive_limits = vec![
        (small_size + 100, "allow small only"),
        (medium_size + 100, "allow small and medium"),
        (large_size + 100, "allow all sizes"),
    ];
    
    for (limit, description) in progressive_limits {
        println!("  Testing limit: {} ({})", limit, description);
        
        let wire_config = WireConfig::with_max_message_size(limit);
        let framed_message = FramedMessage::new(wire_config);
        
        // Test small message (should always work)
        let mut small_stream = MockStream::new();
        let small_result = framed_message.write_message(&mut small_stream, &small_envelope).await;
        assert!(small_result.is_ok(), "Small message should always be accepted");
        println!("    ✓ Small message accepted");
        
        // Test medium message
        let mut medium_stream = MockStream::new();
        let medium_result = framed_message.write_message(&mut medium_stream, &medium_envelope).await;
        if medium_size <= limit {
            assert!(medium_result.is_ok(), "Medium message should be accepted when within limit");
            println!("    ✓ Medium message accepted (within limit)");
        } else {
            assert!(medium_result.is_err(), "Medium message should be rejected when over limit");
            println!("    ✓ Medium message rejected (over limit)");
        }
        
        // Test large message
        let mut large_stream = MockStream::new();
        let large_result = framed_message.write_message(&mut large_stream, &large_envelope).await;
        if large_size <= limit {
            assert!(large_result.is_ok(), "Large message should be accepted when within limit");
            println!("    ✓ Large message accepted (within limit)");
        } else {
            assert!(large_result.is_err(), "Large message should be rejected when over limit");
            println!("    ✓ Large message rejected (over limit)");
        }
    }
    
    // Test 3: Test protocol behavior with very restrictive and very permissive limits
    println!("\nTest 3: Testing extreme configuration scenarios");
    
    // Very restrictive configuration
    println!("  Testing very restrictive configuration (256 bytes)");
    let restrictive_config = WireConfig::with_max_message_size(256);
    let restrictive_framed = FramedMessage::new(restrictive_config);
    
    // Create a minimal message that should fit
    let minimal_payload = "min".to_string();
    let (minimal_envelope, _) = create_test_envelope(&minimal_payload);
    let minimal_size = bincode::serialize(&minimal_envelope).unwrap().len();
    
    println!("    Minimal message size: {} bytes", minimal_size);
    
    if minimal_size <= 256 {
        let mut restrictive_stream = MockStream::new();
        let restrictive_result = restrictive_framed.write_message(&mut restrictive_stream, &minimal_envelope).await;
        assert!(restrictive_result.is_ok(), "Minimal message should work with restrictive config");
        println!("    ✓ Minimal message works with restrictive configuration");
        
        // Verify round-trip
        let written_data = restrictive_stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(written_data);
        let read_result = restrictive_framed.read_message(&mut read_stream).await;
        assert!(read_result.is_ok(), "Should be able to read back minimal message");
        println!("    ✓ Minimal message round-trip successful");
    } else {
        println!("    ! Even minimal message ({} bytes) exceeds restrictive limit (256 bytes)", minimal_size);
    }
    
    // Very permissive configuration  
    println!("  Testing very permissive configuration ({})", MAX_MESSAGE_SIZE);
    let permissive_config = WireConfig::with_max_message_size(MAX_MESSAGE_SIZE);
    let permissive_framed = FramedMessage::new(permissive_config);
    
    // Create a reasonably large message
    let large_test_payload = "Large test payload for permissive configuration ".repeat(1000);
    let (large_test_envelope, _) = create_test_envelope(&large_test_payload);
    let large_test_size = bincode::serialize(&large_test_envelope).unwrap().len();
    
    println!("    Large test message size: {} bytes", large_test_size);
    
    let mut permissive_stream = MockStream::new();
    let permissive_result = permissive_framed.write_message(&mut permissive_stream, &large_test_envelope).await;
    assert!(permissive_result.is_ok(), "Large message should work with permissive config");
    println!("    ✓ Large message works with permissive configuration");
    
    // Verify round-trip
    let written_data = permissive_stream.get_written_data().to_vec();
    let mut read_stream = MockStream::with_data(written_data);
    let read_result = permissive_framed.read_message(&mut read_stream).await;
    assert!(read_result.is_ok(), "Should be able to read back large message");
    
    let received = read_result.unwrap();
    assert!(received.verify_signature(), "Large message signature should be valid");
    println!("    ✓ Large message round-trip successful with signature verification");
    
    // Test 4: Verify configuration consistency
    println!("\nTest 4: Testing configuration consistency and isolation");
    
    // Create multiple FramedMessage instances with different configurations
    let config_a = WireConfig::with_max_message_size(1024);
    let config_b = WireConfig::with_max_message_size(2048);
    let config_c = WireConfig::with_max_message_size(4096);
    
    let framed_a = FramedMessage::new(config_a);
    let framed_b = FramedMessage::new(config_b);
    let framed_c = FramedMessage::new(config_c);
    
    // Create a message that fits config_b and config_c but not config_a
    let test_payload = "test payload for configuration isolation ".repeat(50);
    let (test_envelope, _) = create_test_envelope(&test_payload);
    let test_size = bincode::serialize(&test_envelope).unwrap().len();
    
    println!("  Test message size: {} bytes", test_size);
    println!("  Config A limit: 1024, Config B limit: 2048, Config C limit: 4096");
    
    // Test with config A (should reject if message is too large)
    let mut stream_a = MockStream::new();
    let result_a = framed_a.write_message(&mut stream_a, &test_envelope).await;
    
    // Test with config B
    let mut stream_b = MockStream::new();
    let result_b = framed_b.write_message(&mut stream_b, &test_envelope).await;
    
    // Test with config C
    let mut stream_c = MockStream::new();
    let result_c = framed_c.write_message(&mut stream_c, &test_envelope).await;
    
    // Verify behavior matches expectations based on message size
    if test_size <= 1024 {
        assert!(result_a.is_ok(), "Message should be accepted by config A");
        assert!(result_b.is_ok(), "Message should be accepted by config B");
        assert!(result_c.is_ok(), "Message should be accepted by config C");
        println!("    ✓ All configurations accepted the small message");
    } else if test_size <= 2048 {
        assert!(result_a.is_err(), "Message should be rejected by config A");
        assert!(result_b.is_ok(), "Message should be accepted by config B");
        assert!(result_c.is_ok(), "Message should be accepted by config C");
        println!("    ✓ Configuration isolation working: A rejected, B and C accepted");
    } else if test_size <= 4096 {
        assert!(result_a.is_err(), "Message should be rejected by config A");
        assert!(result_b.is_err(), "Message should be rejected by config B");
        assert!(result_c.is_ok(), "Message should be accepted by config C");
        println!("    ✓ Configuration isolation working: A and B rejected, C accepted");
    } else {
        assert!(result_a.is_err(), "Message should be rejected by config A");
        assert!(result_b.is_err(), "Message should be rejected by config B");
        assert!(result_c.is_err(), "Message should be rejected by config C");
        println!("    ✓ All configurations correctly rejected the oversized message");
    }
    
    println!("\n✓ All configurable size limits tests passed!");
}

#[tokio::test]
async fn test_default_configuration_sanity() {
    println!("Testing default configuration sanity - verifying production readiness");
    
    // Test 1: Verify default configuration values are reasonable for production use
    println!("Test 1: Verifying default configuration values");
    
    let default_config = WireConfig::default();
    
    // Check default max message size
    assert_eq!(default_config.max_message_size, MAX_MESSAGE_SIZE,
               "Default max message size should match global constant");
    assert!(default_config.max_message_size >= 1024 * 1024,
           "Default max message size should be at least 1MB for practical use");
    assert!(default_config.max_message_size <= 64 * 1024 * 1024,
           "Default max message size should not exceed 64MB to prevent memory issues");
    
    println!("  ✓ Default max message size: {} bytes ({:.1} MB)", 
             default_config.max_message_size,
             default_config.max_message_size as f64 / (1024.0 * 1024.0));
    
    // Check default timeouts
    assert_eq!(default_config.read_timeout, DEFAULT_READ_TIMEOUT,
               "Default read timeout should match global constant");
    assert_eq!(default_config.write_timeout, DEFAULT_WRITE_TIMEOUT,
               "Default write timeout should match global constant");
    
    assert!(default_config.read_timeout >= Duration::from_secs(5),
           "Default read timeout should be at least 5 seconds for network operations");
    assert!(default_config.read_timeout <= Duration::from_secs(300),
           "Default read timeout should not exceed 5 minutes to prevent resource leaks");
    
    assert!(default_config.write_timeout >= Duration::from_secs(5),
           "Default write timeout should be at least 5 seconds for network operations");
    assert!(default_config.write_timeout <= Duration::from_secs(300),
           "Default write timeout should not exceed 5 minutes to prevent resource leaks");
    
    println!("  ✓ Default read timeout: {:?}", default_config.read_timeout);
    println!("  ✓ Default write timeout: {:?}", default_config.write_timeout);
    
    // Test 2: Test that default timeouts are appropriate for network conditions
    println!("\nTest 2: Testing default timeout appropriateness");
    
    let default_framed = FramedMessage::default();
    
    // Create a test message that should work with defaults
    let test_payload = "Default configuration test message".to_string();
    let (test_envelope, _) = create_test_envelope(&test_payload);
    
    // Test normal operation succeeds within default timeouts
    let mut stream = MockStream::new();
    let write_start = std::time::Instant::now();
    
    let write_result = default_framed.write_message(&mut stream, &test_envelope).await;
    let write_duration = write_start.elapsed();
    
    assert!(write_result.is_ok(), "Default configuration should handle normal messages");
    assert!(write_duration < default_config.write_timeout,
           "Normal write should complete well within default timeout");
    
    println!("  ✓ Normal write completed in {:?} (timeout: {:?})", 
             write_duration, default_config.write_timeout);
    
    // Test read operation
    let written_data = stream.get_written_data().to_vec();
    let mut read_stream = MockStream::with_data(written_data);
    let read_start = std::time::Instant::now();
    
    let read_result = default_framed.read_message(&mut read_stream).await;
    let read_duration = read_start.elapsed();
    
    assert!(read_result.is_ok(), "Default configuration should handle normal reads");
    assert!(read_duration < default_config.read_timeout,
           "Normal read should complete well within default timeout");
    
    println!("  ✓ Normal read completed in {:?} (timeout: {:?})", 
             read_duration, default_config.read_timeout);
    
    // Test 3: Test that default size limits provide adequate DoS protection
    println!("\nTest 3: Testing default DoS protection adequacy");
    
    // Verify defaults provide protection against common DoS vectors
    
    // Memory exhaustion protection
    let max_reasonable_allocation = default_config.max_message_size;
    assert!(max_reasonable_allocation <= 64 * 1024 * 1024,
           "Default size limit should prevent excessive memory allocation");
    
    println!("  ✓ Memory exhaustion protection: max allocation {} bytes", max_reasonable_allocation);
    
    // Bandwidth exhaustion protection - ensure messages can't be arbitrarily large
    assert!(default_config.max_message_size >= 64 * 1024,
           "Default should allow reasonable message sizes (at least 64KB)");
    assert!(default_config.max_message_size <= 32 * 1024 * 1024,
           "Default should prevent excessively large messages (max 32MB)");
    
    println!("  ✓ Bandwidth protection: reasonable size limits enforced");
    
    // Timeout-based DoS protection
    assert!(default_config.read_timeout.as_secs() <= 60,
           "Read timeout should prevent indefinite resource holding");
    assert!(default_config.write_timeout.as_secs() <= 60,
           "Write timeout should prevent indefinite resource holding");
    
    println!("  ✓ Timeout-based DoS protection: reasonable timeout limits");
    
    // Test that defaults can handle realistic workloads
    println!("  Testing realistic workload handling...");
    
    let realistic_test_cases = vec![
        ("small control message", 100),
        ("typical data message", 1024),
        ("large but reasonable message", 64 * 1024),
        ("bulk data message", 256 * 1024),
    ];
    
    for (description, payload_size) in realistic_test_cases {
        let payload = "X".repeat(payload_size);
        let (envelope, _) = create_test_envelope(&payload);
        let serialized_size = bincode::serialize(&envelope).unwrap().len();
        
        if serialized_size <= default_config.max_message_size {
            let mut test_stream = MockStream::new();
            let result = default_framed.write_message(&mut test_stream, &envelope).await;
            
            assert!(result.is_ok(), "Default config should handle {}", description);
            println!("    ✓ {} ({} bytes) handled successfully", description, serialized_size);
        } else {
            println!("    ! {} ({} bytes) exceeds default limit", description, serialized_size);
        }
    }
    
    // Test 4: Verify production-specific configurations work as expected
    println!("\nTest 4: Testing production-specific configurations");
    
    let production_framed = FramedMessage::for_production();
    let client_framed = FramedMessage::for_client();
    let server_framed = FramedMessage::for_server();
    
    // Test that specialized configs can handle their intended use cases
    let test_message = "Production test message for validation".to_string();
    let (test_envelope, _) = create_test_envelope(&test_message);
    
    // Production config test
    let mut prod_stream = MockStream::new();
    let prod_result = production_framed.write_message(&mut prod_stream, &test_envelope).await;
    assert!(prod_result.is_ok(), "Production config should handle normal messages");
    println!("  ✓ Production configuration working");
    
    // Client config test
    let mut client_stream = MockStream::new();
    let client_result = client_framed.write_message(&mut client_stream, &test_envelope).await;
    assert!(client_result.is_ok(), "Client config should handle normal messages");
    println!("  ✓ Client configuration working");
    
    // Server config test
    let mut server_stream = MockStream::new();
    let server_result = server_framed.write_message(&mut server_stream, &test_envelope).await;
    assert!(server_result.is_ok(), "Server config should handle normal messages");
    println!("  ✓ Server configuration working");
    
    println!("\n✓ All default configuration sanity tests passed!");
} 