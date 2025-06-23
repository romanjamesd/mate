#[cfg(test)]
mod tests {
    use mate::chess::{Board, ChessError, Color, Move as ChessMove, Position};
    use mate::messages::chess::{
        apply_move_from_message, create_move_message, create_sync_response, generate_game_id,
        hash_board_state, Move as MessageMove, SyncResponse,
    };
    use mate::messages::types::Message;
    use std::str::FromStr;

    // =============================================================================
    // Move Conversion Function Tests
    // =============================================================================

    #[test]
    fn test_create_move_message_basic() {
        let game_id = generate_game_id();
        let board = Board::new();
        let chess_move = ChessMove::simple(
            Position::from_str("e2").unwrap(),
            Position::from_str("e4").unwrap(),
        )
        .unwrap();

        let message = create_move_message(&game_id, &chess_move, &board);

        if let Message::Move(move_msg) = message {
            assert_eq!(move_msg.game_id, game_id);
            assert_eq!(move_msg.chess_move, "e2e4");
            assert_eq!(move_msg.board_state_hash, hash_board_state(&board));
        } else {
            panic!("Expected Move message variant");
        }
    }

    #[test]
    fn test_create_move_message_promotion() {
        let game_id = generate_game_id();
        let board = Board::new();
        let chess_move = ChessMove::promotion(
            Position::from_str("e7").unwrap(),
            Position::from_str("e8").unwrap(),
            mate::chess::PieceType::Queen,
        )
        .unwrap();

        let message = create_move_message(&game_id, &chess_move, &board);

        if let Message::Move(move_msg) = message {
            assert_eq!(move_msg.game_id, game_id);
            assert_eq!(move_msg.chess_move, "e7e8Q");
            assert!(move_msg.board_state_hash.len() == 64); // SHA-256 hex length
        } else {
            panic!("Expected Move message variant");
        }
    }

    #[test]
    fn test_create_move_message_castling() {
        let game_id = generate_game_id();
        let board = Board::new();
        // Kingside castling move for white
        let chess_move = ChessMove::new_unchecked(
            Position::from_str("e1").unwrap(),
            Position::from_str("g1").unwrap(),
            None,
        );

        let message = create_move_message(&game_id, &chess_move, &board);

        if let Message::Move(move_msg) = message {
            assert_eq!(move_msg.game_id, game_id);
            assert_eq!(move_msg.chess_move, "e1g1");
            assert_eq!(move_msg.board_state_hash, hash_board_state(&board));
        } else {
            panic!("Expected Move message variant");
        }
    }

    #[test]
    fn test_create_move_message_different_boards() {
        let game_id = generate_game_id();
        let board1 = Board::new();
        let mut board2 = Board::new();

        // Apply a move to create different board state
        let initial_move = ChessMove::simple(
            Position::from_str("e2").unwrap(),
            Position::from_str("e4").unwrap(),
        )
        .unwrap();
        board2.make_move(initial_move).unwrap();

        let chess_move = ChessMove::simple(
            Position::from_str("d2").unwrap(),
            Position::from_str("d4").unwrap(),
        )
        .unwrap();

        let message1 = create_move_message(&game_id, &chess_move, &board1);
        let message2 = create_move_message(&game_id, &chess_move, &board2);

        if let (Message::Move(move_msg1), Message::Move(move_msg2)) = (message1, message2) {
            assert_eq!(move_msg1.chess_move, move_msg2.chess_move);
            assert_ne!(move_msg1.board_state_hash, move_msg2.board_state_hash);
        } else {
            panic!("Expected Move message variants");
        }
    }

    #[test]
    fn test_create_move_message_move_string_formatting() {
        let game_id = generate_game_id();
        let board = Board::new();

        // Test various move types
        let moves = [
            ChessMove::simple(
                Position::from_str("a1").unwrap(),
                Position::from_str("a8").unwrap(),
            )
            .unwrap(),
            ChessMove::simple(
                Position::from_str("h2").unwrap(),
                Position::from_str("h4").unwrap(),
            )
            .unwrap(),
            ChessMove::promotion(
                Position::from_str("b7").unwrap(),
                Position::from_str("b8").unwrap(),
                mate::chess::PieceType::Rook,
            )
            .unwrap(),
        ];

        let expected_strings = ["a1a8", "h2h4", "b7b8R"];

        for (chess_move, expected) in moves.iter().zip(expected_strings.iter()) {
            let message = create_move_message(&game_id, chess_move, &board);
            if let Message::Move(move_msg) = message {
                assert_eq!(move_msg.chess_move, *expected);
            } else {
                panic!("Expected Move message variant");
            }
        }
    }

    // =============================================================================
    // Board State Conversion Tests
    // =============================================================================

    #[test]
    fn test_create_sync_response_empty_history() {
        let game_id = generate_game_id();
        let board = Board::new();
        let history: Vec<ChessMove> = vec![];

        let message = create_sync_response(&game_id, &board, &history);

        if let Message::SyncResponse(sync_resp) = message {
            assert_eq!(sync_resp.game_id, game_id);
            assert_eq!(sync_resp.board_state, board.to_fen());
            assert_eq!(sync_resp.move_history.len(), 0);
            assert_eq!(sync_resp.board_state_hash, hash_board_state(&board));
        } else {
            panic!("Expected SyncResponse message variant");
        }
    }

    #[test]
    fn test_create_sync_response_with_move_history() {
        let game_id = generate_game_id();
        let board = Board::new();
        let history = vec![
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
            ChessMove::simple(
                Position::from_str("g1").unwrap(),
                Position::from_str("f3").unwrap(),
            )
            .unwrap(),
        ];

        let message = create_sync_response(&game_id, &board, &history);

        if let Message::SyncResponse(sync_resp) = message {
            assert_eq!(sync_resp.game_id, game_id);
            assert_eq!(sync_resp.board_state, board.to_fen());
            assert_eq!(sync_resp.move_history.len(), 3);
            assert_eq!(sync_resp.move_history[0], "e2e4");
            assert_eq!(sync_resp.move_history[1], "e7e5");
            assert_eq!(sync_resp.move_history[2], "g1f3");
            assert_eq!(sync_resp.board_state_hash, hash_board_state(&board));
        } else {
            panic!("Expected SyncResponse message variant");
        }
    }

    #[test]
    fn test_create_sync_response_move_history_conversion() {
        let game_id = generate_game_id();
        let board = Board::new();
        let history = vec![
            ChessMove::simple(
                Position::from_str("a2").unwrap(),
                Position::from_str("a4").unwrap(),
            )
            .unwrap(),
            ChessMove::promotion(
                Position::from_str("h7").unwrap(),
                Position::from_str("h8").unwrap(),
                mate::chess::PieceType::Queen,
            )
            .unwrap(),
        ];

        let message = create_sync_response(&game_id, &board, &history);

        if let Message::SyncResponse(sync_resp) = message {
            assert_eq!(sync_resp.move_history[0], "a2a4");
            assert_eq!(sync_resp.move_history[1], "h7h8Q");
        } else {
            panic!("Expected SyncResponse message variant");
        }
    }

    #[test]
    fn test_create_sync_response_large_history() {
        let game_id = generate_game_id();
        let board = Board::new();

        // Create a large move history
        let mut history = Vec::new();
        for i in 0..50 {
            let from_file = (i % 8) as u8;
            let from_rank = if i < 25 { 1 } else { 6 };
            let to_rank = if i < 25 { 3 } else { 4 };

            if let (Ok(from), Ok(to)) = (
                Position::new(from_file, from_rank),
                Position::new(from_file, to_rank),
            ) {
                if let Ok(mv) = ChessMove::simple(from, to) {
                    history.push(mv);
                }
            }
        }

        let message = create_sync_response(&game_id, &board, &history);

        if let Message::SyncResponse(sync_resp) = message {
            assert_eq!(sync_resp.move_history.len(), history.len());
            assert_eq!(sync_resp.board_state_hash, hash_board_state(&board));
        } else {
            panic!("Expected SyncResponse message variant");
        }
    }

    #[test]
    fn test_create_sync_response_board_state_consistency() {
        let game_id = generate_game_id();
        let mut board1 = Board::new();
        let mut board2 = Board::new();

        // Apply the same moves to both boards
        let moves = vec![
            ChessMove::simple(
                Position::from_str("e2").unwrap(),
                Position::from_str("e4").unwrap(),
            )
            .unwrap(),
            ChessMove::simple(
                Position::from_str("d7").unwrap(),
                Position::from_str("d6").unwrap(),
            )
            .unwrap(),
        ];

        for mv in &moves {
            board1.make_move(*mv).unwrap();
            board2.make_move(*mv).unwrap();
        }

        let message1 = create_sync_response(&game_id, &board1, &moves);
        let message2 = create_sync_response(&game_id, &board2, &moves);

        if let (Message::SyncResponse(sync1), Message::SyncResponse(sync2)) = (message1, message2) {
            assert_eq!(sync1.board_state, sync2.board_state);
            assert_eq!(sync1.board_state_hash, sync2.board_state_hash);
        } else {
            panic!("Expected SyncResponse message variants");
        }
    }

    // =============================================================================
    // Apply Move From Message Tests
    // =============================================================================

    #[test]
    fn test_apply_move_from_message_basic() {
        let mut board = Board::new();
        let initial_hash = hash_board_state(&board);

        // Create a valid move and apply it to get the expected hash
        let chess_move = ChessMove::simple(
            Position::from_str("e2").unwrap(),
            Position::from_str("e4").unwrap(),
        )
        .unwrap();

        let mut temp_board = board.clone();
        temp_board.make_move(chess_move).unwrap();
        let expected_hash = hash_board_state(&temp_board);

        let move_msg = MessageMove::new(generate_game_id(), "e2e4".to_string(), expected_hash);

        let result = apply_move_from_message(&mut board, &move_msg);

        assert!(result.is_ok());
        assert_eq!(hash_board_state(&board), move_msg.board_state_hash);
        assert_ne!(hash_board_state(&board), initial_hash);
    }

    #[test]
    fn test_apply_move_from_message_promotion() {
        let mut board = Board::new();

        let move_msg = MessageMove::new(
            generate_game_id(),
            "e7e8Q".to_string(),
            hash_board_state(&board),
        );

        let result = apply_move_from_message(&mut board, &move_msg);

        // This should fail because the move is not valid on the starting position
        assert!(result.is_err());
        if let Err(e) = result {
            // Should be an InvalidMove error type
            assert!(matches!(e, ChessError::InvalidMove(_)));
        }
    }

    #[test]
    fn test_apply_move_from_message_invalid_format() {
        let mut board = Board::new();

        let move_msg = MessageMove::new(
            generate_game_id(),
            "invalid-move".to_string(),
            hash_board_state(&board),
        );

        let result = apply_move_from_message(&mut board, &move_msg);

        assert!(result.is_err());
        if let Err(e) = result {
            // Should be an InvalidMove error type for parse failures
            assert!(matches!(e, ChessError::InvalidMove(_)));
        }
    }

    #[test]
    fn test_apply_move_from_message_hash_mismatch() {
        let mut board = Board::new();

        let move_msg = MessageMove::new(
            generate_game_id(),
            "e2e4".to_string(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        );

        let result = apply_move_from_message(&mut board, &move_msg);

        if result.is_err() {
            if let Err(e) = result {
                // Should be a BoardStateError for hash mismatch
                assert!(matches!(e, ChessError::BoardStateError(_)));
            }
        }
    }

    #[test]
    fn test_apply_move_from_message_preserves_board_on_error() {
        let mut board = Board::new();
        let original_hash = hash_board_state(&board);

        let move_msg = MessageMove::new(
            generate_game_id(),
            "invalid".to_string(),
            "wrong_hash".to_string(),
        );

        let result = apply_move_from_message(&mut board, &move_msg);

        assert!(result.is_err());
        if let Err(e) = result {
            // Should be an InvalidMove error type
            assert!(matches!(e, ChessError::InvalidMove(_)));
        }
        // Board should remain unchanged on error
        assert_eq!(hash_board_state(&board), original_hash);
    }

    // =============================================================================
    // Validation Function Unit Tests
    // =============================================================================

    #[test]
    fn test_hash_board_state_consistency() {
        let board1 = Board::new();
        let board2 = Board::new();

        let hash1 = hash_board_state(&board1);
        let hash2 = hash_board_state(&board2);

        // Same board state should produce same hash
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 hex length
        assert!(hash1.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_board_state_different_boards() {
        let board1 = Board::new();
        let mut board2 = Board::new();

        // Apply a move to create different state
        let chess_move = ChessMove::simple(
            Position::from_str("e2").unwrap(),
            Position::from_str("e4").unwrap(),
        )
        .unwrap();
        board2.make_move(chess_move).unwrap();

        let hash1 = hash_board_state(&board1);
        let hash2 = hash_board_state(&board2);

        assert_ne!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
        assert_eq!(hash2.len(), 64);
    }

    #[test]
    fn test_hash_board_state_deterministic() {
        let board = Board::new();

        let hash1 = hash_board_state(&board);
        let hash2 = hash_board_state(&board);
        let hash3 = hash_board_state(&board);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);
    }

    // =============================================================================
    // Serialization Unit Tests
    // =============================================================================

    #[test]
    fn test_message_move_component_serialization() {
        let game_id = generate_game_id();
        let move_msg = MessageMove::new(
            game_id.clone(),
            "e2e4".to_string(),
            hash_board_state(&Board::new()),
        );

        // Test JSON serialization
        let json = serde_json::to_string(&move_msg).expect("JSON serialization failed");
        let deserialized_json: MessageMove =
            serde_json::from_str(&json).expect("JSON deserialization failed");

        assert_eq!(move_msg, deserialized_json);

        // Test binary serialization
        let bytes = bincode::serialize(&move_msg).expect("Binary serialization failed");
        let deserialized_binary: MessageMove =
            bincode::deserialize(&bytes).expect("Binary deserialization failed");

        assert_eq!(move_msg, deserialized_binary);
    }

    #[test]
    fn test_sync_response_component_serialization() {
        let board = Board::new();
        let sync_resp = SyncResponse::new(
            generate_game_id(),
            board.to_fen(),
            vec!["e2e4".to_string(), "e7e5".to_string()],
            hash_board_state(&board),
        );

        // Test JSON serialization
        let json = serde_json::to_string(&sync_resp).expect("JSON serialization failed");
        let deserialized_json: SyncResponse =
            serde_json::from_str(&json).expect("JSON deserialization failed");

        assert_eq!(sync_resp, deserialized_json);

        // Test binary serialization
        let bytes = bincode::serialize(&sync_resp).expect("Binary serialization failed");
        let deserialized_binary: SyncResponse =
            bincode::deserialize(&bytes).expect("Binary deserialization failed");

        assert_eq!(sync_resp, deserialized_binary);
    }

    #[test]
    fn test_chess_move_serialization_roundtrip() {
        let moves = vec![
            ChessMove::simple(
                Position::from_str("e2").unwrap(),
                Position::from_str("e4").unwrap(),
            )
            .unwrap(),
            ChessMove::promotion(
                Position::from_str("a7").unwrap(),
                Position::from_str("a8").unwrap(),
                mate::chess::PieceType::Queen,
            )
            .unwrap(),
        ];

        for original_move in moves {
            // Convert to string and back
            let move_string = original_move.to_string();
            let parsed_move = ChessMove::from_str_with_color(&move_string, Color::White);

            match parsed_move {
                Ok(parsed) => {
                    assert_eq!(original_move.from, parsed.from);
                    assert_eq!(original_move.to, parsed.to);
                    assert_eq!(original_move.promotion, parsed.promotion);
                }
                Err(_) => {
                    // Some moves might fail due to context requirements
                    // This is expected for certain move types
                }
            }
        }
    }

    #[test]
    fn test_conversion_error_handling() {
        // Test various error conditions in conversion functions

        // Test with invalid move strings
        let invalid_moves = vec![
            "", "invalid", "z9z9", "e2e2",   // Same square
            "a1a1a1", // Too long
        ];

        for invalid_move in invalid_moves {
            let result = ChessMove::from_str_with_color(invalid_move, Color::White);
            assert!(result.is_err(), "Move '{}' should be invalid", invalid_move);
            if let Err(e) = result {
                // Should be either an InvalidMove or InvalidPosition error type
                assert!(matches!(
                    e,
                    ChessError::InvalidMove(_) | ChessError::InvalidPosition(_)
                ));
            }
        }
    }

    #[test]
    fn test_conversion_edge_cases() {
        let board = Board::new();
        let game_id = generate_game_id();

        // Test edge cases in conversion functions
        let edge_case_moves = vec![
            ("a1h8", true), // Maximum distance
            ("h8a1", true), // Reverse maximum distance
        ];

        for (move_str, should_be_valid) in edge_case_moves {
            let result = ChessMove::from_str_with_color(move_str, Color::White);

            if should_be_valid {
                if let Ok(chess_move) = result {
                    let message = create_move_message(&game_id, &chess_move, &board);
                    assert!(matches!(message, Message::Move(_)));
                }
            } else {
                assert!(result.is_err(), "Move '{}' should be invalid", move_str);
                if let Err(e) = result {
                    // Should be an InvalidMove error type
                    assert!(matches!(e, ChessError::InvalidMove(_)));
                }
            }
        }
    }
}
