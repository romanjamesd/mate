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