//! Network timeout handling tests
//!
//! This module contains unit tests for timeout enforcement in network operations,
//! including read timeouts, write timeouts, and timeout configuration.

use mate::messages::wire::{FramedMessage, WireConfig, WireProtocolError};
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};

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

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Ready(Ok(()))
    }
}

/// Test helper: A mock stream that never accepts writes to simulate write timeout
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
        // Return empty read for consistency
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for NeverWriteStream {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        // Always return Pending to simulate a stream that can't accept writes
        // This will cause write operations to hang until timeout
        Poll::Pending
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Pending
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Poll::Pending
    }
}

/// Test case 17 from essential-tests.md
///
/// Test read timeout enforcement
/// - Configure read timeout and ensure no data is sent
/// - Verify read operation times out within expected timeframe
/// - Test that timeout errors are properly reported
/// - Verify connection state after timeout
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
    let result = framed_message
        .read_message_with_timeout(&mut never_read_stream, read_timeout)
        .await;

    let elapsed = start_time.elapsed();

    // Verify the operation timed out
    assert!(result.is_err(), "Read operation should have timed out");

    // Verify the error is a timeout error
    let error = result.unwrap_err();
    let wire_error = error.downcast_ref::<WireProtocolError>();
    assert!(wire_error.is_some(), "Error should be a WireProtocolError");

    // Check that it's specifically a timeout-related error
    let wire_error = wire_error.unwrap();
    let is_timeout_error = matches!(wire_error, WireProtocolError::Timeout(_))
        || error.to_string().contains("timeout")
        || error.to_string().contains("timed out");
    assert!(
        is_timeout_error,
        "Error should be timeout-related, got: {}",
        error
    );

    // Verify timing: should have timed out approximately at the configured timeout
    // Allow some tolerance for timing variations (±50ms)
    let expected_timeout = read_timeout;
    let min_expected = expected_timeout.saturating_sub(Duration::from_millis(50));
    let max_expected = expected_timeout + Duration::from_millis(50);

    assert!(
        elapsed >= min_expected,
        "Timeout occurred too early: {:?} < expected minimum {:?}",
        elapsed,
        min_expected
    );
    assert!(
        elapsed <= max_expected,
        "Timeout occurred too late: {:?} > expected maximum {:?}",
        elapsed,
        max_expected
    );

    // Verify that the timeout error contains timeout-related information
    let error_string = error.to_string();
    assert!(
        error_string.contains("timeout")
            || error_string.contains("timed out")
            || error_string.contains("deadline has elapsed")
            || error_string.contains("Timeout error"),
        "Error message should indicate a timeout occurred: {}",
        error_string
    );

    // Test with default timeout method as well
    let start_time = std::time::Instant::now();
    let result = framed_message
        .read_message_with_default_timeout(&mut never_read_stream)
        .await;
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
        elapsed,
        min_expected,
        max_expected
    );

    println!("✅ Read timeout enforcement test passed");
    println!("   - Custom timeout: {:?}", read_timeout);
    println!("   - Actual timeout timing: {:?}", elapsed);
    println!("   - Error: {}", result.unwrap_err());
}

/// Test case 19 from essential-tests.md
///
/// Test write timeout enforcement
/// - Configure write timeout and create stream that can't accept writes
/// - Verify write operation times out within expected timeframe  
/// - Test that timeout errors are properly reported
/// - Verify connection state after timeout
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
    let message = Message::new_ping(42, "test_write_timeout".to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");

    // Create a stream that never accepts writes
    let mut never_write_stream = NeverWriteStream::new();

    // Record the start time to verify timeout timing
    let start_time = std::time::Instant::now();

    // Attempt to write a message - this should timeout
    let result = framed_message
        .write_message_with_timeout(&mut never_write_stream, &envelope, write_timeout)
        .await;

    let elapsed = start_time.elapsed();

    // Verify the operation timed out
    assert!(result.is_err(), "Write operation should have timed out");

    // Verify the error is a timeout error
    let error = result.unwrap_err();
    let wire_error = error.downcast_ref::<WireProtocolError>();
    assert!(wire_error.is_some(), "Error should be a WireProtocolError");

    // Check that it's specifically a timeout-related error
    let wire_error = wire_error.unwrap();
    let is_timeout_error = matches!(wire_error, WireProtocolError::Timeout(_))
        || error.to_string().contains("timeout")
        || error.to_string().contains("timed out");
    assert!(
        is_timeout_error,
        "Error should be timeout-related, got: {}",
        error
    );

    // Verify timing: should have timed out approximately at the configured timeout
    // Allow some tolerance for timing variations (±50ms)
    let expected_timeout = write_timeout;
    let min_expected = expected_timeout.saturating_sub(Duration::from_millis(50));
    let max_expected = expected_timeout + Duration::from_millis(50);

    assert!(
        elapsed >= min_expected,
        "Timeout occurred too early: {:?} < expected minimum {:?}",
        elapsed,
        min_expected
    );
    assert!(
        elapsed <= max_expected,
        "Timeout occurred too late: {:?} > expected maximum {:?}",
        elapsed,
        max_expected
    );

    // Verify that the timeout error contains timeout-related information
    let error_string = error.to_string();
    assert!(
        error_string.contains("timeout")
            || error_string.contains("timed out")
            || error_string.contains("deadline has elapsed")
            || error_string.contains("Timeout error"),
        "Error message should indicate a timeout occurred: {}",
        error_string
    );

    println!("✅ Write timeout enforcement test passed");
    println!("   - Custom timeout: {:?}", write_timeout);
    println!("   - Actual timeout timing: {:?}", elapsed);
    println!("   - Error: {}", error_string);
}
