//! Integration tests between chess engine and message protocol components
//!
//! This module tests the complete interaction between the chess engine
//! and message protocol, including move processing, game state synchronization,
//! error propagation, and bidirectional integration.

use crate::common::mock_streams::*;
use crate::common::test_data::*;
use mate::chess::{Board, ChessError, Color, Move as ChessMove, Position};
use mate::messages::chess::{
    apply_move_from_message, create_move_message, create_sync_response, generate_game_id,
    hash_board_state, validate_move_message, verify_board_hash, ChessProtocolError, Move,
    SyncRequest, ValidationError,
};
use mate::messages::types::Message;
use mate::messages::wire::FramedMessage;
use std::str::FromStr;
use std::time::Instant;

// =============================================================================
// End-to-End Move Processing Tests
// =============================================================================

#[tokio::test]
async fn test_end_to_end_move_processing_basic() {
    println!("Testing basic end-to-end move processing");

    let game_id = generate_game_id();
    let mut board = Board::new();

    // Create a chess move (e2e4)
    let chess_move = ChessMove::simple(
        Position::from_str("e2").unwrap(),
        Position::from_str("e4").unwrap(),
    )
    .unwrap();

    // Apply the move to get the target board state
    let mut target_board = board.clone();
    target_board.make_move(chess_move).unwrap();

    // Create message from chess move
    let message = create_move_message(&game_id, &chess_move, &target_board);

    // Extract the move message from the wrapper
    let move_msg = match message {
        Message::Move(msg) => msg,
        _ => panic!("Expected Move message"),
    };

    // Validate the message format
    validate_move_message(&move_msg).expect("Move message should be valid");

    // Apply the message move to the original board
    apply_move_from_message(&mut board, &move_msg).expect("Move should apply successfully");

    // Verify boards match
    assert_eq!(
        board.to_fen(),
        target_board.to_fen(),
        "Board states should match after move processing"
    );

    println!("✓ Basic end-to-end move processing test passed");
}

#[tokio::test]
async fn test_end_to_end_move_processing_complex_sequence() {
    println!("Testing complex sequence of moves end-to-end");

    let game_id = generate_game_id();
    let mut board = Board::new();

    // Define a sequence of moves (Scholar's Mate)
    let move_sequence = vec![
        ("e2", "e4"), // 1. e4
        ("e7", "e5"), // 1... e5
        ("d1", "h5"), // 2. Qh5
        ("b8", "c6"), // 2... Nc6
        ("f1", "c4"), // 3. Bc4
        ("g8", "f6"), // 3... Nf6
    ];

    for (i, (from_str, to_str)) in move_sequence.iter().enumerate() {
        let chess_move = ChessMove::simple(
            Position::from_str(from_str).unwrap(),
            Position::from_str(to_str).unwrap(),
        )
        .unwrap();

        // Create target board state
        let mut target_board = board.clone();
        target_board.make_move(chess_move).unwrap();

        // Create and process message
        let message = create_move_message(&game_id, &chess_move, &target_board);
        let move_msg = match message {
            Message::Move(msg) => msg,
            _ => panic!("Expected Move message"),
        };

        // Validate and apply
        validate_move_message(&move_msg).expect("Move message should be valid");
        apply_move_from_message(&mut board, &move_msg).expect("Move should apply successfully");

        // Verify state consistency
        assert_eq!(
            board.to_fen(),
            target_board.to_fen(),
            "Board states should match after move {} ({}{})",
            i + 1,
            from_str,
            to_str
        );
    }

    println!("✓ Complex move sequence processing test passed");
}

#[tokio::test]
async fn test_end_to_end_castling_moves() {
    println!("Testing castling moves end-to-end processing");

    let game_id = generate_game_id();

    // Test castling moves after setting up appropriate position
    let castling_fen = "r3k2r/pppppppp/8/8/8/8/PPPPPPPP/R3K2R w KQkq - 0 1";
    let mut board = Board::from_fen(castling_fen).unwrap();

    // Test kingside castling
    let kingside_castle = ChessMove::from_str_with_color("O-O", Color::White).unwrap();
    let mut target_board = board.clone();
    target_board.make_move(kingside_castle).unwrap();

    let message = create_move_message(&game_id, &kingside_castle, &target_board);
    let move_msg = match message {
        Message::Move(msg) => msg,
        _ => panic!("Expected Move message"),
    };

    apply_move_from_message(&mut board, &move_msg).expect("Castling should succeed");

    // Verify king and rook positions after castling
    assert_eq!(
        board.to_fen(),
        target_board.to_fen(),
        "Board should match after kingside castling"
    );

    println!("✓ Castling move processing test passed");
}

#[tokio::test]
async fn test_end_to_end_promotion_moves() {
    println!("Testing promotion moves end-to-end processing");

    let game_id = generate_game_id();

    // Set up position where white pawn can promote
    let promotion_fen = "8/P7/8/8/8/8/8/8 w - - 0 1";
    let mut board = Board::from_fen(promotion_fen).unwrap();

    // Create promotion move (a7a8q)
    let promotion_move = ChessMove::promotion(
        Position::from_str("a7").unwrap(),
        Position::from_str("a8").unwrap(),
        mate::chess::PieceType::Queen,
    )
    .unwrap();

    let mut target_board = board.clone();
    target_board.make_move(promotion_move).unwrap();

    let message = create_move_message(&game_id, &promotion_move, &target_board);
    let move_msg = match message {
        Message::Move(msg) => msg,
        _ => panic!("Expected Move message"),
    };

    apply_move_from_message(&mut board, &move_msg).expect("Promotion should succeed");

    assert_eq!(
        board.to_fen(),
        target_board.to_fen(),
        "Board should match after promotion"
    );

    println!("✓ Promotion move processing test passed");
}

// =============================================================================
// Game State Synchronization Tests
// =============================================================================

#[tokio::test]
async fn test_game_state_synchronization_empty_history() {
    println!("Testing game state synchronization with empty history");

    let game_id = generate_game_id();
    let board = Board::new();
    let history: Vec<ChessMove> = vec![];

    // Create sync response
    let message = create_sync_response(&game_id, &board, &history);
    let sync_response = match message {
        Message::SyncResponse(response) => response,
        _ => panic!("Expected SyncResponse message"),
    };

    // Verify sync response contents
    assert_eq!(sync_response.game_id, game_id);
    assert_eq!(sync_response.board_state, board.to_fen());
    assert!(sync_response.move_history.is_empty());
    assert_eq!(sync_response.board_state_hash, hash_board_state(&board));

    // Verify board recreation from sync
    let reconstructed_board = Board::from_fen(&sync_response.board_state).unwrap();
    assert_eq!(
        reconstructed_board.to_fen(),
        board.to_fen(),
        "Reconstructed board should match original"
    );

    println!("✓ Empty history synchronization test passed");
}

#[tokio::test]
async fn test_game_state_synchronization_large_history() {
    println!("Testing game state synchronization with large move history");

    let game_id = generate_game_id();
    let mut board = Board::new();
    let mut history = Vec::new();

    // Create a realistic game with many moves (50 moves total)
    let move_pairs = vec![
        ("e2", "e4", "e7", "e5"),
        ("g1", "f3", "b8", "c6"),
        ("f1", "b5", "a7", "a6"),
        ("b5", "a4", "g8", "f6"),
        ("O-O", "O-O", "f8", "e7"), // Note: using standard notation
        ("f1", "e1", "b7", "b5"),
        ("a4", "b3", "d7", "d6"),
        ("c2", "c3", "c8", "g4"),
        ("h2", "h3", "g4", "h5"),
        ("d2", "d3", "f6", "d7"),
    ];

    for (white_from, white_to, black_from, black_to) in move_pairs {
        // White move
        let white_move = if white_from == "O-O" {
            ChessMove::from_str_with_color("O-O", Color::White).unwrap()
        } else {
            ChessMove::simple(
                Position::from_str(white_from).unwrap(),
                Position::from_str(white_to).unwrap(),
            )
            .unwrap()
        };

        board.make_move(white_move).unwrap();
        history.push(white_move);

        // Black move
        let black_move = if black_from == "O-O" {
            ChessMove::from_str_with_color("O-O", Color::Black).unwrap()
        } else {
            ChessMove::simple(
                Position::from_str(black_from).unwrap(),
                Position::from_str(black_to).unwrap(),
            )
            .unwrap()
        };

        board.make_move(black_move).unwrap();
        history.push(black_move);
    }

    // Create sync response with large history
    let message = create_sync_response(&game_id, &board, &history);
    let sync_response = match message {
        Message::SyncResponse(response) => response,
        _ => panic!("Expected SyncResponse message"),
    };

    // Verify sync response integrity
    assert_eq!(sync_response.game_id, game_id);
    assert_eq!(sync_response.board_state, board.to_fen());
    assert_eq!(sync_response.move_history.len(), history.len());
    assert_eq!(sync_response.board_state_hash, hash_board_state(&board));

    // Verify move history accuracy
    for (i, expected_move) in history.iter().enumerate() {
        assert_eq!(
            sync_response.move_history[i],
            expected_move.to_string(),
            "Move {} should match in history",
            i + 1
        );
    }

    // Test reconstruction from sync data
    let mut reconstructed_board = Board::new();
    for (i, move_str) in sync_response.move_history.iter().enumerate() {
        let color = if i % 2 == 0 {
            Color::White
        } else {
            Color::Black
        };
        let chess_move = ChessMove::from_str_with_color(move_str, color).unwrap();
        reconstructed_board.make_move(chess_move).unwrap();
    }

    assert_eq!(
        reconstructed_board.to_fen(),
        sync_response.board_state,
        "Reconstructed board should match sync board state"
    );

    println!("✓ Large history synchronization test passed");
}

#[tokio::test]
async fn test_fen_accuracy_in_sync_messages() {
    println!("Testing FEN accuracy in sync messages");

    let game_id = generate_game_id();

    // Test various board positions
    let test_positions = vec![
        (
            "Starting position",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        ),
        (
            "After 1.e4",
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        ),
        (
            "After 1.e4 e5",
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
        ),
        ("Empty board", "8/8/8/8/8/8/8/8 w - - 0 1"),
    ];

    for (description, expected_fen) in test_positions {
        let board = Board::from_fen(expected_fen).unwrap();
        let history = Vec::new(); // Empty for these tests

        let message = create_sync_response(&game_id, &board, &history);
        let sync_response = match message {
            Message::SyncResponse(response) => response,
            _ => panic!("Expected SyncResponse message"),
        };

        assert_eq!(
            sync_response.board_state, expected_fen,
            "FEN should be accurate for {}",
            description
        );

        // Verify roundtrip accuracy
        let reconstructed_board = Board::from_fen(&sync_response.board_state).unwrap();
        assert_eq!(
            reconstructed_board.to_fen(),
            expected_fen,
            "FEN roundtrip should be accurate for {}",
            description
        );
    }

    println!("✓ FEN accuracy in sync messages test passed");
}

#[tokio::test]
async fn test_board_state_hash_verification() {
    println!("Testing board state hash verification in sync");

    let game_id = generate_game_id();
    let mut board = Board::new();

    // Make a few moves to create a unique position
    let moves = vec![
        ChessMove::simple(
            Position::from_str("e2").unwrap(),
            Position::from_str("e4").unwrap(),
        )
        .unwrap(),
        ChessMove::simple(
            Position::from_str("e7").unwrap(),
            Position::from_str("e5").unwrap(),
        )
        .unwrap(),
    ];

    for chess_move in &moves {
        board.make_move(*chess_move).unwrap();
    }

    let message = create_sync_response(&game_id, &board, &moves);
    let sync_response = match message {
        Message::SyncResponse(response) => response,
        _ => panic!("Expected SyncResponse message"),
    };

    // Verify hash matches current board
    let actual_hash = hash_board_state(&board);
    assert_eq!(
        sync_response.board_state_hash, actual_hash,
        "Sync response hash should match board hash"
    );

    // Verify hash verification utility
    assert!(
        verify_board_hash(&board, &sync_response.board_state_hash),
        "Board hash verification should succeed"
    );

    // Test with incorrect hash
    let wrong_hash = "incorrect_hash";
    assert!(
        !verify_board_hash(&board, wrong_hash),
        "Board hash verification should fail with wrong hash"
    );

    println!("✓ Board state hash verification test passed");
}

// =============================================================================
// Cross-Module Error Propagation Tests
// =============================================================================

#[tokio::test]
async fn test_invalid_move_error_propagation() {
    println!("Testing invalid move error propagation");

    let game_id = generate_game_id();
    let mut board = Board::new();

    // Create an invalid move message (moving to same square)
    let invalid_move_msg = Move::new(
        game_id.clone(),
        "e2e2".to_string(),
        "dummy_hash".to_string(),
    );

    // Test error propagation from chess engine
    let result = apply_move_from_message(&mut board, &invalid_move_msg);
    assert!(result.is_err(), "Invalid move should produce an error");

    match result.unwrap_err() {
        ChessError::InvalidMove(_) => {
            // Expected error type
        }
        other => panic!("Expected InvalidMove error, got {:?}", other),
    }

    println!("✓ Invalid move error propagation test passed");
}

#[tokio::test]
async fn test_board_hash_mismatch_error_propagation() {
    println!("Testing board hash mismatch error propagation");

    let game_id = generate_game_id();
    let mut board = Board::new();

    // Create a valid move but with wrong hash
    let move_msg = Move::new(game_id, "e2e4".to_string(), "wrong_hash_value".to_string());

    let result = apply_move_from_message(&mut board, &move_msg);
    assert!(
        result.is_err(),
        "Move with wrong hash should produce an error"
    );

    match result.unwrap_err() {
        ChessError::BoardStateError(_) => {
            // Expected error type for hash mismatch
        }
        other => panic!("Expected BoardStateError, got {:?}", other),
    }

    println!("✓ Board hash mismatch error propagation test passed");
}

#[tokio::test]
async fn test_validation_error_propagation() {
    println!("Testing validation error propagation");

    // Test invalid game ID
    let invalid_move_msg = Move::new(
        "not-a-uuid".to_string(),
        "e2e4".to_string(),
        "hash".to_string(),
    );

    let validation_result = validate_move_message(&invalid_move_msg);
    assert!(
        validation_result.is_err(),
        "Invalid game ID should fail validation"
    );

    match validation_result.unwrap_err() {
        ValidationError::InvalidGameId(_) => {
            // Expected error type
        }
        other => panic!("Expected InvalidGameId error, got {:?}", other),
    }

    // Test invalid move format
    let invalid_format_msg = Move::new(
        generate_game_id(),
        "invalid_move_format".to_string(),
        "hash".to_string(),
    );

    let format_result = validate_move_message(&invalid_format_msg);
    assert!(
        format_result.is_err(),
        "Invalid move format should fail validation"
    );

    println!("✓ Validation error propagation test passed");
}

#[tokio::test]
async fn test_chess_protocol_error_conversions() {
    println!("Testing chess protocol error conversions");

    let game_id = generate_game_id();

    // Test ValidationError conversion
    let validation_error = ValidationError::InvalidGameId("test".to_string());
    let protocol_error: ChessProtocolError = validation_error.into();
    match protocol_error {
        ChessProtocolError::Validation(_) => {}
        other => panic!("Expected Validation variant, got {:?}", other),
    }

    // Test ChessError conversion
    let chess_error = ChessError::InvalidMove("test move".to_string());
    let protocol_error: ChessProtocolError = chess_error.into();
    match protocol_error {
        ChessProtocolError::ChessEngine(_) => {}
        other => panic!("Expected ChessEngine variant, got {:?}", other),
    }

    // Test error categorization
    let sync_error = ChessProtocolError::sync_error(game_id.clone(), "sync failed".to_string());
    assert!(!sync_error.is_security_related());
    assert_eq!(sync_error.category(), "sync");

    println!("✓ Chess protocol error conversions test passed");
}

// =============================================================================
// Bidirectional Integration Tests
// =============================================================================

#[tokio::test]
async fn test_complete_move_roundtrip() {
    println!("Testing complete move roundtrip integration");

    let game_id = generate_game_id();
    let mut source_board = Board::new();
    let mut target_board = Board::new();

    // Create and apply move on source
    let chess_move = ChessMove::simple(
        Position::from_str("d2").unwrap(),
        Position::from_str("d4").unwrap(),
    )
    .unwrap();

    source_board.make_move(chess_move).unwrap();

    // Create message from source board
    let message = create_move_message(&game_id, &chess_move, &source_board);
    let move_msg = match message {
        Message::Move(msg) => msg,
        _ => panic!("Expected Move message"),
    };

    // Transmit through wire protocol (simulation)
    let framed_message = FramedMessage::default();
    let (envelope, _) = create_test_envelope_with_message(&Message::Move(move_msg.clone()));

    let mut stream = MockStream::new();
    framed_message
        .write_message(&mut stream, &envelope)
        .await
        .expect("Should write message");

    let written_data = stream.get_written_data().to_vec();
    let mut read_stream = MockStream::with_data(written_data);

    let received_envelope = framed_message
        .read_message(&mut read_stream)
        .await
        .expect("Should read message");

    let received_message = received_envelope.get_message().expect("Should get message");
    let received_move_msg = match received_message {
        Message::Move(msg) => msg,
        _ => panic!("Expected Move message"),
    };

    // Apply received message to target board
    apply_move_from_message(&mut target_board, &received_move_msg)
        .expect("Should apply received move");

    // Verify complete roundtrip
    assert_eq!(
        source_board.to_fen(),
        target_board.to_fen(),
        "Boards should match after complete roundtrip"
    );

    println!("✓ Complete move roundtrip integration test passed");
}

#[tokio::test]
async fn test_sync_request_response_cycle() {
    println!("Testing sync request-response cycle integration");

    let game_id = generate_game_id();
    let mut game_board = Board::new();
    let mut moves_history = Vec::new();

    // Play several moves to create game state
    let moves = vec![("e2", "e4"), ("e7", "e5"), ("f1", "c4"), ("f8", "c5")];

    for (from_str, to_str) in moves {
        let chess_move = ChessMove::simple(
            Position::from_str(from_str).unwrap(),
            Position::from_str(to_str).unwrap(),
        )
        .unwrap();

        game_board.make_move(chess_move).unwrap();
        moves_history.push(chess_move);
    }

    // Create sync request
    let sync_request = SyncRequest::new(game_id.clone());
    let _request_message = Message::SyncRequest(sync_request);

    // Process sync request and create response
    let response_message = create_sync_response(&game_id, &game_board, &moves_history);
    let sync_response = match response_message {
        Message::SyncResponse(response) => response,
        _ => panic!("Expected SyncResponse message"),
    };

    // Client receives sync response and reconstructs game state
    let client_board = Board::from_fen(&sync_response.board_state).unwrap();

    // Verify reconstructed state matches
    assert_eq!(
        client_board.to_fen(),
        game_board.to_fen(),
        "Client board should match server board after sync"
    );

    // Verify move history can be replayed
    let mut replay_board = Board::new();
    for (i, move_str) in sync_response.move_history.iter().enumerate() {
        let color = if i % 2 == 0 {
            Color::White
        } else {
            Color::Black
        };
        let chess_move = ChessMove::from_str_with_color(move_str, color).unwrap();
        replay_board.make_move(chess_move).unwrap();
    }

    assert_eq!(
        replay_board.to_fen(),
        game_board.to_fen(),
        "Replayed game should match original"
    );

    println!("✓ Sync request-response cycle integration test passed");
}

#[tokio::test]
async fn test_message_validation_in_integration_flow() {
    println!("Testing message validation in integration flow");

    let game_id = generate_game_id();
    let board = Board::new();

    // Create valid move message
    let chess_move = ChessMove::simple(
        Position::from_str("a2").unwrap(),
        Position::from_str("a3").unwrap(),
    )
    .unwrap();

    let mut target_board = board.clone();
    target_board.make_move(chess_move).unwrap();

    let message = create_move_message(&game_id, &chess_move, &target_board);
    let move_msg = match message {
        Message::Move(msg) => msg,
        _ => panic!("Expected Move message"),
    };

    // Validation should pass
    validate_move_message(&move_msg).expect("Valid message should pass validation");

    // Test integration with invalid data
    let invalid_msg = Move::new(
        "invalid-id".to_string(),
        "invalid-move".to_string(),
        "invalid-hash".to_string(),
    );

    let validation_result = validate_move_message(&invalid_msg);
    assert!(
        validation_result.is_err(),
        "Invalid message should fail validation"
    );

    println!("✓ Message validation in integration flow test passed");
}

// =============================================================================
// Performance with Realistic Game Scenarios Tests
// =============================================================================

#[tokio::test]
async fn test_performance_large_game_processing() {
    println!("Testing performance with large game processing");

    let game_id = generate_game_id();
    let mut board = Board::new();
    let mut moves_history = Vec::new();

    // Create a long game sequence (100 moves)
    let start_time = Instant::now();

    for move_num in 1..=50 {
        // Alternate between some basic opening moves and random legal moves
        let (white_move, black_move) = match move_num {
            1 => ("e2e4", "e7e5"),
            2 => ("g1f3", "b8c6"),
            3 => ("f1b5", "a7a6"),
            4 => ("b5a4", "g8f6"),
            5 => ("d2d3", "f8e7"),
            _ => {
                // Use some repetitive but legal moves
                if move_num % 2 == 0 {
                    ("h1g1", "h8g8")
                } else {
                    ("g1h1", "g8h8")
                }
            }
        };

        // Process white move
        let white_chess_move = ChessMove::from_str(white_move).unwrap();
        let mut white_target = board.clone();
        white_target.make_move(white_chess_move).unwrap();

        let white_message = create_move_message(&game_id, &white_chess_move, &white_target);
        let white_move_msg = match white_message {
            Message::Move(msg) => msg,
            _ => panic!("Expected Move message"),
        };

        apply_move_from_message(&mut board, &white_move_msg).unwrap();
        moves_history.push(white_chess_move);

        // Process black move
        let black_chess_move = ChessMove::from_str(black_move).unwrap();
        let mut black_target = board.clone();
        black_target.make_move(black_chess_move).unwrap();

        let black_message = create_move_message(&game_id, &black_chess_move, &black_target);
        let black_move_msg = match black_message {
            Message::Move(msg) => msg,
            _ => panic!("Expected Move message"),
        };

        apply_move_from_message(&mut board, &black_move_msg).unwrap();
        moves_history.push(black_chess_move);
    }

    let processing_duration = start_time.elapsed();

    // Performance assertions
    assert!(
        processing_duration.as_millis() < 1000,
        "Processing 100 moves should complete in under 1 second, took {:?}",
        processing_duration
    );

    // Verify final state integrity
    assert_eq!(moves_history.len(), 100, "Should have processed 100 moves");

    // Test sync response creation performance
    let sync_start = Instant::now();
    let _sync_message = create_sync_response(&game_id, &board, &moves_history);
    let sync_duration = sync_start.elapsed();

    assert!(
        sync_duration.as_millis() < 100,
        "Sync response creation should complete in under 100ms, took {:?}",
        sync_duration
    );

    println!("✓ Performance with large game processing test passed");
    println!("  - 100 moves processed in {:?}", processing_duration);
    println!("  - Sync response created in {:?}", sync_duration);
}

#[tokio::test]
async fn test_concurrent_game_message_processing() {
    println!("Testing concurrent game message processing");

    use tokio::task;

    let num_games = 10;
    let moves_per_game = 20;

    let start_time = Instant::now();

    // Spawn multiple concurrent game processing tasks
    let mut handles = Vec::new();

    for game_num in 0..num_games {
        let handle = task::spawn(async move {
            let game_id = generate_game_id();
            let mut board = Board::new();
            let mut move_count = 0;

            for move_idx in 0..moves_per_game {
                let move_str = if move_idx % 4 == 0 {
                    "e2e4"
                } else if move_idx % 4 == 1 {
                    "e7e5"
                } else if move_idx % 4 == 2 {
                    "e4e2"
                } else {
                    "e5e7"
                };

                let chess_move = ChessMove::from_str(move_str).unwrap();
                let mut target_board = board.clone();
                target_board.make_move(chess_move).unwrap();

                let message = create_move_message(&game_id, &chess_move, &target_board);
                let move_msg = match message {
                    Message::Move(msg) => msg,
                    _ => panic!("Expected Move message"),
                };

                apply_move_from_message(&mut board, &move_msg).unwrap();
                move_count += 1;
            }

            (game_num, move_count)
        });

        handles.push(handle);
    }

    // Wait for all games to complete
    let mut total_moves = 0;
    for handle in handles {
        let (game_num, moves) = handle.await.unwrap();
        total_moves += moves;
        assert_eq!(
            moves, moves_per_game,
            "Game {} should complete all moves",
            game_num
        );
    }

    let total_duration = start_time.elapsed();

    // Performance assertions
    assert_eq!(
        total_moves,
        num_games * moves_per_game,
        "Should process all moves"
    );
    assert!(
        total_duration.as_millis() < 2000,
        "Concurrent processing should complete in under 2 seconds, took {:?}",
        total_duration
    );

    println!("✓ Concurrent game message processing test passed");
    println!("  - {} games with {} moves each", num_games, moves_per_game);
    println!("  - Total processing time: {:?}", total_duration);
}

#[tokio::test]
async fn test_memory_efficiency_large_sync_messages() {
    println!("Testing memory efficiency with large sync messages");

    let game_id = generate_game_id();
    let board = Board::new();

    // Create very large move history (simulate long game)
    let large_history: Vec<ChessMove> = (0..500)
        .map(|i| {
            let move_str = if i % 2 == 0 { "e2e4" } else { "e4e2" };
            ChessMove::from_str(move_str).unwrap()
        })
        .collect();

    let start_time = Instant::now();

    // Create sync response
    let sync_message = create_sync_response(&game_id, &board, &large_history);
    let sync_response = match sync_message {
        Message::SyncResponse(response) => response,
        _ => panic!("Expected SyncResponse message"),
    };

    let creation_duration = start_time.elapsed();

    // Verify large history handling
    assert_eq!(
        sync_response.move_history.len(),
        large_history.len(),
        "Should preserve all moves in history"
    );

    // Test serialization efficiency
    let serialization_start = Instant::now();
    let (envelope, _) =
        create_test_envelope_with_message(&Message::SyncResponse(sync_response.clone()));
    let serialization_duration = serialization_start.elapsed();

    // Performance and memory efficiency assertions
    assert!(
        creation_duration.as_millis() < 50,
        "Large sync creation should be fast, took {:?}",
        creation_duration
    );

    assert!(
        serialization_duration.as_millis() < 100,
        "Large sync serialization should be efficient, took {:?}",
        serialization_duration
    );

    // Verify message size is reasonable (not excessive overhead)
    let message_json = serde_json::to_string(&envelope).unwrap();
    let message_size = message_json.len();

    // Should be roughly proportional to move count, not exponential
    assert!(
        message_size < 100_000, // 100KB should be more than enough for 500 moves
        "Message size should be reasonable, got {} bytes",
        message_size
    );

    println!("✓ Memory efficiency with large sync messages test passed");
    println!("  - 500 moves processed in {:?}", creation_duration);
    println!("  - Serialization took {:?}", serialization_duration);
    println!("  - Message size: {} bytes", message_size);
}
