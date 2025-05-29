use mate::crypto::Identity;
use mate::messages::{Message, SignedEnvelope};
use mate::messages::wire::FramedMessage;
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

/// Helper function to create a test SignedEnvelope with a specific message and identity
fn create_test_envelope_with_identity(message: &Message, identity: &Identity, timestamp: Option<u64>) -> SignedEnvelope {
    SignedEnvelope::create(message, identity, timestamp)
        .expect("Failed to create signed envelope")
}

/// Helper function to create a test SignedEnvelope with default identity
fn create_test_envelope(message: &Message) -> (SignedEnvelope, Identity) {
    let identity = Identity::generate().expect("Failed to generate identity");
    let envelope = create_test_envelope_with_identity(message, &identity, Some(1234567890));
    (envelope, identity)
}

#[tokio::test]
async fn test_signed_envelope_transmission_integrity() {
    let framed_message = FramedMessage::default();
    
    // Test data: various Message types with different characteristics
    let test_cases = vec![
        // Basic ping messages
        ("small_ping", Message::new_ping(1, "Hello".to_string())),
        ("large_ping", Message::new_ping(999, "A".repeat(1000))),
        ("empty_payload_ping", Message::new_ping(0, String::new())),
        ("max_nonce_ping", Message::new_ping(u64::MAX, "Max nonce test".to_string())),
        
        // Basic pong messages  
        ("small_pong", Message::new_pong(42, "World".to_string())),
        ("large_pong", Message::new_pong(777, "B".repeat(500))),
        ("empty_payload_pong", Message::new_pong(100, String::new())),
        ("max_nonce_pong", Message::new_pong(u64::MAX, "Max nonce pong".to_string())),
        
        // Special character and Unicode tests
        ("unicode_ping", Message::new_ping(123, "🚀 Hello, 世界! 🎉".to_string())),
        ("special_chars_pong", Message::new_pong(456, "!@#$%^&*()_+-=[]{}|;':\",./<>?".to_string())),
        
        // Newlines and control characters
        ("multiline_ping", Message::new_ping(789, "Line 1\nLine 2\tTabbed\r\nWindows".to_string())),
        ("binary_like_pong", Message::new_pong(321, "\x00\x01\x02\x7F\x7E\x7D".to_string())),
    ];
    
    println!("Testing SignedEnvelope transmission integrity with {} test cases", test_cases.len());
    
    for (description, original_message) in test_cases {
        println!("  Testing case: {}", description);
        
        // Create identity and signed envelope
        let identity = Identity::generate().expect("Failed to generate identity");
        let original_envelope = create_test_envelope_with_identity(&original_message, &identity, Some(1234567890));
        
        // Store original values for comparison
        let original_sender = original_envelope.sender().to_string();
        let original_timestamp = original_envelope.timestamp();
        let original_message_nonce = original_message.get_nonce();
        let original_message_payload = original_message.get_payload().to_string();
        let original_message_type = original_message.message_type();
        
        // Verify signature before transmission
        assert!(original_envelope.verify_signature(), 
               "Original envelope signature should be valid for {}", description);
        
        // Send SignedEnvelope through wire protocol
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &original_envelope)
            .await
            .expect(&format!("Failed to write envelope for {}", description));
        
        // Read it back through wire protocol
        let written_data = stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(written_data);
        
        let received_envelope = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to read envelope for {}", description));
        
        // Verify signatures remain valid after transmission
        assert!(received_envelope.verify_signature(),
               "Signature should remain valid after transmission for {}", description);
        
        // Verify envelope metadata is preserved
        assert_eq!(original_sender, received_envelope.sender(),
                  "Sender should be preserved for {}", description);
        assert_eq!(original_timestamp, received_envelope.timestamp(),
                  "Timestamp should be preserved for {}", description);
        
        // Verify timestamp and identity information is intact
        assert_eq!(original_envelope.sender(), received_envelope.sender(),
                  "Identity information should be intact for {}", description);
        assert_eq!(original_envelope.timestamp(), received_envelope.timestamp(),
                  "Timestamp information should be intact for {}", description);
        
        // Extract and verify the message content
        let received_message = received_envelope.get_message()
            .expect(&format!("Failed to deserialize message for {}", description));
        
        assert_eq!(original_message_nonce, received_message.get_nonce(),
                  "Message nonce should be preserved for {}", description);
        assert_eq!(original_message_payload, received_message.get_payload(),
                  "Message payload should be preserved for {}", description);
        assert_eq!(original_message_type, received_message.message_type(),
                  "Message type should be preserved for {}", description);
        
        // Additional integrity checks
        assert_eq!(original_message.is_ping(), received_message.is_ping(),
                  "Ping status should be preserved for {}", description);
        assert_eq!(original_message.is_pong(), received_message.is_pong(),
                  "Pong status should be preserved for {}", description);
        
        println!("    ✓ {} passed all integrity checks", description);
    }
    
    println!("✓ All SignedEnvelope transmission integrity tests passed");
}

#[tokio::test]
async fn test_message_type_compatibility() {
    let framed_message = FramedMessage::default();
    
    // Test all supported Message variants work with wire protocol
    println!("Testing wire protocol compatibility with all Message types");
    
    let identity = Identity::generate().expect("Failed to generate identity");
    
    // Test cases covering different aspects of Message types
    let message_variants = vec![
        ("ping_zero_nonce", Message::new_ping(0, "Zero nonce ping".to_string())),
        ("ping_max_nonce", Message::new_ping(u64::MAX, "Max nonce ping".to_string())),
        ("pong_zero_nonce", Message::new_pong(0, "Zero nonce pong".to_string())),
        ("pong_max_nonce", Message::new_pong(u64::MAX, "Max nonce pong".to_string())),
    ];
    
    for (description, message) in message_variants {
        println!("  Testing {}", description);
        
        // Create envelope
        let envelope = create_test_envelope_with_identity(&message, &identity, Some(1234567890));
        
        // Transmit through wire protocol
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &envelope)
            .await
            .expect(&format!("Failed to transmit {}", description));
        
        // Receive and verify
        let data = stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(data);
        
        let received = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to receive {}", description));
        
        assert!(received.verify_signature(), 
               "Signature should be valid for {}", description);
        
        let received_msg = received.get_message()
            .expect(&format!("Failed to deserialize message for {}", description));
        
        // Verify message type specific properties
        assert_eq!(message.is_ping(), received_msg.is_ping(),
                  "Ping type should be preserved for {}", description);
        assert_eq!(message.is_pong(), received_msg.is_pong(),
                  "Pong type should be preserved for {}", description);
        assert_eq!(message.get_nonce(), received_msg.get_nonce(),
                  "Nonce should be preserved for {}", description);
        assert_eq!(message.get_payload(), received_msg.get_payload(),
                  "Payload should be preserved for {}", description);
        
        println!("    ✓ {} successfully transmitted and verified", description);
    }
    
    println!("✓ All Message type compatibility tests passed");
}

#[tokio::test]
async fn test_cross_identity_signature_verification() {
    let framed_message = FramedMessage::default();
    
    println!("Testing signature verification across different identities");
    
    // Create two different identities
    let identity1 = Identity::generate().expect("Failed to generate identity1");
    let identity2 = Identity::generate().expect("Failed to generate identity2");
    
    let message = Message::new_ping(42, "Cross-identity test".to_string());
    
    // Create envelope with identity1
    let envelope1 = create_test_envelope_with_identity(&message, &identity1, Some(1234567890));
    
    // Transmit through wire protocol
    let mut stream = MockStream::new();
    framed_message.write_message(&mut stream, &envelope1)
        .await
        .expect("Failed to transmit envelope");
    
    let data = stream.get_written_data().to_vec();
    let mut read_stream = MockStream::with_data(data);
    
    let received = framed_message.read_message(&mut read_stream)
        .await
        .expect("Failed to receive envelope");
    
    // Verify signature is valid (it should be - same identity)
    assert!(received.verify_signature(), 
           "Signature should be valid for same identity");
    
    // Verify the sender is identity1, not identity2
    assert_eq!(received.sender(), identity1.peer_id().as_str(),
              "Sender should match identity1");
    assert_ne!(received.sender(), identity2.peer_id().as_str(),
              "Sender should not match identity2");
    
    println!("✓ Cross-identity signature verification test passed");
}

#[tokio::test]
async fn test_timestamp_preservation_across_transmission() {
    let framed_message = FramedMessage::default();
    
    println!("Testing timestamp preservation across wire protocol transmission");
    
    let identity = Identity::generate().expect("Failed to generate identity");
    let message = Message::new_ping(123, "Timestamp test".to_string());
    
    // Test various timestamp values
    let timestamp_test_cases = vec![
        ("zero_timestamp", 0),
        ("unix_epoch", 1),
        ("y2k_timestamp", 946684800), // January 1, 2000
        ("recent_timestamp", 1640995200), // January 1, 2022
        ("future_timestamp", 2147483647), // January 19, 2038 (near i32 max)
        ("large_timestamp", u64::MAX), // Maximum possible timestamp
    ];
    
    for (description, timestamp) in timestamp_test_cases {
        println!("  Testing {}: {}", description, timestamp);
        
        let envelope = create_test_envelope_with_identity(&message, &identity, Some(timestamp));
        
        // Verify original timestamp
        assert_eq!(envelope.timestamp(), timestamp,
                  "Original timestamp should match for {}", description);
        
        // Transmit through wire protocol
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &envelope)
            .await
            .expect(&format!("Failed to transmit for {}", description));
        
        let data = stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(data);
        
        let received = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to receive for {}", description));
        
        // Verify timestamp is preserved exactly
        assert_eq!(received.timestamp(), timestamp,
                  "Timestamp should be preserved exactly for {}", description);
        
        // Verify signature is still valid
        assert!(received.verify_signature(),
               "Signature should remain valid for {}", description);
        
        println!("    ✓ {} preserved timestamp correctly", description);
    }
    
    println!("✓ All timestamp preservation tests passed");
}

#[tokio::test]
async fn test_large_message_transmission_integrity() {
    let framed_message = FramedMessage::default();
    
    println!("Testing transmission integrity with large messages");
    
    let identity = Identity::generate().expect("Failed to generate identity");
    
    // Test progressively larger messages
    let size_test_cases = vec![
        ("1KB", 1024),
        ("10KB", 10 * 1024),
        ("100KB", 100 * 1024),
        ("500KB", 500 * 1024),
    ];
    
    for (description, size) in size_test_cases {
        println!("  Testing {} message", description);
        
        // Create a large payload with predictable content for verification
        let mut payload = String::with_capacity(size);
        for i in 0..size {
            payload.push(char::from(b'A' + (i % 26) as u8));
        }
        
        let message = Message::new_ping(size as u64, payload.clone());
        let envelope = create_test_envelope_with_identity(&message, &identity, Some(1234567890));
        
        // Transmit through wire protocol
        let mut stream = MockStream::new();
        framed_message.write_message(&mut stream, &envelope)
            .await
            .expect(&format!("Failed to transmit {} message", description));
        
        let data = stream.get_written_data().to_vec();
        let mut read_stream = MockStream::with_data(data);
        
        let received = framed_message.read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to receive {} message", description));
        
        // Verify signature integrity
        assert!(received.verify_signature(),
               "Signature should be valid for {} message", description);
        
        // Verify message content integrity
        let received_msg = received.get_message()
            .expect(&format!("Failed to deserialize {} message", description));
        
        assert_eq!(received_msg.get_nonce(), size as u64,
                  "Nonce should be preserved for {} message", description);
        assert_eq!(received_msg.get_payload(), &payload,
                  "Payload should be preserved exactly for {} message", description);
        assert_eq!(received_msg.get_payload().len(), size,
                  "Payload size should be preserved for {} message", description);
        
        println!("    ✓ {} message transmitted with full integrity", description);
    }
    
    println!("✓ All large message transmission integrity tests passed");
} 