//! Tests for basic message transmission and ordering

use crate::common::mock_streams::*;
use crate::common::test_data::*;
use mate::messages::wire::{FramedMessage, LENGTH_PREFIX_SIZE};

#[tokio::test]
async fn test_successful_message_roundtrip() {
    // Test case 1 from essential-tests.md
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
        framed_message
            .write_message(&mut stream, &original_envelope)
            .await
            .expect("Failed to write message");

        // Get the written data
        let written_data = stream.get_written_data().to_vec();

        // Verify the wire format:
        // First 4 bytes should be the length prefix (big-endian u32)
        assert!(
            written_data.len() >= LENGTH_PREFIX_SIZE,
            "Written data should contain at least the length prefix for {}",
            description
        );

        let length_prefix_bytes = &written_data[0..LENGTH_PREFIX_SIZE];
        let expected_message_length = u32::from_be_bytes([
            length_prefix_bytes[0],
            length_prefix_bytes[1],
            length_prefix_bytes[2],
            length_prefix_bytes[3],
        ]);

        // Verify the length prefix matches the actual message size
        let actual_message_length = (written_data.len() - LENGTH_PREFIX_SIZE) as u32;
        assert_eq!(
            expected_message_length, actual_message_length,
            "Length prefix should match actual message size for {}",
            description
        );

        // Verify 4-byte length prefix is correctly written
        assert_eq!(
            length_prefix_bytes.len(),
            LENGTH_PREFIX_SIZE,
            "Length prefix should be exactly 4 bytes for {}",
            description
        );

        // Set up the stream for reading by providing the written data
        let mut read_stream = MockStream::with_data(written_data);

        // Read the message back from the stream
        let received_envelope = framed_message
            .read_message(&mut read_stream)
            .await
            .expect("Failed to read message");

        // Verify received message matches sent message exactly
        assert_eq!(
            original_envelope.sender(),
            received_envelope.sender(),
            "Sender should match for {}",
            description
        );
        assert_eq!(
            original_envelope.timestamp(),
            received_envelope.timestamp(),
            "Timestamp should match for {}",
            description
        );

        // Verify the message content
        let received_message = received_envelope
            .get_message()
            .expect("Failed to deserialize received message");

        assert_eq!(
            original_message.get_nonce(),
            received_message.get_nonce(),
            "Message nonce should match for {}",
            description
        );
        assert_eq!(
            original_message.get_payload(),
            received_message.get_payload(),
            "Message payload should match for {}",
            description
        );
        assert_eq!(
            original_message.message_type(),
            received_message.message_type(),
            "Message type should match for {}",
            description
        );

        // Verify signature is still valid after round-trip
        assert!(
            received_envelope.verify_signature(),
            "Signature should be valid after round-trip for {}",
            description
        );

        println!("✓ Round-trip test passed for {}", description);
    }

    println!("✓ All round-trip tests passed!");
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
        println!(
            "Created message with nonce {} and payload length {}",
            nonce,
            payload.len()
        );
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

        let received_envelope = framed_message
            .read_message(&mut read_stream)
            .await
            .unwrap_or_else(|_| panic!("Failed to read message {}", i + 1));

        // Verify signature is still valid
        assert!(
            received_envelope.verify_signature(),
            "Signature should be valid for message {}",
            i + 1
        );

        // Deserialize the message to check content
        let received_message = received_envelope
            .get_message()
            .unwrap_or_else(|_| panic!("Failed to deserialize message {}", i + 1));

        received_messages.push((received_envelope, received_message));
        println!(
            "Successfully received message {} with nonce {}",
            i + 1,
            received_messages[i].1.get_nonce()
        );
    }

    // Verify ordering is preserved by checking nonces
    println!("Verifying message ordering preservation...");
    for (i, ((original_envelope, original_message), (received_envelope, received_message))) in
        messages_with_envelopes
            .iter()
            .zip(received_messages.iter())
            .enumerate()
    {
        // Check nonces match (primary ordering verification)
        assert_eq!(
            original_message.get_nonce(),
            received_message.get_nonce(),
            "Message {} nonce mismatch: expected {}, got {}",
            i + 1,
            original_message.get_nonce(),
            received_message.get_nonce()
        );

        // Check payloads match
        assert_eq!(
            original_message.get_payload(),
            received_message.get_payload(),
            "Message {} payload mismatch",
            i + 1
        );

        // Check message types match
        assert_eq!(
            original_message.message_type(),
            received_message.message_type(),
            "Message {} type mismatch",
            i + 1
        );

        // Check timestamps match
        assert_eq!(
            original_envelope.timestamp(),
            received_envelope.timestamp(),
            "Message {} timestamp mismatch",
            i + 1
        );

        println!(
            "✓ Message {} ordering and content verified (nonce: {}, payload_len: {})",
            i + 1,
            received_message.get_nonce(),
            received_message.get_payload().len()
        );
    }

    // Verify we received exactly the expected number of messages
    assert_eq!(
        received_messages.len(),
        test_messages.len(),
        "Should receive exactly {} messages, got {}",
        test_messages.len(),
        received_messages.len()
    );

    // Additional verification: ensure message sizes didn't affect ordering
    let original_nonces: Vec<u64> = messages_with_envelopes
        .iter()
        .map(|(_, msg)| msg.get_nonce())
        .collect();
    let received_nonces: Vec<u64> = received_messages
        .iter()
        .map(|(_, msg)| msg.get_nonce())
        .collect();

    assert_eq!(
        original_nonces, received_nonces,
        "Message ordering should be preserved regardless of size. Original: {:?}, Received: {:?}",
        original_nonces, received_nonces
    );

    println!("✓ Message ordering preservation test passed!");
    println!(
        "  - Sent {} messages with varying sizes ({} bytes to {} bytes)",
        test_messages.len(),
        test_messages
            .iter()
            .map(|(payload, _)| payload.len())
            .min()
            .unwrap(),
        test_messages
            .iter()
            .map(|(payload, _)| payload.len())
            .max()
            .unwrap()
    );
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
    assert_eq!(
        empty_message.get_payload(),
        "",
        "Test message should have empty payload"
    );

    // Write and read back the empty payload message
    let mut empty_stream = MockStream::new();
    framed_message
        .write_message(&mut empty_stream, &empty_envelope)
        .await
        .expect("Failed to write empty payload message");

    let empty_data = empty_stream.get_written_data().to_vec();
    let mut empty_read_stream = MockStream::with_data(empty_data.clone());

    let received_empty = framed_message
        .read_message(&mut empty_read_stream)
        .await
        .expect("Failed to read empty payload message");

    // Verify empty message integrity
    assert!(
        received_empty.verify_signature(),
        "Empty message signature should be valid"
    );
    let received_empty_msg = received_empty
        .get_message()
        .expect("Failed to deserialize empty message");
    assert_eq!(
        received_empty_msg.get_payload(),
        "",
        "Empty payload should be preserved"
    );
    assert_eq!(
        received_empty_msg.get_nonce(),
        empty_message.get_nonce(),
        "Nonce should match"
    );
    println!("✓ Empty payload message handled correctly");

    // Test 2: Single character payload (minimal non-empty)
    println!("Testing single character payload message...");
    let (single_envelope, _single_message) = create_test_envelope("a");

    let mut single_stream = MockStream::new();
    framed_message
        .write_message(&mut single_stream, &single_envelope)
        .await
        .expect("Failed to write single character message");

    let single_data = single_stream.get_written_data().to_vec();
    let mut single_read_stream = MockStream::with_data(single_data.clone());

    let received_single = framed_message
        .read_message(&mut single_read_stream)
        .await
        .expect("Failed to read single character message");

    assert!(
        received_single.verify_signature(),
        "Single character message signature should be valid"
    );
    let received_single_msg = received_single
        .get_message()
        .expect("Failed to deserialize single character message");
    assert_eq!(
        received_single_msg.get_payload(),
        "a",
        "Single character payload should be preserved"
    );
    println!("✓ Single character payload message handled correctly");

    // Test 3: Different message types with minimal payloads
    println!("Testing minimal Ping and Pong messages...");

    // Create minimal Ping message
    let identity = mate::crypto::Identity::generate().expect("Failed to generate identity");
    let ping_message = mate::messages::Message::new_ping(0, String::new()); // nonce 0, empty payload
    let ping_envelope =
        mate::messages::SignedEnvelope::create(&ping_message, &identity, Some(1234567890))
            .expect("Failed to create ping envelope");

    // Create minimal Pong message
    let pong_message = mate::messages::Message::new_pong(0, String::new()); // nonce 0, empty payload
    let pong_envelope =
        mate::messages::SignedEnvelope::create(&pong_message, &identity, Some(1234567890))
            .expect("Failed to create pong envelope");

    // Test minimal Ping
    let mut ping_stream = MockStream::new();
    framed_message
        .write_message(&mut ping_stream, &ping_envelope)
        .await
        .expect("Failed to write minimal ping message");

    let ping_data = ping_stream.get_written_data().to_vec();
    let mut ping_read_stream = MockStream::with_data(ping_data.clone());

    let received_ping = framed_message
        .read_message(&mut ping_read_stream)
        .await
        .expect("Failed to read minimal ping message");

    assert!(
        received_ping.verify_signature(),
        "Minimal ping signature should be valid"
    );
    let received_ping_msg = received_ping
        .get_message()
        .expect("Failed to deserialize minimal ping message");
    assert!(received_ping_msg.is_ping(), "Should be a Ping message");
    assert_eq!(received_ping_msg.get_nonce(), 0, "Ping nonce should be 0");
    assert_eq!(
        received_ping_msg.get_payload(),
        "",
        "Ping payload should be empty"
    );

    // Test minimal Pong
    let mut pong_stream = MockStream::new();
    framed_message
        .write_message(&mut pong_stream, &pong_envelope)
        .await
        .expect("Failed to write minimal pong message");

    let pong_data = pong_stream.get_written_data().to_vec();
    let mut pong_read_stream = MockStream::with_data(pong_data.clone());

    let received_pong = framed_message
        .read_message(&mut pong_read_stream)
        .await
        .expect("Failed to read minimal pong message");

    assert!(
        received_pong.verify_signature(),
        "Minimal pong signature should be valid"
    );
    let received_pong_msg = received_pong
        .get_message()
        .expect("Failed to deserialize minimal pong message");
    assert!(received_pong_msg.is_pong(), "Should be a Pong message");
    assert_eq!(received_pong_msg.get_nonce(), 0, "Pong nonce should be 0");
    assert_eq!(
        received_pong_msg.get_payload(),
        "",
        "Pong payload should be empty"
    );

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
        assert!(
            data.len() > LENGTH_PREFIX_SIZE,
            "{} should have more than {} bytes (length prefix + message)",
            description,
            LENGTH_PREFIX_SIZE
        );

        // Extract and verify length prefix
        let length_prefix = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        let actual_message_length = data.len() - LENGTH_PREFIX_SIZE;

        assert_eq!(
            length_prefix as usize, actual_message_length,
            "{} length prefix should match actual message size",
            description
        );

        // Verify message is not unreasonably small (SignedEnvelope has minimum structure)
        assert!(
            length_prefix > 0,
            "{} should have non-zero message length",
            description
        );

        println!(
            "✓ {} passed minimum size requirements (total: {} bytes, message: {} bytes)",
            description,
            data.len(),
            length_prefix
        );
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
        framed_message
            .write_message(&mut edge_stream, &edge_envelope)
            .await
            .unwrap_or_else(|_| panic!("Failed to write {} message", description));

        let edge_data = edge_stream.get_written_data().to_vec();
        let mut edge_read_stream = MockStream::with_data(edge_data);

        let received_edge = framed_message
            .read_message(&mut edge_read_stream)
            .await
            .unwrap_or_else(|_| panic!("Failed to read {} message", description));

        assert!(
            received_edge.verify_signature(),
            "{} signature should be valid",
            description
        );
        let received_edge_msg = received_edge
            .get_message()
            .unwrap_or_else(|_| panic!("Failed to deserialize {} message", description));
        assert_eq!(
            received_edge_msg.get_nonce(),
            nonce,
            "{} nonce should match",
            description
        );

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
