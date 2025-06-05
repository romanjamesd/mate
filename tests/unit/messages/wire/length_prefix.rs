//! Tests for length prefix format compliance and accuracy

use crate::common::mock_streams::*;
use crate::common::test_data::*;
use mate::messages::wire::{FramedMessage, LENGTH_PREFIX_SIZE};

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
        framed_message
            .write_message(&mut stream, &envelope)
            .await
            .expect("Failed to write message");

        let written_data = stream.get_written_data();

        // Test 1: Verify length prefix is exactly 4 bytes
        assert!(
            written_data.len() >= LENGTH_PREFIX_SIZE,
            "Written data should contain at least {} bytes for length prefix",
            LENGTH_PREFIX_SIZE
        );

        let length_prefix_bytes = &written_data[0..LENGTH_PREFIX_SIZE];
        assert_eq!(
            length_prefix_bytes.len(),
            LENGTH_PREFIX_SIZE,
            "Length prefix should be exactly {} bytes",
            LENGTH_PREFIX_SIZE
        );

        // Test 2: Verify length is encoded as big-endian u32
        let length_from_be = u32::from_be_bytes([
            length_prefix_bytes[0],
            length_prefix_bytes[1],
            length_prefix_bytes[2],
            length_prefix_bytes[3],
        ]);

        // Verify this produces a sensible length value
        let expected_message_length = written_data.len() - LENGTH_PREFIX_SIZE;
        assert_eq!(
            length_from_be as usize, expected_message_length,
            "Big-endian u32 interpretation should match actual message length"
        );

        // Test 3: Test that length prefix correctly represents message byte count
        // The length prefix should equal the number of bytes following it
        let actual_message_bytes = &written_data[LENGTH_PREFIX_SIZE..];
        assert_eq!(
            length_from_be as usize,
            actual_message_bytes.len(),
            "Length prefix should correctly represent message byte count"
        );

        // Test 4: Verify receiver can parse length prefix correctly
        // Create a reader with the written data and verify we can read the length prefix
        let mut read_stream = MockStream::with_data(written_data.to_vec());

        // Manually read the length prefix to verify parsing
        let mut length_buffer = [0u8; LENGTH_PREFIX_SIZE];
        tokio::io::AsyncReadExt::read_exact(&mut read_stream, &mut length_buffer)
            .await
            .expect("Should be able to read length prefix");

        let parsed_length = u32::from_be_bytes(length_buffer);
        assert_eq!(
            parsed_length, length_from_be,
            "Receiver should parse length prefix correctly"
        );

        // Verify the receiver can use this length to read the exact message
        let mut message_buffer = vec![0u8; parsed_length as usize];
        tokio::io::AsyncReadExt::read_exact(&mut read_stream, &mut message_buffer)
            .await
            .expect("Should be able to read message using parsed length");

        assert_eq!(
            message_buffer.len(),
            parsed_length as usize,
            "Should read exactly the number of bytes specified by length prefix"
        );
        assert_eq!(
            message_buffer, actual_message_bytes,
            "Read message bytes should match original message bytes"
        );

        // Additional verification: Test endianness by checking individual bytes
        let length_as_bytes = (expected_message_length as u32).to_be_bytes();
        assert_eq!(
            length_prefix_bytes[0], length_as_bytes[0],
            "Most significant byte should match"
        );
        assert_eq!(
            length_prefix_bytes[1], length_as_bytes[1],
            "Second byte should match"
        );
        assert_eq!(
            length_prefix_bytes[2], length_as_bytes[2],
            "Third byte should match"
        );
        assert_eq!(
            length_prefix_bytes[3], length_as_bytes[3],
            "Least significant byte should match"
        );

        println!(
            "✓ Length prefix format verified for {} (prefix: {} bytes, message: {} bytes)",
            description, LENGTH_PREFIX_SIZE, parsed_length
        );
    }

    // Test edge case: Verify format consistency across size boundaries
    println!("Testing format consistency across size boundaries...");

    let boundary_sizes = vec![1, 255, 256, 65535, 65536];
    for size in boundary_sizes {
        let payload = "x".repeat(size);
        let (envelope, _) = create_test_envelope(&payload);

        let mut stream = MockStream::new();
        framed_message
            .write_message(&mut stream, &envelope)
            .await
            .expect("Failed to write boundary size message");

        let written_data = stream.get_written_data();

        // Verify format is consistent regardless of message size
        assert_eq!(
            written_data[0..LENGTH_PREFIX_SIZE].len(),
            LENGTH_PREFIX_SIZE,
            "Length prefix should always be {} bytes regardless of message size",
            LENGTH_PREFIX_SIZE
        );

        let length_prefix = u32::from_be_bytes([
            written_data[0],
            written_data[1],
            written_data[2],
            written_data[3],
        ]);

        let actual_message_size = written_data.len() - LENGTH_PREFIX_SIZE;
        assert_eq!(
            length_prefix as usize, actual_message_size,
            "Length prefix format should be consistent for {} byte payload",
            size
        );
    }

    println!("✓ Length prefix format compliance test passed!");
    println!(
        "  - Length prefix is always exactly {} bytes",
        LENGTH_PREFIX_SIZE
    );
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
        let serialized_envelope =
            bincode::serialize(&envelope).expect("Failed to serialize test envelope");
        let expected_serialized_size = serialized_envelope.len();

        println!("  Payload size: {} bytes", *size);
        println!(
            "  Serialized envelope size: {} bytes",
            expected_serialized_size
        );

        // Write the message using the wire protocol
        let mut stream = MockStream::new();
        framed_message
            .write_message(&mut stream, &envelope)
            .await
            .expect(&format!("Failed to write {} message", description));

        let written_data = stream.get_written_data();

        // Extract and verify length prefix
        assert!(
            written_data.len() >= LENGTH_PREFIX_SIZE,
            "Written data should contain at least the length prefix for {}",
            description
        );

        let length_prefix = u32::from_be_bytes([
            written_data[0],
            written_data[1],
            written_data[2],
            written_data[3],
        ]);

        // The length prefix should match the serialized message size exactly
        assert_eq!(
            length_prefix as usize, expected_serialized_size,
            "Length prefix ({}) should match actual serialized message size ({}) for {}",
            length_prefix, expected_serialized_size, description
        );

        // Verify the total written data size is length prefix + message data
        let actual_message_size = written_data.len() - LENGTH_PREFIX_SIZE;
        assert_eq!(
            actual_message_size, expected_serialized_size,
            "Actual written message size ({}) should match expected serialized size ({}) for {}",
            actual_message_size, expected_serialized_size, description
        );

        // Additional verification: ensure we can read the message back correctly
        let mut read_stream = MockStream::with_data(written_data.to_vec());
        let received_envelope = framed_message
            .read_message(&mut read_stream)
            .await
            .expect(&format!("Failed to read back {} message", description));

        // Verify the received message has the same payload size
        let received_message = received_envelope
            .get_message()
            .expect("Failed to deserialize received message");
        assert_eq!(
            received_message.get_payload().len(),
            *size,
            "Received message payload size should match original for {}",
            description
        );

        // Verify content integrity
        assert_eq!(
            received_message.get_payload(),
            payload,
            "Received message payload should match original content for {}",
            description
        );

        println!(
            "  ✓ Length prefix accuracy verified: {} bytes",
            length_prefix
        );
        println!("  ✓ Round-trip successful for {}", description);
    }

    println!("✓ All length prefix accuracy tests passed!");
    println!("  - Tested {} different message sizes", test_cases.len());
    println!("  - Verified length prefixes match actual serialized message sizes");
    println!("  - Tested messages at size boundaries (1KB, 1MB, etc.)");
    println!("  - All messages successfully round-tripped through wire protocol");
}
