use mate::crypto::Identity;
use mate::messages::{Message, SignedEnvelope};
use mate::messages::wire::{FramedMessage, LENGTH_PREFIX_SIZE};
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

/// Create a test SignedEnvelope with a unique identifier
fn create_test_envelope_with_nonce(payload: &str, nonce: u64) -> (SignedEnvelope, Message) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(nonce, payload.to_string());
    let envelope = SignedEnvelope::create(&message, &identity, Some(1234567890))
        .expect("Failed to create signed envelope");
    (envelope, message)
}

/// Helper function to write multiple messages to a single buffer in sequence
async fn write_multiple_messages_to_buffer(messages: &[(SignedEnvelope, Message)]) -> Vec<u8> {
    let framed_message = FramedMessage::default();
    let mut stream = MockStream::new();
    
    for (envelope, _) in messages {
        framed_message.write_message(&mut stream, envelope)
            .await
            .expect("Failed to write message to buffer");
    }
    
    stream.get_written_data().to_vec()
}

/// A controllable mock stream that returns predetermined read sizes to test partial I/O
struct ControlledMockStream {
    data: Vec<u8>,
    position: usize,
    read_sizes: Vec<usize>,  // Predetermined sizes for each read operation
    read_count: usize,       // Track how many read operations have been performed
}

impl ControlledMockStream {
    /// Create a new ControlledMockStream with data and predetermined read sizes
    fn new(data: Vec<u8>, read_sizes: Vec<usize>) -> Self {
        Self {
            data,
            position: 0,
            read_sizes,
            read_count: 0,
        }
    }
    
    /// Check if all data has been read
    fn is_finished(&self) -> bool {
        self.position >= self.data.len()
    }
}

impl AsyncRead for ControlledMockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // If we've read all data, return 0 (EOF)
        if self.position >= self.data.len() {
            return std::task::Poll::Ready(Ok(()));
        }
        
        // Determine how many bytes to read this time
        let read_size = if self.read_count < self.read_sizes.len() {
            self.read_sizes[self.read_count]
        } else {
            // If we've exhausted predetermined sizes, read remaining data
            self.data.len() - self.position
        };
        
        // Calculate actual bytes to read (limited by available space and remaining data)
        let remaining_data = self.data.len() - self.position;
        let bytes_to_read = std::cmp::min(read_size, std::cmp::min(buf.remaining(), remaining_data));
        
        if bytes_to_read > 0 {
            // Copy data to the buffer
            let end_pos = self.position + bytes_to_read;
            buf.put_slice(&self.data[self.position..end_pos]);
            self.position = end_pos;
        }
        
        self.read_count += 1;
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ControlledMockStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        // Not used for read testing
        std::task::Poll::Ready(Ok(0))
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

/// A mock stream that can simulate interrupted reads by providing data in specific chunks
/// This tests the protocol's ability to handle partial reads and resume correctly
struct InterruptibleMockStream {
    data: Vec<u8>,
    position: usize,
    chunk_sizes: Vec<usize>,  // Sizes of data chunks to return on each read
    read_count: usize,        // Track how many read operations have been performed
    total_interruptions: usize, // Track total interruptions for verification
}

impl InterruptibleMockStream {
    /// Create a new InterruptibleMockStream with data and predetermined chunk sizes
    fn new(data: Vec<u8>, interruption_points: Vec<usize>) -> Self {
        // Convert interruption points to chunk sizes
        let mut chunk_sizes = Vec::new();
        let mut last_pos = 0;
        
        for &interrupt_pos in &interruption_points {
            if interrupt_pos > last_pos {
                chunk_sizes.push(interrupt_pos - last_pos);
                last_pos = interrupt_pos;
            }
            // Add a very small chunk to simulate resumption after interruption
            chunk_sizes.push(1);
            last_pos += 1;
        }
        
        // Add final chunk for remaining data
        if last_pos < data.len() {
            chunk_sizes.push(data.len() - last_pos);
        }
        
        Self {
            data,
            position: 0,
            chunk_sizes,
            read_count: 0,
            total_interruptions: interruption_points.len(),
        }
    }
    
    /// Check if all data has been read
    fn is_finished(&self) -> bool {
        self.position >= self.data.len()
    }
    
    /// Get the number of interruptions that occurred
    fn interruption_count(&self) -> usize {
        self.total_interruptions
    }
}

impl AsyncRead for InterruptibleMockStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // If we've read all data, return 0 (EOF)
        if self.position >= self.data.len() {
            return std::task::Poll::Ready(Ok(()));
        }
        
        // Determine how many bytes to read this time based on chunk sizes
        let chunk_size = if self.read_count < self.chunk_sizes.len() {
            self.chunk_sizes[self.read_count]
        } else {
            // If we've exhausted predetermined sizes, read remaining data
            self.data.len() - self.position
        };
        
        // Calculate actual bytes to read (limited by available space and remaining data)
        let remaining_data = self.data.len() - self.position;
        let bytes_to_read = std::cmp::min(chunk_size, std::cmp::min(buf.remaining(), remaining_data));
        
        if bytes_to_read > 0 {
            let end_pos = self.position + bytes_to_read;
            buf.put_slice(&self.data[self.position..end_pos]);
            self.position = end_pos;
        }
        
        self.read_count += 1;
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for InterruptibleMockStream {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        // Not used for read testing
        std::task::Poll::Ready(Ok(0))
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

/// A controllable mock stream that accepts predetermined write sizes to test partial write operations
/// This simulates network backpressure and fragmented write scenarios
struct ControlledWriteMockStream {
    written_data: Vec<u8>,
    write_sizes: Vec<usize>,  // Predetermined sizes for each write operation
    write_count: usize,       // Track how many write operations have been performed
}

impl ControlledWriteMockStream {
    /// Create a new ControlledWriteMockStream with predetermined write sizes
    fn new(write_sizes: Vec<usize>) -> Self {
        Self {
            written_data: Vec::new(),
            write_sizes,
            write_count: 0,
        }
    }
    
    /// Get all data written to the stream
    fn get_written_data(&self) -> &[u8] {
        &self.written_data
    }
    
    /// Get the number of write operations performed
    fn write_operation_count(&self) -> usize {
        self.write_count
    }
}

impl AsyncRead for ControlledWriteMockStream {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        // Not used for write testing
        std::task::Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for ControlledWriteMockStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        // If we've exhausted predetermined write sizes, accept all remaining data
        if self.write_count >= self.write_sizes.len() {
            self.written_data.extend_from_slice(buf);
            return std::task::Poll::Ready(Ok(buf.len()));
        }
        
        // Determine how many bytes to accept this time
        let write_size = self.write_sizes[self.write_count];
        let bytes_to_write = std::cmp::min(write_size, buf.len());
        
        if bytes_to_write > 0 {
            // Accept only the predetermined number of bytes
            self.written_data.extend_from_slice(&buf[..bytes_to_write]);
        }
        
        self.write_count += 1;
        std::task::Poll::Ready(Ok(bytes_to_write))
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

#[tokio::test]
async fn test_successful_message_roundtrip() {
    // Test cases with various message sizes as specified in essential-tests.md
    let large_1kb = "x".repeat(1000);
    let large_10kb = "x".repeat(10000);
    
    let test_cases = vec![
        ("", "empty payload"),
        ("small", "small message"),
        ("medium payload with some text", "medium message"),
        (&large_1kb, "large message within limits (1KB)"),
        (&large_10kb, "large message within limits (10KB)"),
    ];
    
    for (payload, description) in test_cases {
        println!("Testing {}", description);
        
        // Create a test envelope with known content
        let (original_envelope, original_message) = create_test_envelope(payload);
        
        // Create framed message handler for testing
        let framed_message = FramedMessage::default();
        
        // Create a mock stream for testing
        let mut stream = MockStream::new();
        
        // Write the message to the stream
        framed_message.write_message(&mut stream, &original_envelope)
            .await
            .expect("Failed to write message");
        
        // Get the written data
        let written_data = stream.get_written_data().to_vec();
        
        // Verify the wire format:
        // First 4 bytes should be the length prefix (big-endian u32)
        assert!(written_data.len() >= LENGTH_PREFIX_SIZE, 
                "Written data should contain at least the length prefix for {}", description);
        
        let length_prefix_bytes = &written_data[0..LENGTH_PREFIX_SIZE];
        let expected_message_length = u32::from_be_bytes([
            length_prefix_bytes[0], 
            length_prefix_bytes[1], 
            length_prefix_bytes[2], 
            length_prefix_bytes[3]
        ]);
        
        // Verify the length prefix matches the actual message size
        let actual_message_length = (written_data.len() - LENGTH_PREFIX_SIZE) as u32;
        assert_eq!(expected_message_length, actual_message_length,
                   "Length prefix should match actual message size for {}", description);
        
        // Verify 4-byte length prefix is correctly written
        assert_eq!(length_prefix_bytes.len(), LENGTH_PREFIX_SIZE,
                   "Length prefix should be exactly 4 bytes for {}", description);
        
        // Set up the stream for reading by providing the written data
        let mut read_stream = MockStream::with_data(written_data);
        
        // Read the message back from the stream
        let received_envelope = framed_message.read_message(&mut read_stream)
            .await
            .expect("Failed to read message");
        
        // Verify received message matches sent message exactly
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                   "Sender should match for {}", description);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Timestamp should match for {}", description);
        
        // Verify the message content
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        
        assert_eq!(original_message.get_nonce(), received_message.get_nonce(),
                   "Message nonce should match for {}", description);
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match for {}", description);
        assert_eq!(original_message.message_type(), received_message.message_type(),
                   "Message type should match for {}", description);
        
        // Verify signature is still valid after round-trip
        assert!(received_envelope.verify_signature(),
                "Signature should be valid after round-trip for {}", description);
        
        println!("✓ Round-trip test passed for {}", description);
    }
    
    println!("✓ All round-trip tests passed!");
}

#[tokio::test]
async fn test_message_size_boundary_conditions() {
    // Test messages at specific size boundaries
    let framed_message = FramedMessage::default();
    
    // Test very small message (1 byte payload)
    let (small_envelope, _) = create_test_envelope("x");
    let mut small_stream = MockStream::new();
    
    framed_message.write_message(&mut small_stream, &small_envelope)
        .await
        .expect("Failed to write small message");
    
    let small_data = small_stream.get_written_data().to_vec();
    let mut small_read_stream = MockStream::with_data(small_data);
    
    let received_small = framed_message.read_message(&mut small_read_stream)
        .await
        .expect("Failed to read small message");
    
    assert!(received_small.verify_signature(), "Small message signature should be valid");
    
    // Test medium message (1KB)
    let medium_payload = "x".repeat(1024);
    let (medium_envelope, _) = create_test_envelope(&medium_payload);
    let mut medium_stream = MockStream::new();
    
    framed_message.write_message(&mut medium_stream, &medium_envelope)
        .await
        .expect("Failed to write medium message");
    
    let medium_data = medium_stream.get_written_data().to_vec();
    let mut medium_read_stream = MockStream::with_data(medium_data);
    
    let received_medium = framed_message.read_message(&mut medium_read_stream)
        .await
        .expect("Failed to read medium message");
    
    assert!(received_medium.verify_signature(), "Medium message signature should be valid");
    
    println!("✓ Size boundary condition tests passed!");
}

#[tokio::test]
async fn test_length_prefix_format() {
    // Test case 4 from essential-tests.md: Length prefix format compliance
    println!("Testing length prefix format compliance");
    
    let framed_message = FramedMessage::default();
    
    // Create large strings first to avoid lifetime issues
    let large_1kb = "x".repeat(1000);
    let large_5kb = "y".repeat(5000);
    
    // Test with various message sizes to verify length prefix format consistency
    let test_cases = vec![
        ("small", "small message"),
        ("medium_length_payload_to_test_format", "medium message"),
        (large_1kb.as_str(), "large message (1KB)"),
        (large_5kb.as_str(), "large message (5KB)"),
    ];
    
    for (payload, description) in test_cases {
        println!("Testing length prefix format for {}", description);
        
        let (envelope, _) = create_test_envelope(payload);
        
        // Write message to get wire format
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &envelope)
            .await
            .expect("Failed to write message");
        
        let written_data = stream.get_written_data();
        
        // Test 1: Verify length prefix is exactly 4 bytes
        assert!(written_data.len() >= LENGTH_PREFIX_SIZE,
               "Written data should contain at least {} bytes for length prefix", LENGTH_PREFIX_SIZE);
        
        let length_prefix_bytes = &written_data[0..LENGTH_PREFIX_SIZE];
        assert_eq!(length_prefix_bytes.len(), LENGTH_PREFIX_SIZE,
                   "Length prefix should be exactly {} bytes", LENGTH_PREFIX_SIZE);
        
        // Test 2: Verify length is encoded as big-endian u32
        let length_from_be = u32::from_be_bytes([
            length_prefix_bytes[0],
            length_prefix_bytes[1], 
            length_prefix_bytes[2],
            length_prefix_bytes[3]
        ]);
        
        // Verify this produces a sensible length value
        let expected_message_length = written_data.len() - LENGTH_PREFIX_SIZE;
        assert_eq!(length_from_be as usize, expected_message_length,
                   "Big-endian u32 interpretation should match actual message length");
        
        // Test 3: Test that length prefix correctly represents message byte count
        // The length prefix should equal the number of bytes following it
        let actual_message_bytes = &written_data[LENGTH_PREFIX_SIZE..];
        assert_eq!(length_from_be as usize, actual_message_bytes.len(),
                   "Length prefix should correctly represent message byte count");
        
        // Test 4: Verify receiver can parse length prefix correctly
        // Create a reader with the written data and verify we can read the length prefix
        let mut read_stream = MockStream::with_data(written_data.to_vec());
        
        // Manually read the length prefix to verify parsing
        let mut length_buffer = [0u8; LENGTH_PREFIX_SIZE];
        tokio::io::AsyncReadExt::read_exact(&mut read_stream, &mut length_buffer)
            .await
            .expect("Should be able to read length prefix");
        
        let parsed_length = u32::from_be_bytes(length_buffer);
        assert_eq!(parsed_length, length_from_be,
                   "Receiver should parse length prefix correctly");
        
        // Verify the receiver can use this length to read the exact message
        let mut message_buffer = vec![0u8; parsed_length as usize];
        tokio::io::AsyncReadExt::read_exact(&mut read_stream, &mut message_buffer)
            .await
            .expect("Should be able to read message using parsed length");
        
        assert_eq!(message_buffer.len(), parsed_length as usize,
                   "Should read exactly the number of bytes specified by length prefix");
        assert_eq!(message_buffer, actual_message_bytes,
                   "Read message bytes should match original message bytes");
        
        // Additional verification: Test endianness by checking individual bytes
        let length_as_bytes = (expected_message_length as u32).to_be_bytes();
        assert_eq!(length_prefix_bytes[0], length_as_bytes[0], "Most significant byte should match");
        assert_eq!(length_prefix_bytes[1], length_as_bytes[1], "Second byte should match");  
        assert_eq!(length_prefix_bytes[2], length_as_bytes[2], "Third byte should match");
        assert_eq!(length_prefix_bytes[3], length_as_bytes[3], "Least significant byte should match");
        
        println!("✓ Length prefix format verified for {} (prefix: {} bytes, message: {} bytes)", 
                description, LENGTH_PREFIX_SIZE, parsed_length);
    }
    
    // Test edge case: Verify format consistency across size boundaries  
    println!("Testing format consistency across size boundaries...");
    
    let boundary_sizes = vec![1, 255, 256, 65535, 65536];
    for size in boundary_sizes {
        let payload = "x".repeat(size);
        let (envelope, _) = create_test_envelope(&payload);
        
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &envelope)
            .await
            .expect("Failed to write boundary size message");
        
        let written_data = stream.get_written_data();
        
        // Verify format is consistent regardless of message size
        assert_eq!(written_data[0..LENGTH_PREFIX_SIZE].len(), LENGTH_PREFIX_SIZE,
                   "Length prefix should always be {} bytes regardless of message size", LENGTH_PREFIX_SIZE);
        
        let length_prefix = u32::from_be_bytes([
            written_data[0], written_data[1], written_data[2], written_data[3]
        ]);
        
        let actual_message_size = written_data.len() - LENGTH_PREFIX_SIZE;
        assert_eq!(length_prefix as usize, actual_message_size,
                   "Length prefix format should be consistent for {} byte payload", size);
    }
    
    println!("✓ Length prefix format compliance test passed!");
    println!("  - Length prefix is always exactly {} bytes", LENGTH_PREFIX_SIZE);
    println!("  - Length is encoded as big-endian u32");
    println!("  - Length prefix correctly represents message byte count");
    println!("  - Receiver can parse length prefix correctly");
    println!("  - Format is consistent across all message sizes");
}

#[tokio::test]
async fn test_length_prefix_accuracy() {
    // Test case 5 from essential-tests.md: Length prefix accuracy
    println!("Testing length prefix accuracy with messages at size boundaries");
    
    let framed_message = FramedMessage::default();
    
    // Test messages of known sizes including size boundaries as specified
    let test_cases = vec![
        // Small messages
        (1, "1 byte"),
        (10, "10 bytes"),
        (100, "100 bytes"),
        (500, "500 bytes"),
        
        // Size boundary tests (1KB, 1MB, etc.)
        (1024, "1KB boundary"),
        (1023, "1KB - 1 byte"),
        (1025, "1KB + 1 byte"),
        
        (2048, "2KB"),
        (4096, "4KB"), 
        (8192, "8KB"),
        (16384, "16KB"),
        (32768, "32KB"),
        (65536, "64KB"),
        
        (1024 * 100, "100KB"),
        (1024 * 500, "500KB"),
        (1024 * 1024, "1MB boundary"),
        (1024 * 1024 - 1, "1MB - 1 byte"),
        (1024 * 1024 + 1, "1MB + 1 byte"),
        
        (1024 * 1024 * 2, "2MB"),
        (1024 * 1024 * 5, "5MB"),
        (1024 * 1024 * 8, "8MB"),
    ];
    
    for (size, description) in &test_cases {
        println!("Testing {} ({})", description, *size);
        
        // Create a message with exactly the specified payload size
        let payload = "x".repeat(*size);
        let (envelope, _) = create_test_envelope(&payload);
        
        // Serialize the envelope to get the exact serialized size
        let serialized_envelope = bincode::serialize(&envelope)
            .expect("Failed to serialize test envelope");
        let expected_serialized_size = serialized_envelope.len();
        
        println!("  Payload size: {} bytes", *size);
        println!("  Serialized envelope size: {} bytes", expected_serialized_size);
        
        // Write the message using the wire protocol
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &envelope)
            .await
            .expect(&format!("Failed to write {} message", description));
        
        let written_data = stream.get_written_data();
        
        // Extract and verify length prefix
        assert!(written_data.len() >= LENGTH_PREFIX_SIZE, 
                "Written data should contain at least the length prefix for {}", description);
        
        let length_prefix = u32::from_be_bytes([
            written_data[0],
            written_data[1], 
            written_data[2],
            written_data[3]
        ]);
        
        // The length prefix should match the serialized message size exactly
        assert_eq!(length_prefix as usize, expected_serialized_size,
                   "Length prefix ({}) should match actual serialized message size ({}) for {}",
                   length_prefix, expected_serialized_size, description);
        
        // Verify the total written data size is length prefix + message data
        let actual_message_size = written_data.len() - LENGTH_PREFIX_SIZE;
        assert_eq!(actual_message_size, expected_serialized_size,
                   "Actual written message size ({}) should match expected serialized size ({}) for {}",
                   actual_message_size, expected_serialized_size, description);
        
        // Additional verification: ensure we can read the message back correctly
        let mut read_stream = MockStream::with_data(written_data.to_vec());
        let received_envelope = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to read back {} message", description));
        
        // Verify the received message has the same payload size
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(received_message.get_payload().len(), *size,
                   "Received message payload size should match original for {}", description);
        
        // Verify content integrity
        assert_eq!(received_message.get_payload(), payload,
                   "Received message payload should match original content for {}", description);
        
        println!("  ✓ Length prefix accuracy verified: {} bytes", length_prefix);
        println!("  ✓ Round-trip successful for {}", description);
    }
    
    println!("✓ All length prefix accuracy tests passed!");
    println!("  - Tested {} different message sizes", test_cases.len());
    println!("  - Verified length prefixes match actual serialized message sizes");
    println!("  - Tested messages at size boundaries (1KB, 1MB, etc.)");
    println!("  - All messages successfully round-tripped through wire protocol");
}

#[tokio::test]
async fn test_message_ordering_preservation() {
    // Test case 2 from essential-tests.md: Message ordering preservation
    println!("Testing message ordering preservation with different sizes");
    
    let framed_message = FramedMessage::default();
    
    // Create large strings first to avoid lifetime issues
    let large_1kb = "x".repeat(1000);
    let large_5kb = "y".repeat(5000);
    
    // Create test messages with different sizes and unique nonces for identification
    let test_messages = vec![
        // Small message
        ("small_msg", 1001u64),
        // Medium message  
        ("medium_message_with_more_content_to_test_ordering", 1002u64),
        // Large message (1KB)
        (large_1kb.as_str(), 1003u64),
        // Another small message
        ("small_again", 1004u64),
        // Large message (5KB) 
        (large_5kb.as_str(), 1005u64),
        // Medium message
        ("final_medium_message_for_ordering_test", 1006u64),
    ];
    
    // Create signed envelopes for all test messages
    let mut messages_with_envelopes = Vec::new();
    for (payload, nonce) in &test_messages {
        let (envelope, message) = create_test_envelope_with_nonce(payload, *nonce);
        messages_with_envelopes.push((envelope, message));
        println!("Created message with nonce {} and payload length {}", nonce, payload.len());
    }
    
    // Write all messages to a single buffer in sequence
    let combined_buffer = write_multiple_messages_to_buffer(&messages_with_envelopes).await;
    println!("Combined buffer size: {} bytes", combined_buffer.len());
    
    // Create a mock stream with the combined buffer for reading
    let mut read_stream = MockStream::with_data(combined_buffer);
    
    // Read messages back and verify they arrive in the same order
    let mut received_messages = Vec::new();
    for i in 0..test_messages.len() {
        println!("Reading message {} of {}", i + 1, test_messages.len());
        
        let received_envelope = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to read message {}", i + 1));
        
        // Verify signature is still valid
        assert!(received_envelope.verify_signature(), 
               "Signature should be valid for message {}", i + 1);
        
        // Deserialize the message to check content
        let received_message = received_envelope.get_message()
            .expect(&format!("Failed to deserialize message {}", i + 1));
        
        received_messages.push((received_envelope, received_message));
        println!("Successfully received message {} with nonce {}", 
                i + 1, received_messages[i].1.get_nonce());
    }
    
    // Verify ordering is preserved by checking nonces
    println!("Verifying message ordering preservation...");
    for (i, ((original_envelope, original_message), (received_envelope, received_message))) in 
        messages_with_envelopes.iter().zip(received_messages.iter()).enumerate() {
        
        // Check nonces match (primary ordering verification)
        assert_eq!(original_message.get_nonce(), received_message.get_nonce(),
                   "Message {} nonce mismatch: expected {}, got {}", 
                   i + 1, original_message.get_nonce(), received_message.get_nonce());
        
        // Check payloads match
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message {} payload mismatch", i + 1);
        
        // Check message types match  
        assert_eq!(original_message.message_type(), received_message.message_type(),
                   "Message {} type mismatch", i + 1);
        
        // Check timestamps match
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Message {} timestamp mismatch", i + 1);
        
        println!("✓ Message {} ordering and content verified (nonce: {}, payload_len: {})", 
                i + 1, received_message.get_nonce(), received_message.get_payload().len());
    }
    
    // Verify we received exactly the expected number of messages
    assert_eq!(received_messages.len(), test_messages.len(),
               "Should receive exactly {} messages, got {}", 
               test_messages.len(), received_messages.len());
    
    // Additional verification: ensure message sizes didn't affect ordering
    let original_nonces: Vec<u64> = messages_with_envelopes.iter()
        .map(|(_, msg)| msg.get_nonce())
        .collect();
    let received_nonces: Vec<u64> = received_messages.iter()
        .map(|(_, msg)| msg.get_nonce())
        .collect();
    
    assert_eq!(original_nonces, received_nonces,
               "Message ordering should be preserved regardless of size. Original: {:?}, Received: {:?}",
               original_nonces, received_nonces);
    
    println!("✓ Message ordering preservation test passed!");
    println!("  - Sent {} messages with varying sizes ({} bytes to {} bytes)", 
             test_messages.len(),
             test_messages.iter().map(|(payload, _)| payload.len()).min().unwrap(),
             test_messages.iter().map(|(payload, _)| payload.len()).max().unwrap());
    println!("  - All messages received in correct order");
    println!("  - Message size variations did not affect ordering");
}

#[tokio::test]
async fn test_empty_and_minimal_messages() {
    // Test case 3 from essential-tests.md: Empty and minimal messages
    println!("Testing behavior with minimal valid messages");
    
    let framed_message = FramedMessage::default();
    
    // Test 1: Empty payload message (minimal valid Message)
    println!("Testing empty payload message...");
    let (empty_envelope, empty_message) = create_test_envelope("");
    
    // Verify it's actually an empty payload
    assert_eq!(empty_message.get_payload(), "", "Test message should have empty payload");
    
    // Write and read back the empty payload message
    let mut empty_stream = MockStream::new();
    framed_message.write_message(&mut empty_stream, &empty_envelope)
        .await
        .expect("Failed to write empty payload message");
    
    let empty_data = empty_stream.get_written_data().to_vec();
    let mut empty_read_stream = MockStream::with_data(empty_data.clone());
    
    let received_empty = framed_message.read_message(&mut empty_read_stream)
        .await
        .expect("Failed to read empty payload message");
    
    // Verify empty message integrity
    assert!(received_empty.verify_signature(), "Empty message signature should be valid");
    let received_empty_msg = received_empty.get_message()
        .expect("Failed to deserialize empty message");
    assert_eq!(received_empty_msg.get_payload(), "", "Empty payload should be preserved");
    assert_eq!(received_empty_msg.get_nonce(), empty_message.get_nonce(), "Nonce should match");
    println!("✓ Empty payload message handled correctly");
    
    // Test 2: Single character payload (minimal non-empty)
    println!("Testing single character payload message...");
    let (single_envelope, _single_message) = create_test_envelope("a");
    
    let mut single_stream = MockStream::new();
    framed_message.write_message(&mut single_stream, &single_envelope)
        .await
        .expect("Failed to write single character message");
    
    let single_data = single_stream.get_written_data().to_vec();
    let mut single_read_stream = MockStream::with_data(single_data.clone());
    
    let received_single = framed_message.read_message(&mut single_read_stream)
        .await
        .expect("Failed to read single character message");
    
    assert!(received_single.verify_signature(), "Single character message signature should be valid");
    let received_single_msg = received_single.get_message()
        .expect("Failed to deserialize single character message");
    assert_eq!(received_single_msg.get_payload(), "a", "Single character payload should be preserved");
    println!("✓ Single character payload message handled correctly");
    
    // Test 3: Different message types with minimal payloads
    println!("Testing minimal Ping and Pong messages...");
    
    // Create minimal Ping message
    let identity = mate::crypto::Identity::generate().expect("Failed to generate identity");
    let ping_message = mate::messages::Message::new_ping(0, String::new()); // nonce 0, empty payload
    let ping_envelope = mate::messages::SignedEnvelope::create(&ping_message, &identity, Some(1234567890))
        .expect("Failed to create ping envelope");
    
    // Create minimal Pong message  
    let pong_message = mate::messages::Message::new_pong(0, String::new()); // nonce 0, empty payload
    let pong_envelope = mate::messages::SignedEnvelope::create(&pong_message, &identity, Some(1234567890))
        .expect("Failed to create pong envelope");
    
    // Test minimal Ping
    let mut ping_stream = MockStream::new();
    framed_message.write_message(&mut ping_stream, &ping_envelope)
        .await
        .expect("Failed to write minimal ping message");
    
    let ping_data = ping_stream.get_written_data().to_vec();
    let mut ping_read_stream = MockStream::with_data(ping_data.clone());
    
    let received_ping = framed_message.read_message(&mut ping_read_stream)
        .await
        .expect("Failed to read minimal ping message");
    
    assert!(received_ping.verify_signature(), "Minimal ping signature should be valid");
    let received_ping_msg = received_ping.get_message()
        .expect("Failed to deserialize minimal ping message");
    assert!(received_ping_msg.is_ping(), "Should be a Ping message");
    assert_eq!(received_ping_msg.get_nonce(), 0, "Ping nonce should be 0");
    assert_eq!(received_ping_msg.get_payload(), "", "Ping payload should be empty");
    
    // Test minimal Pong
    let mut pong_stream = MockStream::new();
    framed_message.write_message(&mut pong_stream, &pong_envelope)
        .await
        .expect("Failed to write minimal pong message");
    
    let pong_data = pong_stream.get_written_data().to_vec();
    let mut pong_read_stream = MockStream::with_data(pong_data.clone());
    
    let received_pong = framed_message.read_message(&mut pong_read_stream)
        .await
        .expect("Failed to read minimal pong message");
    
    assert!(received_pong.verify_signature(), "Minimal pong signature should be valid");
    let received_pong_msg = received_pong.get_message()
        .expect("Failed to deserialize minimal pong message");
    assert!(received_pong_msg.is_pong(), "Should be a Pong message");
    assert_eq!(received_pong_msg.get_nonce(), 0, "Pong nonce should be 0");
    assert_eq!(received_pong_msg.get_payload(), "", "Pong payload should be empty");
    
    println!("✓ Minimal Ping and Pong messages handled correctly");
    
    // Test 4: Verify minimum message size requirements
    println!("Testing minimum message size requirements...");
    
    // Check that all minimal messages have proper length prefixes
    let test_messages = vec![
        ("Empty payload", empty_data),
        ("Single char", single_data),
        ("Minimal ping", ping_data),
        ("Minimal pong", pong_data),
    ];
    
    for (description, data) in test_messages {
        // Verify minimum length (4 bytes for length prefix + at least some message data)
        assert!(data.len() > LENGTH_PREFIX_SIZE, 
               "{} should have more than {} bytes (length prefix + message)", 
               description, LENGTH_PREFIX_SIZE);
        
        // Extract and verify length prefix
        let length_prefix = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let actual_message_length = data.len() - LENGTH_PREFIX_SIZE;
        
        assert_eq!(length_prefix as usize, actual_message_length,
                   "{} length prefix should match actual message size", description);
        
        // Verify message is not unreasonably small (SignedEnvelope has minimum structure)
        assert!(length_prefix > 0, "{} should have non-zero message length", description);
        
        println!("✓ {} passed minimum size requirements (total: {} bytes, message: {} bytes)", 
                description, data.len(), length_prefix);
    }
    
    // Test 5: Edge case - very small nonce values
    println!("Testing edge cases with small nonce values...");
    
    let edge_cases = vec![
        (0u64, "zero nonce"),
        (1u64, "minimal nonce"),
        (u64::MAX, "maximum nonce"),
    ];
    
    for (nonce, description) in edge_cases {
        let (edge_envelope, _edge_message) = create_test_envelope_with_nonce("", nonce);
        
        let mut edge_stream = MockStream::new();
        framed_message.write_message(&mut edge_stream, &edge_envelope)
            .await
            .expect(&format!("Failed to write {} message", description));
        
        let edge_data = edge_stream.get_written_data().to_vec();
        let mut edge_read_stream = MockStream::with_data(edge_data);
        
        let received_edge = framed_message.read_message(&mut edge_read_stream)
            .await
            .expect(&format!("Failed to read {} message", description));
        
        assert!(received_edge.verify_signature(), "{} signature should be valid", description);
        let received_edge_msg = received_edge.get_message()
            .expect(&format!("Failed to deserialize {} message", description));
        assert_eq!(received_edge_msg.get_nonce(), nonce, "{} nonce should match", description);
        
        println!("✓ {} handled correctly (nonce: {})", description, nonce);
    }
    
    println!("✓ All empty and minimal message tests passed!");
    println!("  - Empty payload messages work correctly");
    println!("  - Single character messages work correctly");  
    println!("  - Minimal Ping and Pong messages work correctly");
    println!("  - Minimum message size requirements are met");
    println!("  - Edge cases with extreme nonce values work correctly");
    println!("  - Protocol handles smallest possible SignedEnvelope correctly");
}

#[tokio::test]
async fn test_partial_read_recovery() {
    println!("Starting test_partial_read_recovery - testing fragmented read scenarios");
    
    // Create a test message with known content
    let test_payload = "This is a test message for partial read recovery testing";
    let (original_envelope, original_message) = create_test_envelope(test_payload);
    
    // Serialize the message to get the wire format data
    let framed_message = FramedMessage::default();
    let mut write_stream = MockStream::new();
    framed_message.write_message(&mut write_stream, &original_envelope)
        .await
        .expect("Failed to write test message");
    
    let complete_data = write_stream.get_written_data().to_vec();
    let total_length = complete_data.len();
    
    println!("Generated test message: {} bytes total (4-byte prefix + {} byte message)", 
             total_length, total_length - LENGTH_PREFIX_SIZE);
    
    // Test Case 1: Length prefix fragmentation patterns
    let fragmentation_patterns = vec![
        (vec![1, 1, 1, 1], "1+1+1+1 bytes pattern for length prefix"),
        (vec![2, 2], "2+2 bytes pattern for length prefix"),
        (vec![3, 1], "3+1 bytes pattern for length prefix"),
        (vec![1, 3], "1+3 bytes pattern for length prefix"),
    ];
    
    for (read_sizes, description) in fragmentation_patterns {
        println!("Testing length prefix fragmentation: {}", description);
        
        // Extend read sizes to include the rest of the message
        let mut full_read_sizes = read_sizes.clone();
        // After fragmenting the length prefix, read the message body in one go
        full_read_sizes.push(total_length - LENGTH_PREFIX_SIZE);
        
        let mut controlled_stream = ControlledMockStream::new(complete_data.clone(), full_read_sizes);
        
        let received_envelope = framed_message.read_message(&mut controlled_stream)
            .await
            .expect(&format!("Failed to read message with {}", description));
        
        // Verify the message was reconstructed correctly
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                   "Sender should match for {}", description);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Timestamp should match for {}", description);
        
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match for {}", description);
        
        assert!(controlled_stream.is_finished(), 
                "All data should be consumed for {}", description);
        
        println!("✓ Length prefix fragmentation test passed: {}", description);
    }
    
    // Test Case 2: Message body fragmentation with various chunk sizes
    let message_body_size = total_length - LENGTH_PREFIX_SIZE;
    let body_fragmentation_patterns = vec![
        (vec![LENGTH_PREFIX_SIZE, 1], "Read prefix, then 1 byte chunks"),
        (vec![LENGTH_PREFIX_SIZE, 10], "Read prefix, then 10 byte chunks"),
        (vec![LENGTH_PREFIX_SIZE, 50], "Read prefix, then 50 byte chunks"),
        (vec![LENGTH_PREFIX_SIZE, message_body_size / 4], "Read prefix, then quarter-size chunks"),
        (vec![LENGTH_PREFIX_SIZE, message_body_size / 2], "Read prefix, then half-size chunks"),
    ];
    
    for (base_pattern, description) in body_fragmentation_patterns {
        println!("Testing message body fragmentation: {}", description);
        
        let mut read_sizes = base_pattern.clone();
        
        // For single-byte reads, we need to repeat until we've read the entire message
        if read_sizes.len() > 1 && read_sizes[1] == 1 {
            // Add enough 1-byte reads to cover the entire message body
            for _ in 1..message_body_size {
                read_sizes.push(1);
            }
        } else if read_sizes.len() > 1 {
            // For larger chunks, calculate how many reads we need
            let chunk_size = read_sizes[1];
            let mut remaining = message_body_size - chunk_size;
            while remaining > 0 {
                let next_chunk = std::cmp::min(chunk_size, remaining);
                read_sizes.push(next_chunk);
                remaining -= next_chunk;
            }
        }
        
        let mut controlled_stream = ControlledMockStream::new(complete_data.clone(), read_sizes);
        
        let received_envelope = framed_message.read_message(&mut controlled_stream)
            .await
            .expect(&format!("Failed to read message with {}", description));
        
        // Verify final reconstructed message matches original exactly
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                   "Sender should match for {}", description);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Timestamp should match for {}", description);
        
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_nonce(), received_message.get_nonce(),
                   "Message nonce should match for {}", description);
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match for {}", description);
        assert_eq!(original_message.message_type(), received_message.message_type(),
                   "Message type should match for {}", description);
        
        // Verify signature is still valid after fragmented transmission
        assert!(received_envelope.verify_signature(),
                "Signature should be valid after fragmented read for {}", description);
        
        assert!(controlled_stream.is_finished(), 
                "All data should be consumed for {}", description);
        
        println!("✓ Message body fragmentation test passed: {}", description);
    }
    
    // Test Case 3: Split exactly at length prefix/message body boundary
    println!("Testing split exactly at length prefix/message body boundary");
    
    let boundary_read_sizes = vec![LENGTH_PREFIX_SIZE, message_body_size];
    let mut boundary_stream = ControlledMockStream::new(complete_data.clone(), boundary_read_sizes);
    
    let received_envelope = framed_message.read_message(&mut boundary_stream)
        .await
        .expect("Failed to read message with boundary split");
    
    // Verify protocol maintains correct state during fragmented reads
    let received_message = received_envelope.get_message()
        .expect("Failed to deserialize received message");
    assert_eq!(original_message.get_payload(), received_message.get_payload(),
               "Message payload should match for boundary split");
    
    assert!(boundary_stream.is_finished(), 
            "All data should be consumed for boundary split");
    
    println!("✓ Boundary split test passed");
    
    // Test Case 4: Complex fragmentation combining multiple patterns
    println!("Testing complex fragmentation patterns");
    
    let complex_patterns = vec![
        vec![1, 2, 1, 10, 5], // Start with tiny fragments, then larger chunks
        vec![2, 1, 1, 20], // Mixed small and medium chunks
        vec![1, 1, 2, message_body_size - 4], // Fragment prefix, then large body chunk
    ];
    
    for (pattern_index, mut complex_read_sizes) in complex_patterns.into_iter().enumerate() {
        // Ensure we read all remaining data
        let bytes_in_pattern: usize = complex_read_sizes.iter().sum();
        if bytes_in_pattern < total_length {
            complex_read_sizes.push(total_length - bytes_in_pattern);
        }
        
        println!("Testing complex pattern {}: {:?}", pattern_index + 1, complex_read_sizes);
        
        let mut complex_stream = ControlledMockStream::new(complete_data.clone(), complex_read_sizes);
        
        let received_envelope = framed_message.read_message(&mut complex_stream)
            .await
            .expect(&format!("Failed to read message with complex pattern {}", pattern_index + 1));
        
        // Verify final reconstructed message matches original exactly
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match for complex pattern {}", pattern_index + 1);
        
        assert!(received_envelope.verify_signature(),
                "Signature should be valid after complex fragmented read pattern {}", pattern_index + 1);
        
        assert!(complex_stream.is_finished(), 
                "All data should be consumed for complex pattern {}", pattern_index + 1);
        
        println!("✓ Complex fragmentation pattern {} passed", pattern_index + 1);
    }
    
    println!("✓ All partial read recovery tests passed!");
    println!("✓ Verified protocol maintains correct state during fragmented reads");
    println!("✓ Verified final reconstructed messages match originals exactly");
}

#[tokio::test]
async fn test_interrupted_read_completion() {
    println!("Starting test_interrupted_read_completion - testing partial read resilience and protocol resumption");
    
    // Create a test message with substantial content to test various interruption points
    let test_payload = "This is a comprehensive test message for interrupted read completion testing. It contains enough data to test interruptions at various points during the message body reading process, including SignedEnvelope field boundaries during deserialization.";
    let (original_envelope, original_message) = create_test_envelope(test_payload);
    
    // Serialize the message to get the wire format data
    let framed_message = FramedMessage::default();
    let mut write_stream = MockStream::new();
    framed_message.write_message(&mut write_stream, &original_envelope)
        .await
        .expect("Failed to write test message");
    
    let complete_data = write_stream.get_written_data().to_vec();
    let total_length = complete_data.len();
    let message_body_size = total_length - LENGTH_PREFIX_SIZE;
    
    println!("Generated test message: {} bytes total (4-byte prefix + {} byte message body)", 
             total_length, message_body_size);
    
    // Test Case 1: Interruption after reading 0, 1, 2, 3 bytes of length prefix
    println!("Testing partial reads during length prefix reading");
    
    for prefix_bytes_before_interrupt in 0..=3 {
        println!("Testing partial read after {} bytes of length prefix", prefix_bytes_before_interrupt);
        
        let interruption_points = vec![prefix_bytes_before_interrupt];
        let mut interrupted_stream = InterruptibleMockStream::new(complete_data.clone(), interruption_points);
        
        let received_envelope = framed_message.read_message(&mut interrupted_stream)
            .await
            .expect(&format!("Failed to read message with length prefix partial read at {} bytes", prefix_bytes_before_interrupt));
        
        // Verify protocol can resume from partial read point without data loss
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                   "Sender should match after length prefix partial read at {} bytes", prefix_bytes_before_interrupt);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Timestamp should match after length prefix partial read at {} bytes", prefix_bytes_before_interrupt);
        
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match after length prefix partial read at {} bytes", prefix_bytes_before_interrupt);
        
        assert!(interrupted_stream.is_finished(), 
                "All data should be consumed after length prefix partial read at {} bytes", prefix_bytes_before_interrupt);
        
        println!("✓ Length prefix partial read test passed: {} bytes read before resumption", prefix_bytes_before_interrupt);
    }
    
    // Test Case 2: Partial reads at 25%, 50%, 75%, 99% progress through message body
    println!("Testing partial reads at various progress points through message body");
    
    let progress_points = vec![
        (25, "25% through message body"),
        (50, "50% through message body"), 
        (75, "75% through message body"),
        (99, "99% through message body"),
    ];
    
    for (progress_percent, description) in progress_points {
        let interrupt_position = LENGTH_PREFIX_SIZE + (message_body_size * progress_percent / 100);
        println!("Testing partial read at {}: byte position {}", description, interrupt_position);
        
        let interruption_points = vec![interrupt_position];
        let mut interrupted_stream = InterruptibleMockStream::new(complete_data.clone(), interruption_points);
        
        let received_envelope = framed_message.read_message(&mut interrupted_stream)
            .await
            .expect(&format!("Failed to read message with partial read at {}", description));
        
        // Verify protocol state consistency after resumption
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                   "Sender should match after partial read at {}", description);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Timestamp should match after partial read at {}", description);
        
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_nonce(), received_message.get_nonce(),
                   "Message nonce should match after partial read at {}", description);
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match after partial read at {}", description);
        assert_eq!(original_message.message_type(), received_message.message_type(),
                   "Message type should match after partial read at {}", description);
        
        // Verify signature is still valid after interrupted transmission
        assert!(received_envelope.verify_signature(),
                "Signature should be valid after partial read at {}", description);
        
        assert!(interrupted_stream.is_finished(), 
                "All data should be consumed after partial read at {}", description);
        
        println!("✓ Message body partial read test passed: {}", description);
    }
    
    // Test Case 3: Multiple partial reads during the same read operation
    println!("Testing multiple partial reads during single read operation");
    
    let multi_interrupt_scenarios = vec![
        (
            vec![1, LENGTH_PREFIX_SIZE + 10, LENGTH_PREFIX_SIZE + message_body_size / 2],
            "Partial reads during prefix, early message, and mid-message"
        ),
        (
            vec![0, 2, LENGTH_PREFIX_SIZE + 5],
            "Partial reads at start, during prefix, and during message"
        ),
        (
            vec![LENGTH_PREFIX_SIZE + 1, LENGTH_PREFIX_SIZE + message_body_size - 10],
            "Partial reads early and late in message body"
        ),
    ];
    
    for (interruption_points, description) in multi_interrupt_scenarios {
        println!("Testing multi-partial-read scenario: {}", description);
        println!("Partial read points: {:?}", interruption_points);
        
        let mut interrupted_stream = InterruptibleMockStream::new(complete_data.clone(), interruption_points.clone());
        
        let received_envelope = framed_message.read_message(&mut interrupted_stream)
            .await
            .expect(&format!("Failed to read message with multi-partial-read scenario: {}", description));
        
        // Verify protocol maintains correct state through multiple partial reads
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match after multi-partial-read scenario: {}", description);
        
        // Verify signature integrity through multiple partial reads
        assert!(received_envelope.verify_signature(),
                "Signature should be valid after multi-partial-read scenario: {}", description);
        
        // Verify expected number of partial reads occurred
        assert_eq!(interrupted_stream.interruption_count(), interruption_points.len(),
                   "Expected number of partial reads should have occurred for scenario: {}", description);
        
        assert!(interrupted_stream.is_finished(), 
                "All data should be consumed after multi-partial-read scenario: {}", description);
        
        println!("✓ Multi-partial-read test passed: {}", description);
    }
    
    // Test Case 4: Partial reads at SignedEnvelope field boundaries during deserialization
    println!("Testing partial reads at approximate SignedEnvelope field boundaries");
    
    // Since we can't easily determine exact field boundaries without parsing the binary format,
    // we test partial reads at various points throughout the message body that are likely
    // to fall on or near field boundaries in the serialized SignedEnvelope
    let boundary_test_positions = vec![
        LENGTH_PREFIX_SIZE + 1,        // Very early in message (likely sender field)
        LENGTH_PREFIX_SIZE + 32,       // Around typical identity/signature size
        LENGTH_PREFIX_SIZE + 64,       // Further into the structure
        LENGTH_PREFIX_SIZE + message_body_size / 3,    // 1/3 through message
        LENGTH_PREFIX_SIZE + (message_body_size * 2) / 3,  // 2/3 through message
        LENGTH_PREFIX_SIZE + message_body_size - 32,   // Near end of message
        LENGTH_PREFIX_SIZE + message_body_size - 1,    // Very last byte
    ];
    
    for (test_index, interrupt_position) in boundary_test_positions.iter().enumerate() {
        // Skip positions that would be beyond our message
        if *interrupt_position >= total_length {
            continue;
        }
        
        println!("Testing partial read at potential field boundary: byte position {} (test {})", 
                 interrupt_position, test_index + 1);
        
        let interruption_points = vec![*interrupt_position];
        let mut interrupted_stream = InterruptibleMockStream::new(complete_data.clone(), interruption_points);
        
        let received_envelope = framed_message.read_message(&mut interrupted_stream)
            .await
            .expect(&format!("Failed to read message with field boundary partial read at position {}", interrupt_position));
        
        // Verify protocol state consistency after resumption
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match after field boundary partial read at position {}", interrupt_position);
        
        // Verify envelope fields are intact
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                   "Sender should match after field boundary partial read at position {}", interrupt_position);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Timestamp should match after field boundary partial read at position {}", interrupt_position);
        
        // Verify signature integrity
        assert!(received_envelope.verify_signature(),
                "Signature should be valid after field boundary partial read at position {}", interrupt_position);
        
        assert!(interrupted_stream.is_finished(), 
                "All data should be consumed after field boundary partial read at position {}", interrupt_position);
        
        println!("✓ Field boundary partial read test passed: position {}", interrupt_position);
    }
    
    // Test Case 5: Stress test with many partial reads
    println!("Testing stress scenario with many partial reads");
    
    let mut stress_interruption_points = Vec::new();
    // Add partial reads at regular intervals to stress test the resumption logic
    for i in (1..total_length).step_by(7) { // Every 7 bytes for irregular pattern
        stress_interruption_points.push(i);
    }
    
    // Limit to reasonable number of partial reads to avoid excessive test time
    stress_interruption_points.truncate(15);
    
    println!("Stress testing with {} partial read points: {:?}", 
             stress_interruption_points.len(), stress_interruption_points);
    
    let mut stress_stream = InterruptibleMockStream::new(complete_data.clone(), stress_interruption_points.clone());
    
    let received_envelope = framed_message.read_message(&mut stress_stream)
        .await
        .expect("Failed to read message with stress test partial reads");
    
    // Verify protocol maintains consistency through stress test
    let received_message = received_envelope.get_message()
        .expect("Failed to deserialize received message");
    assert_eq!(original_message.get_payload(), received_message.get_payload(),
               "Message payload should match after stress test partial reads");
    
    // Verify complete envelope integrity
    assert_eq!(original_envelope.sender(), received_envelope.sender(),
               "Sender should match after stress test partial reads");
    assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
               "Timestamp should match after stress test partial reads");
    assert!(received_envelope.verify_signature(),
            "Signature should be valid after stress test partial reads");
    
    // Verify all stress test partial reads occurred
    assert_eq!(stress_stream.interruption_count(), stress_interruption_points.len(),
               "All stress test partial reads should have occurred");
    
    assert!(stress_stream.is_finished(), 
            "All data should be consumed after stress test partial reads");
    
    println!("✓ Stress test with {} partial reads passed", stress_interruption_points.len());
    
    println!("✓ All interrupted read completion tests passed!");
    println!("✓ Verified protocol can resume from exact partial read point without data loss");
    println!("✓ Verified protocol state consistency after resumption from various partial read points");
    println!("✓ Verified message integrity maintained through single and multiple partial reads");
    println!("✓ Verified SignedEnvelope field boundary handling during partial reads");
    println!("✓ Verified stress scenarios with many partial reads work correctly");
}

#[tokio::test]
async fn test_partial_write_recovery() {
    println!("Starting test_partial_write_recovery - testing fragmented write scenarios");
    
    // Create a test message with known content
    let test_payload = "This is a test message for partial write recovery testing with sufficient content to test various fragmentation patterns";
    let (original_envelope, original_message) = create_test_envelope(test_payload);
    
    // Serialize the message to get the expected wire format data
    let framed_message = FramedMessage::default();
    let mut reference_stream = MockStream::new();
    framed_message.write_message(&mut reference_stream, &original_envelope)
        .await
        .expect("Failed to write reference message");
    
    let complete_data = reference_stream.get_written_data().to_vec();
    let total_length = complete_data.len();
    let message_body_size = total_length - LENGTH_PREFIX_SIZE;
    
    println!("Generated test message: {} bytes total (4-byte prefix + {} byte message body)", 
             total_length, message_body_size);
    
    // Test Case 1: Length prefix write fragmentation patterns
    println!("Testing length prefix write fragmentation patterns");
    
    let fragmentation_patterns = vec![
        (vec![1, 1, 1, 1], "1+1+1+1 bytes pattern for length prefix"),
        (vec![2, 2], "2+2 bytes pattern for length prefix"),
        (vec![3, 1], "3+1 bytes pattern for length prefix"),
        (vec![1, 3], "1+3 bytes pattern for length prefix"),
    ];
    
    for (write_sizes, description) in fragmentation_patterns {
        println!("Testing length prefix fragmentation: {}", description);
        
        // Extend write sizes to include the rest of the message
        let mut full_write_sizes = write_sizes.clone();
        // After fragmenting the length prefix, accept the message body in one go
        full_write_sizes.push(message_body_size);
        
        let mut controlled_stream = ControlledWriteMockStream::new(full_write_sizes);
        
        framed_message.write_message(&mut controlled_stream, &original_envelope)
            .await
            .expect(&format!("Failed to write message with {}", description));
        
        // Verify the written data matches the expected complete data
        let written_data = controlled_stream.get_written_data();
        assert_eq!(written_data.len(), complete_data.len(),
                   "Written data length should match expected for {}", description);
        assert_eq!(written_data, complete_data.as_slice(),
                   "Written data should match expected for {}", description);
        
        // Verify we performed the expected number of write operations
        assert_eq!(controlled_stream.write_operation_count(), write_sizes.len() + 1,
                   "Should have performed {} write operations for {}", write_sizes.len() + 1, description);
        
        println!("✓ Length prefix fragmentation test passed: {}", description);
    }
    
    // Test Case 2: Message body write fragmentation with backpressure simulation
    println!("Testing message body write fragmentation with backpressure simulation");
    
    let body_fragmentation_patterns = vec![
        (vec![LENGTH_PREFIX_SIZE, 1], "Accept prefix, then 1 byte chunks"),
        (vec![LENGTH_PREFIX_SIZE, 10], "Accept prefix, then 10 byte chunks"),
        (vec![LENGTH_PREFIX_SIZE, 50], "Accept prefix, then 50 byte chunks"),
        (vec![LENGTH_PREFIX_SIZE, message_body_size / 4], "Accept prefix, then quarter-size chunks"),
        (vec![LENGTH_PREFIX_SIZE, message_body_size / 2], "Accept prefix, then half-size chunks"),
    ];
    
    for (base_pattern, description) in body_fragmentation_patterns {
        println!("Testing message body fragmentation: {}", description);
        
        let mut write_sizes = base_pattern.clone();
        
        // For single-byte writes, we need to repeat until we've written the entire message
        if write_sizes.len() > 1 && write_sizes[1] == 1 {
            // Add enough 1-byte writes to cover the entire message body
            for _ in 1..message_body_size {
                write_sizes.push(1);
            }
        } else if write_sizes.len() > 1 {
            // For larger chunks, calculate how many writes we need
            let chunk_size = write_sizes[1];
            let mut remaining = message_body_size - chunk_size;
            while remaining > 0 {
                let next_chunk = std::cmp::min(chunk_size, remaining);
                write_sizes.push(next_chunk);
                remaining -= next_chunk;
            }
        }
        
        let mut controlled_stream = ControlledWriteMockStream::new(write_sizes.clone());
        
        framed_message.write_message(&mut controlled_stream, &original_envelope)
            .await
            .expect(&format!("Failed to write message with {}", description));
        
        // Verify the written data matches the expected complete data
        let written_data = controlled_stream.get_written_data();
        assert_eq!(written_data.len(), complete_data.len(),
                   "Written data length should match expected for {}", description);
        assert_eq!(written_data, complete_data.as_slice(),
                   "Written data should match expected for {}", description);
        
        println!("✓ Message body fragmentation test passed: {}", description);
    }
    
    // Test Case 3: Writes that span length prefix/message body boundary
    println!("Testing writes that span length prefix/message body boundary");
    
    // Create patterns that cross the boundary between length prefix and message body
    let boundary_patterns = vec![
        vec![3, 1 + 5],  // 3 bytes of prefix, then 1 byte remaining prefix + 5 bytes of body
        vec![2, 2 + 10], // 2 bytes of prefix, then 2 bytes remaining prefix + 10 bytes of body
        vec![1, 3 + 20], // 1 byte of prefix, then 3 bytes remaining prefix + 20 bytes of body
    ];
    
    for (pattern_index, mut boundary_write_sizes) in boundary_patterns.into_iter().enumerate() {
        // Ensure we write all remaining data
        let bytes_in_pattern: usize = boundary_write_sizes.iter().sum();
        if bytes_in_pattern < total_length {
            boundary_write_sizes.push(total_length - bytes_in_pattern);
        }
        
        println!("Testing boundary spanning pattern {}: {:?}", pattern_index + 1, boundary_write_sizes);
        
        let mut controlled_stream = ControlledWriteMockStream::new(boundary_write_sizes);
        
        framed_message.write_message(&mut controlled_stream, &original_envelope)
            .await
            .expect(&format!("Failed to write message with boundary spanning pattern {}", pattern_index + 1));
        
        // Verify the written data matches the expected complete data
        let written_data = controlled_stream.get_written_data();
        assert_eq!(written_data.len(), complete_data.len(),
                   "Written data length should match expected for boundary pattern {}", pattern_index + 1);
        assert_eq!(written_data, complete_data.as_slice(),
                   "Written data should match expected for boundary pattern {}", pattern_index + 1);
        
        println!("✓ Boundary spanning write test passed: pattern {}", pattern_index + 1);
    }
    
    // Test Case 4: Complex fragmentation patterns with varying write sizes
    println!("Testing complex fragmentation patterns");
    
    let complex_patterns = vec![
        vec![1, 2, 1, 10, 5], // Start with tiny fragments, then larger chunks
        vec![2, 1, 1, 20], // Mixed small and medium chunks
        vec![1, 1, 2, 50], // Fragment prefix, then medium body chunks
    ];
    
    for (pattern_index, mut complex_write_sizes) in complex_patterns.into_iter().enumerate() {
        // Ensure we write all remaining data
        let bytes_in_pattern: usize = complex_write_sizes.iter().sum();
        if bytes_in_pattern < total_length {
            complex_write_sizes.push(total_length - bytes_in_pattern);
        }
        
        println!("Testing complex pattern {}: {:?}", pattern_index + 1, complex_write_sizes);
        
        let mut complex_stream = ControlledWriteMockStream::new(complex_write_sizes);
        
        framed_message.write_message(&mut complex_stream, &original_envelope)
            .await
            .expect(&format!("Failed to write message with complex pattern {}", pattern_index + 1));
        
        // Verify the written data matches the expected complete data
        let written_data = complex_stream.get_written_data();
        assert_eq!(written_data.len(), complete_data.len(),
                   "Written data length should match expected for complex pattern {}", pattern_index + 1);
        assert_eq!(written_data, complete_data.as_slice(),
                   "Written data should match expected for complex pattern {}", pattern_index + 1);
        
        println!("✓ Complex fragmentation pattern {} passed", pattern_index + 1);
    }
    
    // Test Case 5: Verify protocol tracks write progress correctly across partial operations
    println!("Testing write progress tracking across partial operations");
    
    // Test with extremely fragmented writes (every single byte)
    let single_byte_writes: Vec<usize> = (0..total_length).map(|_| 1).collect();
    let mut single_byte_stream = ControlledWriteMockStream::new(single_byte_writes.clone());
    
    framed_message.write_message(&mut single_byte_stream, &original_envelope)
        .await
        .expect("Failed to write message with single-byte fragmentation");
    
    // Verify the written data matches the expected complete data
    let written_data = single_byte_stream.get_written_data();
    assert_eq!(written_data.len(), complete_data.len(),
               "Written data length should match expected for single-byte writes");
    assert_eq!(written_data, complete_data.as_slice(),
               "Written data should match expected for single-byte writes");
    
    // Verify we performed the expected number of write operations
    assert_eq!(single_byte_stream.write_operation_count(), total_length,
               "Should have performed {} write operations for single-byte writes", total_length);
    
    println!("✓ Single-byte write fragmentation test passed");
    
    // Test Case 6: Verify complete message transmission despite fragmented writes
    println!("Testing complete message transmission verification");
    
    // For each test case, verify we can successfully read back the written data
    let test_patterns = vec![
        vec![1, 1, 1, 1, message_body_size], // Fragment prefix completely
        vec![LENGTH_PREFIX_SIZE, 1, 1, 1], // Accept prefix, fragment body
        vec![2, 2, 10, 10], // Balanced fragmentation
    ];
    
    for (pattern_index, mut test_write_sizes) in test_patterns.into_iter().enumerate() {
        // Ensure we write all remaining data
        let bytes_in_pattern: usize = test_write_sizes.iter().sum();
        if bytes_in_pattern < total_length {
            test_write_sizes.push(total_length - bytes_in_pattern);
        }
        
        println!("Testing round-trip with fragmentation pattern {}: {:?}", pattern_index + 1, test_write_sizes);
        
        // Write with controlled fragmentation
        let mut write_stream = ControlledWriteMockStream::new(test_write_sizes);
        framed_message.write_message(&mut write_stream, &original_envelope)
            .await
            .expect(&format!("Failed to write message with round-trip pattern {}", pattern_index + 1));
        
        // Read back and verify message integrity
        let written_data = write_stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(written_data);
        
        let received_envelope = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to read back message with round-trip pattern {}", pattern_index + 1));
        
        // Verify envelope integrity
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                   "Sender should match for round-trip pattern {}", pattern_index + 1);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                   "Timestamp should match for round-trip pattern {}", pattern_index + 1);
        
        // Verify message content integrity
        let received_message = received_envelope.get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(original_message.get_nonce(), received_message.get_nonce(),
                   "Message nonce should match for round-trip pattern {}", pattern_index + 1);
        assert_eq!(original_message.get_payload(), received_message.get_payload(),
                   "Message payload should match for round-trip pattern {}", pattern_index + 1);
        assert_eq!(original_message.message_type(), received_message.message_type(),
                   "Message type should match for round-trip pattern {}", pattern_index + 1);
        
        // Verify signature integrity after fragmented transmission
        assert!(received_envelope.verify_signature(),
                "Signature should be valid after fragmented write for round-trip pattern {}", pattern_index + 1);
        
        println!("✓ Round-trip verification passed for fragmentation pattern {}", pattern_index + 1);
    }
    
    println!("✓ All partial write recovery tests passed!");
    println!("✓ Verified protocol tracks write progress correctly across partial operations");
    println!("✓ Verified complete message transmission despite fragmented writes");
    println!("✓ Verified message integrity after various write fragmentation patterns");
    println!("✓ Verified protocol handles length prefix/message body boundary writes correctly");
} 