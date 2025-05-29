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
async fn test_length_prefix_accuracy() {
    let framed_message = FramedMessage::default();
    
    // Test messages of known sizes to verify length prefix accuracy
    let test_sizes = vec![1, 10, 100, 500, 1000, 5000];
    
    for size in test_sizes {
        let payload = "x".repeat(size);
        let (envelope, _) = create_test_envelope(&payload);
        
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &envelope)
            .await
            .expect("Failed to write message");
        
        let written_data = stream.get_written_data();
        
        // Extract length prefix
        let length_prefix = u32::from_be_bytes([
            written_data[0],
            written_data[1], 
            written_data[2],
            written_data[3]
        ]);
        
        // Verify length prefix matches actual serialized message size
        let actual_message_size = written_data.len() - LENGTH_PREFIX_SIZE;
        assert_eq!(length_prefix as usize, actual_message_size,
                   "Length prefix should match actual serialized size for {} byte payload", size);
        
        println!("✓ Length accuracy verified for {} byte payload", size);
    }
    
    println!("✓ Length prefix accuracy tests passed!");
} 