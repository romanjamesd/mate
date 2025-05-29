use mate::messages::wire::{FramedMessage, WireConfig, WireProtocolError};
use tokio::io::{AsyncRead, AsyncWrite};
use std::time::Duration;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Test helper: A mock stream that never provides any data to simulate a timeout condition
/// This stream will never return any data, causing read operations to hang until timeout
struct NeverReadStream {
    _data: Vec<u8>, // Placeholder data that we never actually provide
}

impl NeverReadStream {
    fn new() -> Self {
        Self {
            _data: vec![0u8; 1024], // Some data that we'll never actually provide
        }
    }
}

impl AsyncRead for NeverReadStream {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Always return Pending to simulate a stream that never has data available
        // This will cause read operations to hang until timeout
        Poll::Pending
    }
}

impl AsyncWrite for NeverReadStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        // Accept writes normally for consistency
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

/// Test read timeout enforcement
///
/// From essential-tests.md:
/// 17. **`test_read_timeout_enforcement()`**
///    - Configure read timeout and ensure no data is sent
///    - Verify read operation times out within expected timeframe
///    - Test that timeout errors are properly reported
///    - Verify connection state after timeout
#[tokio::test]
async fn test_read_timeout_enforcement() {
    // Configure a short read timeout for testing
    let read_timeout = Duration::from_millis(100); // 100ms timeout
    let write_timeout = Duration::from_secs(30); // Normal write timeout
    
    let wire_config = WireConfig::new(1024 * 1024, read_timeout, write_timeout);
    let framed_message = FramedMessage::new(wire_config);
    
    // Create a stream that never provides data
    let mut never_read_stream = NeverReadStream::new();
    
    // Record the start time to verify timeout timing
    let start_time = std::time::Instant::now();
    
    // Attempt to read a message - this should timeout
    let result = framed_message.read_message_with_timeout(
        &mut never_read_stream,
        read_timeout
    ).await;
    
    let elapsed = start_time.elapsed();
    
    // Verify the operation timed out
    assert!(result.is_err(), "Read operation should have timed out");
    
    // Verify the error is a timeout error
    let error = result.unwrap_err();
    let wire_error = error.downcast_ref::<WireProtocolError>();
    assert!(wire_error.is_some(), "Error should be a WireProtocolError");
    
    // Check that it's specifically a timeout-related error
    let wire_error = wire_error.unwrap();
    let is_timeout_error = matches!(wire_error, WireProtocolError::Timeout(_)) ||
                          error.to_string().contains("timeout") || 
                          error.to_string().contains("timed out");
    assert!(is_timeout_error, "Error should be timeout-related, got: {}", error);
    
    // Verify timing: should have timed out approximately at the configured timeout
    // Allow some tolerance for timing variations (±50ms)
    let expected_timeout = read_timeout;
    let min_expected = expected_timeout.saturating_sub(Duration::from_millis(50));
    let max_expected = expected_timeout + Duration::from_millis(50);
    
    assert!(
        elapsed >= min_expected,
        "Timeout occurred too early: {:?} < expected minimum {:?}",
        elapsed, min_expected
    );
    assert!(
        elapsed <= max_expected,
        "Timeout occurred too late: {:?} > expected maximum {:?}",
        elapsed, max_expected
    );
    
    // Verify that the timeout error contains timeout-related information
    let error_string = error.to_string();
    assert!(
        error_string.contains("timeout") || 
        error_string.contains("timed out") ||
        error_string.contains("deadline has elapsed") ||
        error_string.contains("Timeout error"),
        "Error message should indicate a timeout occurred: {}",
        error_string
    );
    
    // Test with default timeout method as well
    let start_time = std::time::Instant::now();
    let result = framed_message.read_message_with_default_timeout(&mut never_read_stream).await;
    let elapsed = start_time.elapsed();
    
    // Should also timeout, but with the configured read timeout
    assert!(result.is_err(), "Default timeout read should also timeout");
    
    // Verify timing matches configured timeout
    let expected_timeout = read_timeout; // Same as configured
    let min_expected = expected_timeout.saturating_sub(Duration::from_millis(50));
    let max_expected = expected_timeout + Duration::from_millis(50);
    
    assert!(
        elapsed >= min_expected && elapsed <= max_expected,
        "Default timeout should match configured read timeout: {:?} not in range [{:?}, {:?}]",
        elapsed, min_expected, max_expected
    );
    
    println!("✅ Read timeout enforcement test passed");
    println!("   - Custom timeout: {:?}", read_timeout);
    println!("   - Actual timeout timing: {:?}", elapsed);
    println!("   - Error: {}", result.unwrap_err());
}

/// Test helper: A mock stream that provides a specific amount of data then stops
/// This simulates partial reads where some data is available but then the stream
/// becomes unavailable, triggering a timeout during the read operation
struct PartialDataStream {
    data: Vec<u8>,       // The data to provide
    position: usize,     // Current read position
    max_bytes: usize,    // Maximum bytes to provide before stopping
}

impl PartialDataStream {
    /// Create a new PartialDataStream that will provide `max_bytes` of data from `data`
    fn new(data: Vec<u8>, max_bytes: usize) -> Self {
        Self {
            data,
            position: 0,
            max_bytes,
        }
    }
}

impl AsyncRead for PartialDataStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Check if we've already provided the maximum allowed bytes
        if self.position >= self.max_bytes {
            // Stop providing data - return Pending to simulate timeout condition
            return Poll::Pending;
        }
        
        // Calculate how many bytes we can still provide
        let remaining_allowed = self.max_bytes - self.position;
        let available_data = self.data.len() - self.position;
        let bytes_to_provide = std::cmp::min(
            std::cmp::min(remaining_allowed, available_data),
            buf.remaining()
        );
        
        if bytes_to_provide > 0 {
            // Provide the data
            let end_pos = self.position + bytes_to_provide;
            buf.put_slice(&self.data[self.position..end_pos]);
            self.position = end_pos;
            Poll::Ready(Ok(()))
        } else {
            // No more data to provide or reached max_bytes limit
            Poll::Pending
        }
    }
}

impl AsyncWrite for PartialDataStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        // Accept writes normally for consistency
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

/// Helper function to create a complete message with known length prefix
async fn create_test_message_data() -> Vec<u8> {
    use mate::crypto::Identity;
    use mate::messages::{Message, SignedEnvelope};
    use std::io::Cursor;
    
    // Create a test message
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(42, "test_payload_for_timeout_test".to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    
    // Serialize the message using FramedMessage to get the wire format
    let framed_message = FramedMessage::default();
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);
    
    framed_message.write_message(&mut cursor, &envelope)
        .await
        .expect("Failed to write test message");
    
    buffer
}

/// Test read timeout during partial operation
///
/// From essential-tests.md:
/// 18. **`test_read_timeout_during_partial_operation()`**
///    - Start receiving message, then stop sending data mid-transmission
///    - Verify timeout occurs during partial read
///    - Test timeout during length prefix read vs. message body read
#[tokio::test]
async fn test_read_timeout_during_partial_operation() {
    // Configure a short read timeout for testing
    let read_timeout = Duration::from_millis(100); // 100ms timeout
    let write_timeout = Duration::from_secs(30); // Normal write timeout
    
    let wire_config = WireConfig::new(1024 * 1024, read_timeout, write_timeout);
    let framed_message = FramedMessage::new(wire_config);
    
    // Create test message data to understand the structure
    let complete_message_data = create_test_message_data().await;
    let message_body_size = complete_message_data.len() - 4; // Subtract 4-byte length prefix
    
    println!("Test message structure:");
    println!("  - Total size: {} bytes", complete_message_data.len());
    println!("  - Length prefix: 4 bytes");
    println!("  - Message body: {} bytes", message_body_size);
    
    // Test 1: Timeout during length prefix read
    println!("\nTest 1: Testing timeout during length prefix read");
    
    let partial_prefix_test_cases = vec![
        (0, "no data at all"),
        (1, "1 byte of length prefix"),
        (2, "2 bytes of length prefix"),
        (3, "3 bytes of length prefix"),
    ];
    
    for (partial_bytes, description) in partial_prefix_test_cases {
        println!("  Testing with {}", description);
        
        let mut partial_stream = PartialDataStream::new(
            complete_message_data.clone(),
            partial_bytes
        );
        
        let start_time = std::time::Instant::now();
        
        let result = framed_message.read_message_with_timeout(
            &mut partial_stream,
            read_timeout
        ).await;
        
        let elapsed = start_time.elapsed();
        
        // Verify the operation timed out
        assert!(result.is_err(), "Read operation should have timed out with {}", description);
        
        // Verify timing: should have timed out approximately at the configured timeout
        let min_expected = read_timeout.saturating_sub(Duration::from_millis(50));
        let max_expected = read_timeout + Duration::from_millis(50);
        
        assert!(
            elapsed >= min_expected && elapsed <= max_expected,
            "Timeout timing with {} should be within expected range: {:?} not in [{:?}, {:?}]",
            description, elapsed, min_expected, max_expected
        );
        
        // Verify the error indicates a timeout
        let error = result.unwrap_err();
        let error_string = error.to_string();
        let is_timeout_error = error_string.contains("timeout") || 
                              error_string.contains("timed out") ||
                              error_string.contains("deadline has elapsed") ||
                              error_string.contains("Timeout error");
        assert!(is_timeout_error, 
               "Error should indicate timeout with {}: {}", description, error_string);
        
        println!("    ✓ {} - timed out in {:?}", description, elapsed);
    }
    
    // Test 2: Timeout during message body read
    println!("\nTest 2: Testing timeout during message body read");
    
    let partial_body_test_cases = vec![
        (4, "length prefix only, no message body"),
        (4 + message_body_size / 4, "length prefix + 25% of message body"),
        (4 + message_body_size / 2, "length prefix + 50% of message body"),
        (4 + (message_body_size * 3) / 4, "length prefix + 75% of message body"),
        (complete_message_data.len() - 1, "all but last byte of message"),
    ];
    
    for (partial_bytes, description) in partial_body_test_cases {
        println!("  Testing with {}", description);
        
        let mut partial_stream = PartialDataStream::new(
            complete_message_data.clone(),
            partial_bytes
        );
        
        let start_time = std::time::Instant::now();
        
        let result = framed_message.read_message_with_timeout(
            &mut partial_stream,
            read_timeout
        ).await;
        
        let elapsed = start_time.elapsed();
        
        // Verify the operation timed out
        assert!(result.is_err(), "Read operation should have timed out with {}", description);
        
        // Verify timing: should have timed out approximately at the configured timeout
        let min_expected = read_timeout.saturating_sub(Duration::from_millis(50));
        let max_expected = read_timeout + Duration::from_millis(50);
        
        assert!(
            elapsed >= min_expected && elapsed <= max_expected,
            "Timeout timing with {} should be within expected range: {:?} not in [{:?}, {:?}]",
            description, elapsed, min_expected, max_expected
        );
        
        // Verify the error indicates a timeout
        let error = result.unwrap_err();
        let error_string = error.to_string();
        let is_timeout_error = error_string.contains("timeout") || 
                              error_string.contains("timed out") ||
                              error_string.contains("deadline has elapsed") ||
                              error_string.contains("Timeout error");
        assert!(is_timeout_error, 
               "Error should indicate timeout with {}: {}", description, error_string);
        
        println!("    ✓ {} - timed out in {:?}", description, elapsed);
    }
    
    // Test 3: Verify that complete message would not timeout (control case)
    println!("\nTest 3: Control test - complete message should not timeout");
    
    let mut complete_stream = PartialDataStream::new(
        complete_message_data.clone(),
        complete_message_data.len() // Allow all data to be read
    );
    
    let start_time = std::time::Instant::now();
    
    let result = framed_message.read_message_with_timeout(
        &mut complete_stream,
        read_timeout
    ).await;
    
    let elapsed = start_time.elapsed();
    
    // This should succeed (not timeout)
    assert!(result.is_ok(), "Complete message should not timeout, but got error: {:?}", 
           result.err());
    
    // Should complete much faster than the timeout
    assert!(elapsed < read_timeout.saturating_sub(Duration::from_millis(20)),
           "Complete message should read quickly, but took {:?}", elapsed);
    
    // Verify the message is valid
    let received_envelope = result.unwrap();
    assert!(received_envelope.verify_signature(), 
           "Received message should have valid signature");
    
    println!("    ✓ Complete message read successfully in {:?}", elapsed);
    
    println!("✅ Read timeout during partial operation test passed");
    println!("   - Tested timeout during length prefix read (0, 1, 2, 3 bytes)");
    println!("   - Tested timeout during message body read (25%, 50%, 75%, 99%)");
    println!("   - Verified complete messages don't timeout");
    println!("   - All timeouts occurred within expected timeframe ({:?})", read_timeout);
}

/// Test helper: A mock stream that never accepts any writes to simulate a blocked receiver
/// This stream will never accept any data for writing, causing write operations to hang until timeout
struct NeverWriteStream {
    _buffer: Vec<u8>, // Placeholder buffer that we never actually use
}

impl NeverWriteStream {
    fn new() -> Self {
        Self {
            _buffer: Vec::new(),
        }
    }
}

impl AsyncRead for NeverWriteStream {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Allow reads to succeed for consistency (just return empty data)
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for NeverWriteStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        // Always return Pending to simulate a stream that never accepts writes
        // This will cause write operations to hang until timeout
        Poll::Pending
    }

    fn poll_flush(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        // Also block flush operations
        Poll::Pending
    }

    fn poll_shutdown(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        // Allow shutdown to succeed for cleanup
        Poll::Ready(Ok(()))
    }
}

/// Test write timeout enforcement
///
/// From essential-tests.md:
/// 19. **`test_write_timeout_enforcement()`**
///    - Configure write timeout with blocked receiver
///    - Verify write operation times out within expected timeframe
///    - Test that timeout errors are properly reported
///    - Verify connection state after timeout
#[tokio::test]
async fn test_write_timeout_enforcement() {
    use mate::crypto::Identity;
    use mate::messages::{Message, SignedEnvelope};
    
    // Configure a short write timeout for testing
    let read_timeout = Duration::from_secs(30); // Normal read timeout
    let write_timeout = Duration::from_millis(100); // 100ms write timeout
    
    let wire_config = WireConfig::new(1024 * 1024, read_timeout, write_timeout);
    let framed_message = FramedMessage::new(wire_config);
    
    // Create a test message to write
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(42, "test_payload_for_write_timeout_test".to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    
    // Create a stream that never accepts writes
    let mut never_write_stream = NeverWriteStream::new();
    
    // Record the start time to verify timeout timing
    let start_time = std::time::Instant::now();
    
    // Attempt to write a message - this should timeout
    let result = framed_message.write_message_with_timeout(
        &mut never_write_stream,
        &envelope,
        write_timeout
    ).await;
    
    let elapsed = start_time.elapsed();
    
    // Verify the operation timed out
    assert!(result.is_err(), "Write operation should have timed out");
    
    // Verify the error is a timeout error
    let error = result.unwrap_err();
    let wire_error = error.downcast_ref::<WireProtocolError>();
    assert!(wire_error.is_some(), "Error should be a WireProtocolError");
    
    // Check that it's specifically a timeout-related error
    let wire_error = wire_error.unwrap();
    let is_timeout_error = matches!(wire_error, WireProtocolError::Timeout(_)) ||
                          error.to_string().contains("timeout") || 
                          error.to_string().contains("timed out");
    assert!(is_timeout_error, "Error should be timeout-related, got: {}", error);
    
    // Verify timing: should have timed out approximately at the configured timeout
    // Allow some tolerance for timing variations (±50ms)
    let expected_timeout = write_timeout;
    let min_expected = expected_timeout.saturating_sub(Duration::from_millis(50));
    let max_expected = expected_timeout + Duration::from_millis(50);
    
    assert!(
        elapsed >= min_expected,
        "Timeout occurred too early: {:?} < expected minimum {:?}",
        elapsed, min_expected
    );
    assert!(
        elapsed <= max_expected,
        "Timeout occurred too late: {:?} > expected maximum {:?}",
        elapsed, max_expected
    );
    
    // Verify that the timeout error contains timeout-related information
    let error_string = error.to_string();
    assert!(
        error_string.contains("timeout") || 
        error_string.contains("timed out") ||
        error_string.contains("deadline has elapsed") ||
        error_string.contains("Timeout error"),
        "Error message should indicate a timeout occurred: {}",
        error_string
    );
    
    // Test with default timeout method as well
    let start_time = std::time::Instant::now();
    let result = framed_message.write_message_with_default_timeout(&mut never_write_stream, &envelope).await;
    let elapsed = start_time.elapsed();
    
    // Should also timeout, but with the configured write timeout
    assert!(result.is_err(), "Default timeout write should also timeout");
    
    // Verify timing matches configured timeout
    let expected_timeout = write_timeout; // Same as configured
    let min_expected = expected_timeout.saturating_sub(Duration::from_millis(50));
    let max_expected = expected_timeout + Duration::from_millis(50);
    
    assert!(
        elapsed >= min_expected && elapsed <= max_expected,
        "Default timeout should match configured write timeout: {:?} not in range [{:?}, {:?}]",
        elapsed, min_expected, max_expected
    );
    
    // Verify connection state after timeout - the stream should still be in a consistent state
    // We can't easily test this without more complex mocking, but we can verify the stream
    // hasn't been corrupted by attempting another operation (which should also timeout cleanly)
    let start_time = std::time::Instant::now();
    let result = framed_message.write_message_with_timeout(
        &mut never_write_stream,
        &envelope,
        write_timeout
    ).await;
    let elapsed = start_time.elapsed();
    
    // This should also timeout, indicating the stream is still in a usable state
    assert!(result.is_err(), "Second write operation should also timeout");
    assert!(
        elapsed >= min_expected && elapsed <= max_expected,
        "Second timeout should also be within expected range: {:?}",
        elapsed
    );
    
    println!("✅ Write timeout enforcement test passed");
    println!("   - Write timeout: {:?}", write_timeout);
    println!("   - Actual timeout timing: {:?}", elapsed);
    println!("   - Error: {}", result.unwrap_err());
    println!("   - Connection state remains consistent after timeout");
}

/// Test configurable timeout values
///
/// From essential-tests.md:
/// 20. **`test_configurable_timeout_values()`**
///    - Test wire protocol respects configured timeout values
///    - Verify different timeout values work correctly
///    - Test very short and very long timeout configurations
#[tokio::test]
async fn test_configurable_timeout_values() {
    use mate::crypto::Identity;
    use mate::messages::{Message, SignedEnvelope};
    
    // Create a test message for write operations
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(42, "test_payload_for_configurable_timeout_test".to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    
    // Test cases with different timeout values
    let timeout_test_cases = vec![
        (Duration::from_millis(50), "very short timeout (50ms)"),
        (Duration::from_millis(100), "short timeout (100ms)"),
        (Duration::from_millis(200), "medium timeout (200ms)"),
        (Duration::from_millis(500), "long timeout (500ms)"),
        (Duration::from_millis(1000), "very long timeout (1000ms)"),
    ];
    
    println!("Testing configurable timeout values for READ operations");
    
    for (timeout_duration, description) in &timeout_test_cases {
        println!("  Testing read with {}", description);
        
        // Configure wire config with the specific read timeout
        let wire_config = WireConfig::new(1024 * 1024, *timeout_duration, Duration::from_secs(30));
        let framed_message = FramedMessage::new(wire_config);
        
        // Create a stream that never provides data
        let mut never_read_stream = NeverReadStream::new();
        
        let start_time = std::time::Instant::now();
        
        // Test read with explicit timeout
        let result = framed_message.read_message_with_timeout(
            &mut never_read_stream,
            *timeout_duration
        ).await;
        
        let elapsed = start_time.elapsed();
        
        // Verify the operation timed out
        assert!(result.is_err(), "Read operation should have timed out with {}", description);
        
        // Verify timing: should have timed out approximately at the configured timeout
        // Use more generous tolerance for very short timeouts due to system scheduling
        let tolerance = if timeout_duration.as_millis() < 100 { 
            Duration::from_millis(50) 
        } else { 
            Duration::from_millis(100) 
        };
        
        let min_expected = timeout_duration.saturating_sub(tolerance);
        let max_expected = *timeout_duration + tolerance;
        
        assert!(
            elapsed >= min_expected,
            "Read timeout with {} occurred too early: {:?} < expected minimum {:?}",
            description, elapsed, min_expected
        );
        assert!(
            elapsed <= max_expected,
            "Read timeout with {} occurred too late: {:?} > expected maximum {:?}",
            description, elapsed, max_expected
        );
        
        println!("    ✓ {} - timed out in {:?} (expected: {:?})", description, elapsed, timeout_duration);
        
        // Test read with default timeout (should use configured value)
        let start_time = std::time::Instant::now();
        let result = framed_message.read_message_with_default_timeout(&mut never_read_stream).await;
        let elapsed = start_time.elapsed();
        
        // Should also timeout with the configured read timeout
        assert!(result.is_err(), "Default read timeout should also timeout with {}", description);
        
        assert!(
            elapsed >= min_expected && elapsed <= max_expected,
            "Default read timeout with {} should match configured value: {:?} not in range [{:?}, {:?}]",
            description, elapsed, min_expected, max_expected
        );
        
        println!("    ✓ {} - default timeout in {:?}", description, elapsed);
    }
    
    println!("\nTesting configurable timeout values for WRITE operations");
    
    for (timeout_duration, description) in &timeout_test_cases {
        println!("  Testing write with {}", description);
        
        // Configure wire config with the specific write timeout
        let wire_config = WireConfig::new(1024 * 1024, Duration::from_secs(30), *timeout_duration);
        let framed_message = FramedMessage::new(wire_config);
        
        // Create a stream that never accepts writes
        let mut never_write_stream = NeverWriteStream::new();
        
        let start_time = std::time::Instant::now();
        
        // Test write with explicit timeout
        let result = framed_message.write_message_with_timeout(
            &mut never_write_stream,
            &envelope,
            *timeout_duration
        ).await;
        
        let elapsed = start_time.elapsed();
        
        // Verify the operation timed out
        assert!(result.is_err(), "Write operation should have timed out with {}", description);
        
        // Verify timing: should have timed out approximately at the configured timeout
        let tolerance = if timeout_duration.as_millis() < 100 { 
            Duration::from_millis(50) 
        } else { 
            Duration::from_millis(100) 
        };
        
        let min_expected = timeout_duration.saturating_sub(tolerance);
        let max_expected = *timeout_duration + tolerance;
        
        assert!(
            elapsed >= min_expected,
            "Write timeout with {} occurred too early: {:?} < expected minimum {:?}",
            description, elapsed, min_expected
        );
        assert!(
            elapsed <= max_expected,
            "Write timeout with {} occurred too late: {:?} > expected maximum {:?}",
            description, elapsed, max_expected
        );
        
        println!("    ✓ {} - timed out in {:?} (expected: {:?})", description, elapsed, timeout_duration);
        
        // Test write with default timeout (should use configured value)
        let start_time = std::time::Instant::now();
        let result = framed_message.write_message_with_default_timeout(&mut never_write_stream, &envelope).await;
        let elapsed = start_time.elapsed();
        
        // Should also timeout with the configured write timeout
        assert!(result.is_err(), "Default write timeout should also timeout with {}", description);
        
        assert!(
            elapsed >= min_expected && elapsed <= max_expected,
            "Default write timeout with {} should match configured value: {:?} not in range [{:?}, {:?}]",
            description, elapsed, min_expected, max_expected
        );
        
        println!("    ✓ {} - default timeout in {:?}", description, elapsed);
    }
    
    println!("\nTesting mixed timeout configurations");
    
    // Test with mixed read/write timeout configurations
    let mixed_test_cases = vec![
        (Duration::from_millis(50), Duration::from_millis(200), "short read, long write"),
        (Duration::from_millis(200), Duration::from_millis(50), "long read, short write"),
        (Duration::from_millis(100), Duration::from_millis(100), "equal read/write"),
    ];
    
    for (read_timeout, write_timeout, description) in mixed_test_cases {
        println!("  Testing mixed config: {}", description);
        
        let wire_config = WireConfig::new(1024 * 1024, read_timeout, write_timeout);
        let framed_message = FramedMessage::new(wire_config);
        
        // Test read uses read timeout
        let mut never_read_stream = NeverReadStream::new();
        let start_time = std::time::Instant::now();
        let result = framed_message.read_message_with_default_timeout(&mut never_read_stream).await;
        let elapsed = start_time.elapsed();
        
        assert!(result.is_err(), "Read operation should timeout with {}", description);
        
        let read_tolerance = if read_timeout.as_millis() < 100 { 
            Duration::from_millis(50) 
        } else { 
            Duration::from_millis(100) 
        };
        
        let read_min = read_timeout.saturating_sub(read_tolerance);
        let read_max = read_timeout + read_tolerance;
        
        assert!(
            elapsed >= read_min && elapsed <= read_max,
            "Read operation with {} should use read timeout {:?}, got {:?}",
            description, read_timeout, elapsed
        );
        
        // Test write uses write timeout
        let mut never_write_stream = NeverWriteStream::new();
        let start_time = std::time::Instant::now();
        let result = framed_message.write_message_with_default_timeout(&mut never_write_stream, &envelope).await;
        let elapsed = start_time.elapsed();
        
        assert!(result.is_err(), "Write operation should timeout with {}", description);
        
        let write_tolerance = if write_timeout.as_millis() < 100 { 
            Duration::from_millis(50) 
        } else { 
            Duration::from_millis(100) 
        };
        
        let write_min = write_timeout.saturating_sub(write_tolerance);
        let write_max = write_timeout + write_tolerance;
        
        assert!(
            elapsed >= write_min && elapsed <= write_max,
            "Write operation with {} should use write timeout {:?}, got {:?}",
            description, write_timeout, elapsed
        );
        
        println!("    ✓ {} - read: {:?}, write: {:?}", description, read_timeout, write_timeout);
    }
    
    println!("✅ Configurable timeout values test passed");
    println!("   - Tested very short timeouts (50ms)");
    println!("   - Tested medium timeouts (100ms, 200ms)");
    println!("   - Tested long timeouts (500ms, 1000ms)");
    println!("   - Verified both explicit and default timeout methods");
    println!("   - Tested mixed read/write timeout configurations");
    println!("   - All timeouts respected configured values within expected tolerance");
} 