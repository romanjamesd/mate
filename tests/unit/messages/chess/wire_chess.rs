//! Tests for chess-specific wire protocol configurations and handling

use crate::common::mock_streams::*;
use crate::common::test_data::*;
use mate::chess::{Board, Color};
use mate::messages::chess::{generate_game_id, GameInvite, Move, SyncResponse};
use mate::messages::types::Message;
use mate::messages::wire::{
    FramedMessage, WireConfig, WireProtocolError, MAX_MESSAGE_SIZE, NETWORK_DEFAULT_MESSAGE_SIZE,
    NETWORK_LARGE_MESSAGE_SIZE, NETWORK_SMALL_MESSAGE_SIZE,
};
use std::time::Duration;

// =============================================================================
// Chess Wire Configuration Tests
// =============================================================================

#[tokio::test]
async fn test_chess_standard_configuration() {
    println!("Testing chess standard wire configuration");

    let config = WireConfig::for_chess_standard();
    let framed_message = FramedMessage::new(config.clone());

    // Verify configuration values are appropriate for standard chess messages
    assert_eq!(
        config.max_message_size, NETWORK_SMALL_MESSAGE_SIZE,
        "Standard chess config should use small message size limit (64KB)"
    );
    assert_eq!(
        config.read_timeout,
        Duration::from_secs(15),
        "Standard chess config should have 15-second read timeout"
    );
    assert_eq!(
        config.write_timeout,
        Duration::from_secs(15),
        "Standard chess config should have 15-second write timeout"
    );

    // Test with typical chess messages
    let game_id = generate_game_id();
    let invite = GameInvite::new(game_id.clone(), Some(Color::White));
    let message = Message::GameInvite(invite);
    let (envelope, _) = create_test_envelope_with_message(&message);

    // Should handle standard chess messages efficiently
    let mut stream = MockStream::new();
    let result = framed_message.write_message(&mut stream, &envelope).await;
    assert!(result.is_ok(), "Should handle standard chess messages");

    println!("✓ Chess standard configuration test passed");
}

#[tokio::test]
async fn test_chess_sync_configuration() {
    println!("Testing chess sync wire configuration");

    let config = WireConfig::for_chess_sync();
    let framed_message = FramedMessage::new(config.clone());

    // Verify configuration values are appropriate for sync messages
    assert_eq!(
        config.max_message_size, NETWORK_LARGE_MESSAGE_SIZE,
        "Sync chess config should use large message size limit (8MB)"
    );
    assert_eq!(
        config.read_timeout,
        Duration::from_secs(60),
        "Sync chess config should have 60-second read timeout"
    );
    assert_eq!(
        config.write_timeout,
        Duration::from_secs(60),
        "Sync chess config should have 60-second write timeout"
    );

    // Test with large sync response message
    let game_id = generate_game_id();
    let board = Board::new();
    let large_move_history: Vec<String> = (0..500)
        .map(|i| format!("e{move_one}{move_two}", move_one = 2 + (i % 6), move_two = 4 + (i % 4)))
        .collect();
    let sync_response = SyncResponse::new(
        game_id,
        board.to_fen(),
        large_move_history,
        "sample_hash".to_string(),
    );
    let message = Message::SyncResponse(sync_response);
    let (envelope, _) = create_test_envelope_with_message(&message);

    // Should handle large sync messages
    let mut stream = MockStream::new();
    let result = framed_message.write_message(&mut stream, &envelope).await;
    assert!(result.is_ok(), "Should handle large sync messages");

    println!("✓ Chess sync configuration test passed");
}

#[tokio::test]
async fn test_chess_bulk_configuration() {
    println!("Testing chess bulk wire configuration");

    let config = WireConfig::for_chess_bulk();
    let framed_message = FramedMessage::new(config.clone());

    // Verify configuration values are appropriate for bulk operations
    assert_eq!(
        config.max_message_size, MAX_MESSAGE_SIZE,
        "Bulk chess config should use maximum message size limit (16MB)"
    );
    assert_eq!(
        config.read_timeout,
        Duration::from_secs(120),
        "Bulk chess config should have 120-second read timeout"
    );
    assert_eq!(
        config.write_timeout,
        Duration::from_secs(120),
        "Bulk chess config should have 120-second write timeout"
    );

    // Test configuration allows maximum size operations
    let result = framed_message.test_validate_length(MAX_MESSAGE_SIZE as u32);
    assert!(result.is_ok(), "Should allow maximum message size");

    println!("✓ Chess bulk configuration test passed");
}

#[tokio::test]
async fn test_chess_realtime_configuration() {
    println!("Testing chess realtime wire configuration");

    let config = WireConfig::for_chess_realtime();
    let framed_message = FramedMessage::new(config.clone());

    // Verify configuration values are appropriate for real-time play
    assert_eq!(
        config.max_message_size, NETWORK_DEFAULT_MESSAGE_SIZE,
        "Realtime chess config should use default message size limit (1MB)"
    );
    assert_eq!(
        config.read_timeout,
        Duration::from_secs(10),
        "Realtime chess config should have 10-second read timeout"
    );
    assert_eq!(
        config.write_timeout,
        Duration::from_secs(10),
        "Realtime chess config should have 10-second write timeout"
    );

    // Test with move message for real-time responsiveness
    let game_id = generate_game_id();
    let chess_move = Move::new(game_id, "e2e4".to_string(), "sample_hash".to_string());
    let message = Message::Move(chess_move);
    let (envelope, _) = create_test_envelope_with_message(&message);

    let mut stream = MockStream::new();
    let result = framed_message.write_message(&mut stream, &envelope).await;
    assert!(result.is_ok(), "Should handle real-time move messages");

    println!("✓ Chess realtime configuration test passed");
}

// =============================================================================
// Message Size Handling Tests
// =============================================================================

#[tokio::test]
async fn test_chess_standard_message_size_limits() {
    println!("Testing chess standard configuration message size limits");

    let framed_message = FramedMessage::for_chess_standard();

    // Test that it rejects messages exceeding 64KB limit
    let oversized_length = NETWORK_SMALL_MESSAGE_SIZE + 1;
    let result = framed_message.test_validate_length(oversized_length as u32);

    assert!(
        result.is_err(),
        "Should reject messages exceeding 64KB limit"
    );

    if let Err(error) = result {
        match error {
            WireProtocolError::InvalidLength { length, max, .. } => {
                assert_eq!(length, oversized_length as u32);
                assert_eq!(max, NETWORK_SMALL_MESSAGE_SIZE as u32);
            }
            WireProtocolError::MessageTooLarge { size, max_size } => {
                assert_eq!(size, oversized_length);
                assert_eq!(max_size, NETWORK_SMALL_MESSAGE_SIZE);
            }
            _ => panic!(
                "Expected InvalidLength or MessageTooLarge error for oversized message, got: {:?}",
                error
            ),
        }
    }

    // Test that it accepts messages within the limit
    let valid_length = NETWORK_SMALL_MESSAGE_SIZE - 1000;
    let result = framed_message.test_validate_length(valid_length as u32);
    assert!(result.is_ok(), "Should accept messages within 64KB limit");

    println!("✓ Chess standard message size limits test passed");
}

#[tokio::test]
async fn test_chess_sync_message_size_limits() {
    println!("Testing chess sync configuration message size limits");

    let framed_message = FramedMessage::for_chess_sync();

    // Test that it rejects messages exceeding 8MB limit
    let oversized_length = NETWORK_LARGE_MESSAGE_SIZE + 1;
    let result = framed_message.test_validate_length(oversized_length as u32);

    assert!(
        result.is_err(),
        "Should reject messages exceeding 8MB limit"
    );

    // Test that it accepts large sync messages within the limit
    let large_valid_length = NETWORK_LARGE_MESSAGE_SIZE - 1000;
    let result = framed_message.test_validate_length(large_valid_length as u32);
    assert!(
        result.is_ok(),
        "Should accept large sync messages within 8MB limit"
    );

    println!("✓ Chess sync message size limits test passed");
}

#[tokio::test]
async fn test_chess_bulk_maximum_size_handling() {
    println!("Testing chess bulk configuration maximum size handling");

    let framed_message = FramedMessage::for_chess_bulk();

    // Test that it accepts maximum possible message size
    let max_length = MAX_MESSAGE_SIZE;
    let result = framed_message.test_validate_length(max_length as u32);
    assert!(result.is_ok(), "Should accept maximum message size (16MB)");

    // Test memory allocation at maximum size
    let alloc_result = framed_message.test_safe_allocate(max_length);
    assert!(
        alloc_result.is_ok(),
        "Should safely allocate maximum message size"
    );

    println!("✓ Chess bulk maximum size handling test passed");
}

// =============================================================================
// Timeout Configuration Tests
// =============================================================================

#[tokio::test]
async fn test_chess_configuration_timeout_appropriateness() {
    println!("Testing timeout appropriateness across chess configurations");

    let standard_config = WireConfig::for_chess_standard();
    let sync_config = WireConfig::for_chess_sync();
    let bulk_config = WireConfig::for_chess_bulk();
    let realtime_config = WireConfig::for_chess_realtime();

    // Verify timeout hierarchy: realtime < standard < sync < bulk
    assert!(
        realtime_config.read_timeout < standard_config.read_timeout,
        "Real-time should have shorter timeout than standard"
    );
    assert!(
        standard_config.read_timeout < sync_config.read_timeout,
        "Standard should have shorter timeout than sync"
    );
    assert!(
        sync_config.read_timeout < bulk_config.read_timeout,
        "Sync should have shorter timeout than bulk"
    );

    // Verify same pattern for write timeouts
    assert!(
        realtime_config.write_timeout < standard_config.write_timeout,
        "Real-time write should be faster than standard"
    );
    assert!(
        standard_config.write_timeout < sync_config.write_timeout,
        "Standard write should be faster than sync"
    );
    assert!(
        sync_config.write_timeout < bulk_config.write_timeout,
        "Sync write should be faster than bulk"
    );

    println!("✓ Chess configuration timeout appropriateness test passed");
}

#[tokio::test]
async fn test_chess_realtime_timeout_responsiveness() {
    println!("Testing chess real-time configuration timeout responsiveness");

    let config = WireConfig::for_chess_realtime();

    // Real-time timeouts should be aggressive for responsive gameplay
    assert!(
        config.read_timeout <= Duration::from_secs(10),
        "Real-time read timeout should be 10 seconds or less"
    );
    assert!(
        config.write_timeout <= Duration::from_secs(10),
        "Real-time write timeout should be 10 seconds or less"
    );

    // Verify timeouts are balanced (not too short to cause false timeouts)
    assert!(
        config.read_timeout >= Duration::from_secs(5),
        "Real-time read timeout should be at least 5 seconds"
    );
    assert!(
        config.write_timeout >= Duration::from_secs(5),
        "Real-time write timeout should be at least 5 seconds"
    );

    println!("✓ Chess real-time timeout responsiveness test passed");
}

// =============================================================================
// DoS Protection Tests
// =============================================================================

#[tokio::test]
async fn test_chess_dos_protection_integration() {
    println!("Testing DoS protection integration with chess configurations");

    // Test each chess configuration maintains DoS protection
    let configs = vec![
        ("standard", FramedMessage::for_chess_standard()),
        ("sync", FramedMessage::for_chess_sync()),
        ("bulk", FramedMessage::for_chess_bulk()),
        ("realtime", FramedMessage::for_chess_realtime()),
    ];

    for (config_name, framed_message) in configs {
        println!("Testing DoS protection for {} configuration", config_name);

        // Test that each configuration rejects extremely oversized allocations
        // Use a size much larger than MAX_MESSAGE_SIZE to trigger DoS protection
        let malicious_size = MAX_MESSAGE_SIZE + 1000000; // Well over 16MB
        let dos_result = framed_message.test_safe_allocate(malicious_size);

        assert!(
            dos_result.is_err(),
            "{} config should reject oversized allocations",
            config_name
        );

        // Verify the error is appropriate for DoS protection
        if let Err(error) = dos_result {
            match error {
                WireProtocolError::AllocationDenied { size, limit } => {
                    assert_eq!(size, malicious_size);
                    assert_eq!(limit, MAX_MESSAGE_SIZE); // DoS config uses MAX_MESSAGE_SIZE as limit
                }
                _ => panic!(
                    "Expected AllocationDenied error for oversized allocation, got: {:?}",
                    error
                ),
            }
        }
    }

    println!("✓ Chess DoS protection integration test passed");
}

#[tokio::test]
async fn test_chess_configuration_boundary_security() {
    println!("Testing security at chess configuration boundaries");

    let standard_framed = FramedMessage::for_chess_standard();
    let sync_framed = FramedMessage::for_chess_sync();

    // Test that standard config rejects sync-sized messages (security boundary)
    let sync_sized_message = NETWORK_LARGE_MESSAGE_SIZE - 1000; // Just under sync limit

    let standard_result = standard_framed.test_validate_length(sync_sized_message as u32);
    assert!(
        standard_result.is_err(),
        "Standard config should reject sync-sized messages for security"
    );

    let sync_result = sync_framed.test_validate_length(sync_sized_message as u32);
    assert!(
        sync_result.is_ok(),
        "Sync config should accept sync-sized messages"
    );

    println!("✓ Chess configuration boundary security test passed");
}

// =============================================================================
// Performance Characteristics Tests
// =============================================================================

#[tokio::test]
async fn test_chess_configuration_performance_characteristics() {
    println!("Testing performance characteristics of chess configurations");

    // Create test message for performance comparison
    let game_id = generate_game_id();
    let move_msg = Move::new(game_id, "e2e4".to_string(), "hash".to_string());
    let message = Message::Move(move_msg);
    let (envelope, _) = create_test_envelope_with_message(&message);

    let configs = vec![
        ("realtime", FramedMessage::for_chess_realtime()),
        ("standard", FramedMessage::for_chess_standard()),
    ];

    for (config_name, framed_message) in configs {
        println!("Testing {} configuration performance", config_name);

        // Test serialization performance (should complete quickly)
        let start = std::time::Instant::now();
        let mut stream = MockStream::new();
        let result = framed_message.write_message(&mut stream, &envelope).await;
        let duration = start.elapsed();

        assert!(
            result.is_ok(),
            "{} config should handle messages",
            config_name
        );
        assert!(
            duration < Duration::from_millis(100),
            "{} config should serialize quickly (under 100ms), took {:?}",
            config_name,
            duration
        );
    }

    println!("✓ Chess configuration performance characteristics test passed");
}

#[tokio::test]
async fn test_chess_wire_efficiency_with_chess_messages() {
    println!("Testing wire protocol efficiency with different chess message types");

    let framed_message = FramedMessage::for_chess_standard();
    let game_id = generate_game_id();

    // Test efficiency with different chess message types
    let messages = vec![
        (
            "invite",
            Message::GameInvite(GameInvite::new(game_id.clone(), Some(Color::White))),
        ),
        (
            "move",
            Message::Move(Move::new(
                game_id.clone(),
                "e2e4".to_string(),
                "hash".to_string(),
            )),
        ),
    ];

    for (msg_type, message) in messages {
        let (envelope, _) = create_test_envelope_with_message(&message);

        let mut write_stream = MockStream::new();
        let write_result = framed_message
            .write_message(&mut write_stream, &envelope)
            .await;
        assert!(
            write_result.is_ok(),
            "Should efficiently write {} message",
            msg_type
        );

        let written_data = write_stream.get_written_data().to_vec();

        // Verify wire format efficiency (reasonable overhead)
        let message_size = written_data.len();
        assert!(
            message_size < 1024, // Should be under 1KB for simple chess messages
            "{} message should be efficiently encoded (under 1KB), got {} bytes",
            msg_type,
            message_size
        );

        // Test efficient read-back
        let mut read_stream = MockStream::with_data(written_data);
        let read_result = framed_message.read_message(&mut read_stream).await;
        assert!(
            read_result.is_ok(),
            "Should efficiently read {} message",
            msg_type
        );
    }

    println!("✓ Chess wire efficiency test passed");
}
