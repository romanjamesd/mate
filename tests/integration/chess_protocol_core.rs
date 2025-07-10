//! Chess Protocol Core Integration Tests
//!
//! Tests core chess protocol functionality end-to-end as specified in tests-to-add.md:
//! - Game Flow Integration: Complete game invitation, acceptance, move exchange workflows
//! - Wire Protocol Integration: Chess messages over wire protocol, size handling
//! - Error Handling Integration: Error propagation through protocol stack
//! - Backward Compatibility: Chess messages coexisting with ping/pong protocol
//!
//! Key Focus Areas:
//! - Real-world game scenario simulation
//! - Protocol robustness under various conditions
//! - Compatibility with existing infrastructure
//! - Performance in realistic usage patterns

use anyhow::Result;
use mate::chess::{Board, Color};
use mate::crypto::Identity;
use mate::messages::chess::{generate_game_id, hash_board_state, ChessProtocolError};
use mate::messages::types::{Message, SignedEnvelope};
use mate::messages::wire::{FramedMessage, WireConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::duplex;
use tokio::time::timeout;

use crate::common::mock_streams::MockStream;

/// Mock game state for tracking game progression
#[derive(Debug, Clone)]
struct MockGameState {
    #[allow(dead_code)]
    game_id: String,
    board: Board,
    move_history: Vec<String>,
    #[allow(dead_code)]
    white_player: String,
    #[allow(dead_code)]
    black_player: String,
    current_turn: Color,
}

impl MockGameState {
    fn new_with_players(game_id: String, white_player: String, black_player: String) -> Self {
        Self {
            game_id,
            board: Board::new(),
            move_history: Vec::new(),
            white_player,
            black_player,
            current_turn: Color::White,
        }
    }

    fn apply_move(&mut self, chess_move: &str) -> Result<()> {
        // Simulate move application (simplified for testing)
        self.move_history.push(chess_move.to_string());
        self.current_turn = match self.current_turn {
            Color::White => Color::Black,
            Color::Black => Color::White,
        };
        Ok(())
    }

    fn get_board_hash(&self) -> String {
        hash_board_state(&self.board)
    }
}

/// Test Game Flow Integration: Complete game invitation, acceptance, move exchange workflows
#[tokio::test]
async fn test_complete_game_flow_integration() -> Result<()> {
    println!("Testing complete game flow integration...");

    // Setup identities for two players
    let player1_identity = Arc::new(Identity::generate()?);
    let player2_identity = Arc::new(Identity::generate()?);
    let player1_id = player1_identity.peer_id().to_string();
    let player2_id = player2_identity.peer_id().to_string();

    // Setup wire protocol with chess-optimized configuration
    let wire_config = WireConfig::for_chess_standard();
    let framed_message = FramedMessage::new(wire_config);

    // Create bidirectional communication channels
    let (player1_stream, player2_stream) = duplex(8192);
    let (mut p1_read, mut p1_write) = tokio::io::split(player1_stream);
    let (mut p2_read, mut p2_write) = tokio::io::split(player2_stream);

    // Phase 1: Game Invitation
    let game_id = generate_game_id();
    let invite_msg = Message::new_game_invite(game_id.clone(), Some(Color::White));
    let invite_envelope = SignedEnvelope::create(&invite_msg, &player1_identity, None)?;

    // Player 1 sends invitation
    framed_message
        .write_message(&mut p1_write, &invite_envelope)
        .await?;

    // Player 2 receives invitation
    let received_envelope = framed_message.read_message(&mut p2_read).await?;
    let received_message = received_envelope.get_message()?;

    assert!(matches!(received_message, Message::GameInvite(_)));
    if let Message::GameInvite(invite) = received_message {
        assert_eq!(invite.game_id, game_id);
        assert_eq!(invite.suggested_color, Some(Color::White));
    }

    // Phase 2: Game Acceptance
    let accept_msg = Message::new_game_accept(game_id.clone(), Color::Black);
    let accept_envelope = SignedEnvelope::create(&accept_msg, &player2_identity, None)?;

    // Player 2 sends acceptance
    framed_message
        .write_message(&mut p2_write, &accept_envelope)
        .await?;

    // Player 1 receives acceptance
    let received_envelope = framed_message.read_message(&mut p1_read).await?;
    let received_message = received_envelope.get_message()?;

    assert!(matches!(received_message, Message::GameAccept(_)));
    if let Message::GameAccept(accept) = received_message {
        assert_eq!(accept.game_id, game_id);
        assert_eq!(accept.accepted_color, Color::Black);
    }

    // Initialize game state
    let mut game_state = MockGameState::new_with_players(game_id.clone(), player1_id, player2_id);

    // Phase 3: Move Exchange
    let moves = ["e2e4", "e7e5", "Nf3", "Nc6"];

    for (move_index, chess_move) in moves.iter().enumerate() {
        let current_player_identity = if move_index % 2 == 0 {
            &player1_identity
        } else {
            &player2_identity
        };

        // Apply move to game state
        game_state.apply_move(chess_move)?;
        let board_hash = game_state.get_board_hash();

        // Create and send move message
        let move_msg = Message::new_move(game_id.clone(), chess_move.to_string(), board_hash);
        let move_envelope = SignedEnvelope::create(&move_msg, current_player_identity, None)?;

        if move_index % 2 == 0 {
            // Player 1's move
            framed_message
                .write_message(&mut p1_write, &move_envelope)
                .await?;

            // Player 2 receives and acknowledges
            let received_envelope = framed_message.read_message(&mut p2_read).await?;
            let received_message = received_envelope.get_message()?;

            assert!(matches!(received_message, Message::Move(_)));
            if let Message::Move(mv) = received_message {
                assert_eq!(mv.game_id, game_id);
                assert_eq!(mv.chess_move, *chess_move);
            }

            // Send acknowledgment
            let ack_msg = Message::new_move_ack(game_id.clone(), None);
            let ack_envelope = SignedEnvelope::create(&ack_msg, &player2_identity, None)?;
            framed_message
                .write_message(&mut p2_write, &ack_envelope)
                .await?;

            // Player 1 receives acknowledgment
            let ack_envelope = framed_message.read_message(&mut p1_read).await?;
            let ack_message = ack_envelope.get_message()?;
            assert!(matches!(ack_message, Message::MoveAck(_)));
        } else {
            // Player 2's move
            framed_message
                .write_message(&mut p2_write, &move_envelope)
                .await?;

            // Player 1 receives and acknowledges
            let received_envelope = framed_message.read_message(&mut p1_read).await?;
            let received_message = received_envelope.get_message()?;

            assert!(matches!(received_message, Message::Move(_)));

            // Send acknowledgment
            let ack_msg = Message::new_move_ack(game_id.clone(), None);
            let ack_envelope = SignedEnvelope::create(&ack_msg, &player1_identity, None)?;
            framed_message
                .write_message(&mut p1_write, &ack_envelope)
                .await?;

            // Player 2 receives acknowledgment
            let ack_envelope = framed_message.read_message(&mut p2_read).await?;
            let ack_message = ack_envelope.get_message()?;
            assert!(matches!(ack_message, Message::MoveAck(_)));
        }
    }

    println!("✅ Complete game flow integration test passed");
    println!("   - Game invitation sent and accepted successfully");
    println!("   - {} moves exchanged with acknowledgments", moves.len());
    println!("   - All message types validated correctly");

    Ok(())
}

/// Test Wire Protocol Integration: Chess messages over wire protocol, size handling
#[tokio::test]
async fn test_wire_protocol_integration_with_chess_messages() -> Result<()> {
    println!("Testing wire protocol integration with chess messages...");

    let identity = Arc::new(Identity::generate()?);
    let game_id = generate_game_id();

    // Test different wire configurations for chess messages
    let test_configs = vec![
        ("standard", WireConfig::for_chess_standard()),
        ("sync", WireConfig::for_chess_sync()),
        ("bulk", WireConfig::for_chess_bulk()),
        ("realtime", WireConfig::for_chess_realtime()),
    ];

    for (config_name, wire_config) in test_configs {
        println!("  Testing {} configuration...", config_name);

        let framed_message = FramedMessage::new(wire_config.clone());

        // Test various chess message types
        let test_messages = vec![
            Message::new_game_invite(game_id.clone(), Some(Color::White)),
            Message::new_game_accept(game_id.clone(), Color::Black),
            Message::new_move(
                game_id.clone(),
                "e2e4".to_string(),
                hash_board_state(&Board::new()),
            ),
            Message::new_move_ack(game_id.clone(), Some("move-123".to_string())),
            Message::new_sync_request(game_id.clone()),
            Message::new_sync_response(
                game_id.clone(),
                Board::new().to_fen(),
                vec!["e2e4".to_string(), "e7e5".to_string()],
                hash_board_state(&Board::new()),
            ),
        ];

        for message in test_messages {
            let envelope = SignedEnvelope::create(&message, &identity, None)?;

            // Test message size validation
            let serialized_size =
                envelope.message.len() + envelope.signature.len() + envelope.sender.len() + 8; // timestamp

            assert!(
                serialized_size <= wire_config.max_message_size,
                "Message size {} exceeds limit {} for config {}",
                serialized_size,
                wire_config.max_message_size,
                config_name
            );

            // Test write/read roundtrip using separate streams for each message
            let mut write_stream = MockStream::new();
            framed_message
                .write_message(&mut write_stream, &envelope)
                .await?;

            let written_data = write_stream.get_written_data().to_vec();
            let mut read_stream = MockStream::with_data(written_data);

            let received_envelope = framed_message.read_message(&mut read_stream).await?;
            let received_message = received_envelope.get_message()?;

            // Verify message integrity
            assert_eq!(message.message_type(), received_message.message_type());
            if let Some(original_game_id) = message.get_game_id() {
                assert_eq!(Some(original_game_id), received_message.get_game_id());
            }
        }
    }

    // Test large sync response handling
    println!("  Testing large sync response handling...");
    let large_move_history: Vec<String> = (1..=500).map(|i| format!("move_{i}")).collect();

    let large_sync_msg = Message::new_sync_response(
        game_id.clone(),
        Board::new().to_fen(),
        large_move_history,
        hash_board_state(&Board::new()),
    );

    let large_envelope = SignedEnvelope::create(&large_sync_msg, &identity, None)?;
    let sync_framed = FramedMessage::new(WireConfig::for_chess_sync());
    let mut sync_stream = MockStream::new();

    sync_framed
        .write_message(&mut sync_stream, &large_envelope)
        .await?;

    let written_data = sync_stream.get_written_data().to_vec();
    let mut read_stream = MockStream::with_data(written_data);

    let received_envelope = sync_framed.read_message(&mut read_stream).await?;
    let received_message = received_envelope.get_message()?;

    assert!(matches!(received_message, Message::SyncResponse(_)));
    if let Message::SyncResponse(sync_resp) = received_message {
        assert_eq!(sync_resp.move_history.len(), 500);
    }

    println!("✅ Wire protocol integration test passed");
    println!("   - All configuration types handled chess messages correctly");
    println!("   - Large sync responses processed successfully");
    println!("   - Message size validation working properly");

    Ok(())
}

/// Test Error Handling Integration: Error propagation through protocol stack
#[tokio::test]
async fn test_error_handling_integration() -> Result<()> {
    println!("Testing error handling integration...");

    let framed_message = FramedMessage::new(WireConfig::for_chess_standard());

    // Test invalid game ID propagation
    let invalid_game_id = "not-a-valid-uuid";
    let invalid_invite = Message::new_game_invite(invalid_game_id.to_string(), Some(Color::White));

    // Validation should catch the error
    let validation_result = invalid_invite.validate();
    assert!(validation_result.is_err());

    // Test board hash mismatch error
    let game_id = generate_game_id();
    let incorrect_hash = "invalid_hash_value";

    let move_with_bad_hash = Message::new_move(
        game_id.clone(),
        "e2e4".to_string(),
        incorrect_hash.to_string(),
    );

    // Should fail validation
    let validation_result = move_with_bad_hash.validate();
    assert!(validation_result.is_err());

    // Test error propagation through chess protocol
    let chess_protocol_result = mate::messages::chess::validate_game_id_graceful(invalid_game_id);
    assert!(chess_protocol_result.is_err());

    if let Err(chess_error) = chess_protocol_result {
        assert!(matches!(chess_error, ChessProtocolError::Validation(_)));
        assert!(!chess_error.is_recoverable()); // Invalid game ID is not recoverable
    }

    // Test timeout error integration
    let (mut read_stream, _write_stream) = duplex(1024);

    let timeout_result = timeout(
        Duration::from_millis(10),
        framed_message.read_message(&mut read_stream),
    )
    .await;

    assert!(timeout_result.is_err()); // Should timeout

    println!("✅ Error handling integration test passed");
    println!("   - Invalid game ID errors caught and propagated");
    println!("   - Board hash validation errors handled correctly");
    println!("   - Timeout errors integrated properly");

    Ok(())
}

/// Test Backward Compatibility: Chess messages coexisting with ping/pong protocol
#[tokio::test]
async fn test_backward_compatibility_with_existing_protocol() -> Result<()> {
    println!("Testing backward compatibility with existing protocol...");

    let identity = Arc::new(Identity::generate()?);
    let framed_message = FramedMessage::new(WireConfig::for_network());
    let (stream1, stream2) = duplex(8192);
    let (_read1, mut write1) = tokio::io::split(stream1);
    let (mut read2, _write2) = tokio::io::split(stream2);

    // Test mixed message sequence: ping/pong + chess messages
    let game_id = generate_game_id();
    let mixed_messages = vec![
        Message::new_ping(1, "Hello".to_string()),
        Message::new_game_invite(game_id.clone(), Some(Color::White)),
        Message::new_pong(1, "Hello".to_string()),
        Message::new_game_accept(game_id.clone(), Color::Black),
        Message::new_ping(2, "Still alive".to_string()),
        Message::new_move(
            game_id.clone(),
            "e2e4".to_string(),
            hash_board_state(&Board::new()),
        ),
        Message::new_pong(2, "Still alive".to_string()),
    ];

    // Send all messages
    for message in &mixed_messages {
        let envelope = SignedEnvelope::create(message, &identity, None)?;
        framed_message.write_message(&mut write1, &envelope).await?;
    }

    // Receive and verify all messages maintain their types and integrity
    for (index, expected_message) in mixed_messages.iter().enumerate() {
        let received_envelope = framed_message.read_message(&mut read2).await?;
        let received_message = received_envelope.get_message()?;

        assert_eq!(
            expected_message.message_type(),
            received_message.message_type(),
            "Message {} type mismatch",
            index
        );

        // Verify specific message properties
        match (expected_message, &received_message) {
            (
                Message::Ping {
                    nonce: n1,
                    payload: p1,
                },
                Message::Ping {
                    nonce: n2,
                    payload: p2,
                },
            ) => {
                assert_eq!(n1, n2);
                assert_eq!(p1, p2);
            }
            (
                Message::Pong {
                    nonce: n1,
                    payload: p1,
                },
                Message::Pong {
                    nonce: n2,
                    payload: p2,
                },
            ) => {
                assert_eq!(n1, n2);
                assert_eq!(p1, p2);
            }
            (Message::GameInvite(invite1), Message::GameInvite(invite2)) => {
                assert_eq!(invite1.game_id, invite2.game_id);
                assert_eq!(invite1.suggested_color, invite2.suggested_color);
            }
            (Message::GameAccept(accept1), Message::GameAccept(accept2)) => {
                assert_eq!(accept1.game_id, accept2.game_id);
                assert_eq!(accept1.accepted_color, accept2.accepted_color);
            }
            (Message::Move(move1), Message::Move(move2)) => {
                assert_eq!(move1.game_id, move2.game_id);
                assert_eq!(move1.chess_move, move2.chess_move);
                assert_eq!(move1.board_state_hash, move2.board_state_hash);
            }
            _ => panic!("Unexpected message type combination"),
        }
    }

    // Test message type detection utilities
    let test_message_types = vec![
        (Message::new_ping(1, "test".to_string()), false, true, false),
        (Message::new_pong(1, "test".to_string()), false, false, true),
        (
            Message::new_game_invite(game_id.clone(), None),
            true,
            false,
            false,
        ),
        (
            Message::new_move(game_id.clone(), "e2e4".to_string(), "hash".to_string()),
            true,
            false,
            false,
        ),
    ];

    for (message, should_be_chess, should_be_ping, should_be_pong) in test_message_types {
        assert_eq!(message.is_chess_message(), should_be_chess);
        assert_eq!(message.is_ping(), should_be_ping);
        assert_eq!(message.is_pong(), should_be_pong);
    }

    println!("✅ Backward compatibility test passed");
    println!("   - Mixed ping/pong and chess messages processed correctly");
    println!("   - Message type detection utilities work properly");
    println!("   - All message types maintain integrity in mixed scenarios");

    Ok(())
}

/// Test performance characteristics in realistic usage patterns
#[tokio::test]
async fn test_realistic_usage_performance() -> Result<()> {
    println!("Testing realistic usage performance...");

    let player1_identity = Arc::new(Identity::generate()?);
    let player2_identity = Arc::new(Identity::generate()?);
    let wire_config = WireConfig::for_chess_realtime();

    // Simulate rapid move exchange (blitz game scenario)
    let start_time = std::time::Instant::now();
    let (stream1, stream2) = duplex(16384);
    let (mut read1, mut write1) = tokio::io::split(stream1);
    let (mut read2, mut write2) = tokio::io::split(stream2);

    let game_id = generate_game_id();
    let move_count = 50; // Simulate 25 moves per player

    // Concurrent move processing
    let send_task = tokio::spawn(async move {
        let framed_message = FramedMessage::new(wire_config.clone());
        for i in 0..move_count {
            let identity = if i % 2 == 0 {
                &player1_identity
            } else {
                &player2_identity
            };
            let chess_move = format!("move_{i}");
            let board_hash = hash_board_state(&Board::new());

            let move_msg = Message::new_move(game_id.clone(), chess_move, board_hash);
            let envelope = SignedEnvelope::create(&move_msg, identity, None).unwrap();

            if i % 2 == 0 {
                framed_message
                    .write_message(&mut write1, &envelope)
                    .await
                    .unwrap();
            } else {
                framed_message
                    .write_message(&mut write2, &envelope)
                    .await
                    .unwrap();
            }
        }
    });

    let receive_task = tokio::spawn(async move {
        let framed_message = FramedMessage::new(WireConfig::for_chess_realtime());
        let mut received_count = 0;

        while received_count < move_count {
            tokio::select! {
                result = framed_message.read_message(&mut read1) => {
                    if let Ok(envelope) = result {
                        let message = envelope.get_message().unwrap();
                        assert!(matches!(message, Message::Move(_)));
                        received_count += 1;
                    }
                }
                result = framed_message.read_message(&mut read2) => {
                    if let Ok(envelope) = result {
                        let message = envelope.get_message().unwrap();
                        assert!(matches!(message, Message::Move(_)));
                        received_count += 1;
                    }
                }
            }
        }

        received_count
    });

    let (_, received_count) = tokio::try_join!(send_task, receive_task)?;
    let duration = start_time.elapsed();

    assert_eq!(received_count, move_count);

    let moves_per_second = move_count as f64 / duration.as_secs_f64();
    println!("   - Processed {} moves in {:?}", move_count, duration);
    println!("   - Performance: {:.2} moves/second", moves_per_second);

    // Performance should be reasonable for real-time chess
    assert!(
        moves_per_second > 100.0,
        "Performance too slow: {:.2} moves/second",
        moves_per_second
    );

    println!("✅ Realistic usage performance test passed");

    Ok(())
}
