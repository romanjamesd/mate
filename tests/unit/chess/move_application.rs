use mate::chess::{Board, ChessError, Color, Move, Piece, PieceType, Position};

#[cfg(test)]
mod basic_move_application_tests {
    use super::*;

    #[test]
    fn test_simple_pawn_move() {
        let mut board = Board::new();

        // Test moving white pawn from e2 to e4
        let from = Position::new_unchecked(4, 1); // e2
        let to = Position::new_unchecked(4, 3); // e4
        let pawn_move = Move::new(from, to, None).unwrap();

        // Verify initial state
        assert_eq!(
            board.get_piece(from),
            Some(Piece::new(PieceType::Pawn, Color::White))
        );
        assert_eq!(board.get_piece(to), None);
        assert_eq!(board.active_color(), Color::White);

        // Make the move
        let result = board.make_move(pawn_move);
        assert!(result.is_ok(), "Simple pawn move should succeed");

        // Verify piece moved and square cleared
        assert_eq!(board.get_piece(from), None, "Source square should be empty");
        assert_eq!(
            board.get_piece(to),
            Some(Piece::new(PieceType::Pawn, Color::White)),
            "Destination square should contain the pawn"
        );
    }

    #[test]
    fn test_piece_capture() {
        let mut board = Board::new();

        // Set up a scenario where White can capture Black
        // Place a white knight on d4 and a black pawn on f5
        let knight_pos = Position::new_unchecked(3, 3); // d4
        let pawn_pos = Position::new_unchecked(5, 4); // f5

        board
            .set_piece(
                knight_pos,
                Some(Piece::new(PieceType::Knight, Color::White)),
            )
            .unwrap();
        board
            .set_piece(pawn_pos, Some(Piece::new(PieceType::Pawn, Color::Black)))
            .unwrap();

        // Create capture move from d4 to f5
        let capture_move = Move::new(knight_pos, pawn_pos, None).unwrap();

        // Verify initial state
        assert_eq!(
            board.get_piece(knight_pos),
            Some(Piece::new(PieceType::Knight, Color::White))
        );
        assert_eq!(
            board.get_piece(pawn_pos),
            Some(Piece::new(PieceType::Pawn, Color::Black))
        );

        // Make the capture move
        let result = board.make_move(capture_move);
        assert!(result.is_ok(), "Capture move should succeed");

        // Verify captured piece removed and capturing piece moved
        assert_eq!(
            board.get_piece(knight_pos),
            None,
            "Source square should be empty"
        );
        assert_eq!(
            board.get_piece(pawn_pos),
            Some(Piece::new(PieceType::Knight, Color::White)),
            "Destination square should contain the capturing knight"
        );
    }

    #[test]
    fn test_active_color_switching() {
        let mut board = Board::new();

        // Verify initial state is White to move
        assert_eq!(board.active_color(), Color::White);

        // Make a white move (e2-e4)
        let white_move = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        board.make_move(white_move).unwrap();

        // Verify active color switched to Black
        assert_eq!(board.active_color(), Color::Black);

        // Make a black move (e7-e5)
        let black_move = Move::new(
            Position::new_unchecked(4, 6), // e7
            Position::new_unchecked(4, 4), // e5
            None,
        )
        .unwrap();

        board.make_move(black_move).unwrap();

        // Verify active color switched back to White
        assert_eq!(board.active_color(), Color::White);
    }

    #[test]
    fn test_move_counter_updates() {
        let mut board = Board::new();

        // Verify initial move counters
        assert_eq!(board.fullmove_number(), 1);
        assert_eq!(board.halfmove_clock(), 0);

        // Make a white move (e2-e4)
        let white_move = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        board.make_move(white_move).unwrap();

        // After white's move, fullmove_number should still be 1
        assert_eq!(
            board.fullmove_number(),
            1,
            "Fullmove number should not increment after White's move"
        );

        // Make a black move (e7-e5)
        let black_move = Move::new(
            Position::new_unchecked(4, 6), // e7
            Position::new_unchecked(4, 4), // e5
            None,
        )
        .unwrap();

        board.make_move(black_move).unwrap();

        // After black's move, fullmove_number should increment to 2
        assert_eq!(
            board.fullmove_number(),
            2,
            "Fullmove number should increment after Black's move"
        );
    }

    #[test]
    fn test_move_validation_empty_square() {
        let mut board = Board::new();

        // Try to move from an empty square (e4)
        let empty_square_move = Move::new(
            Position::new_unchecked(4, 3), // e4 (empty)
            Position::new_unchecked(4, 4), // e5
            None,
        )
        .unwrap();

        let result = board.make_move(empty_square_move);
        assert!(result.is_err(), "Move from empty square should fail");

        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(
                    msg.contains("No piece") || msg.contains("empty"),
                    "Error should mention no piece or empty square: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }

    #[test]
    fn test_move_validation_wrong_color() {
        let mut board = Board::new();

        // White to move, try to move a black piece (e7-e5)
        let wrong_color_move = Move::new(
            Position::new_unchecked(4, 6), // e7 (black pawn)
            Position::new_unchecked(4, 4), // e5
            None,
        )
        .unwrap();

        assert_eq!(board.active_color(), Color::White);

        let result = board.make_move(wrong_color_move);
        assert!(result.is_err(), "Move of wrong color piece should fail");

        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(
                    msg.contains("wrong color")
                        || msg.contains("not your")
                        || msg.contains("White")
                        || msg.contains("Black"),
                    "Error should mention wrong color: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }
}

#[cfg(test)]
mod move_validation_error_tests {
    use super::*;

    #[test]
    fn test_move_to_out_of_bounds_destination() {
        let mut board = Board::new();

        // Try to move a piece to an out-of-bounds destination
        let out_of_bounds_moves = [
            // File out of bounds
            Move::new(
                Position::new_unchecked(4, 1), // e2 (valid source)
                Position { file: 8, rank: 3 }, // out of bounds file
                None,
            )
            .unwrap(),
            // Rank out of bounds
            Move::new(
                Position::new_unchecked(4, 1), // e2 (valid source)
                Position { file: 4, rank: 8 }, // out of bounds rank
                None,
            )
            .unwrap(),
            // Both out of bounds
            Move::new(
                Position::new_unchecked(4, 1), // e2 (valid source)
                Position { file: 9, rank: 9 }, // both out of bounds
                None,
            )
            .unwrap(),
        ];

        for mv in out_of_bounds_moves.iter() {
            let result = board.make_move(*mv);
            assert!(
                result.is_err(),
                "Move to out of bounds destination should fail: {:?}",
                mv
            );

            match result.unwrap_err() {
                ChessError::InvalidMove(msg) => {
                    assert!(
                        msg.contains("out of bounds") || msg.contains("bounds"),
                        "Error should mention bounds: {}",
                        msg
                    );
                }
                other => panic!("Expected InvalidMove error, got: {:?}", other),
            }
        }
    }

    #[test]
    fn test_self_capture_attempt() {
        let mut board = Board::new();

        // Try to capture own piece - white pawn trying to "capture" white knight
        let self_capture_move = Move::new(
            Position::new_unchecked(4, 1), // e2 (white pawn)
            Position::new_unchecked(6, 0), // g1 (white knight)
            None,
        )
        .unwrap();

        let result = board.make_move(self_capture_move);
        assert!(result.is_err(), "Self-capture should fail");

        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(
                    msg.contains("own piece") || msg.contains("capture"),
                    "Error should mention own piece or capture: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }

    #[test]
    fn test_move_validation_multiple_error_scenarios() {
        let mut board = Board::new();

        // Test various invalid move scenarios
        let invalid_moves = [
            // From empty square to empty square
            (
                Move::new(
                    Position::new_unchecked(4, 3), // e4 (empty)
                    Position::new_unchecked(4, 4), // e5 (empty)
                    None,
                )
                .unwrap(),
                "empty source",
            ),
            // Wrong color piece
            (
                Move::new(
                    Position::new_unchecked(4, 6), // e7 (black pawn)
                    Position::new_unchecked(4, 5), // e6
                    None,
                )
                .unwrap(),
                "wrong color",
            ),
            // Self capture
            (
                Move::new(
                    Position::new_unchecked(1, 0), // b1 (white knight)
                    Position::new_unchecked(0, 0), // a1 (white rook)
                    None,
                )
                .unwrap(),
                "self capture",
            ),
        ];

        for (mv, description) in invalid_moves.iter() {
            let result = board.make_move(*mv);
            assert!(
                result.is_err(),
                "Move should fail for {}: {:?}",
                description,
                mv
            );

            // Verify it's an InvalidMove error
            match result.unwrap_err() {
                ChessError::InvalidMove(_) => {
                    // Expected error type
                }
                other => panic!(
                    "Expected InvalidMove error for {}, got: {:?}",
                    description, other
                ),
            }
        }
    }

    #[test]
    fn test_invalid_promotion_moves() {
        let mut board = Board::new();

        // Set up a white pawn on 7th rank for promotion testing
        let pawn_pos = Position::new_unchecked(4, 6); // e7
        board
            .set_piece(pawn_pos, Some(Piece::new(PieceType::Pawn, Color::White)))
            .unwrap();

        // Test promotion with non-pawn piece
        board
            .set_piece(
                Position::new_unchecked(3, 3), // d4
                Some(Piece::new(PieceType::Knight, Color::White)),
            )
            .unwrap();

        let invalid_promotion_move = Move::new(
            Position::new_unchecked(3, 3), // d4 (knight)
            Position::new_unchecked(3, 4), // d5
            Some(PieceType::Queen),        // trying to promote a knight
        )
        .unwrap();

        let result = board.make_move(invalid_promotion_move);
        assert!(result.is_err(), "Non-pawn promotion should fail");

        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(
                    msg.contains("pawn") || msg.contains("promotion"),
                    "Error should mention pawn or promotion: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }

    #[test]
    fn test_missing_required_promotion() {
        let mut board = Board::new();

        // Set up a white pawn that should promote
        let pawn_pos = Position::new_unchecked(4, 6); // e7
        board
            .set_piece(pawn_pos, Some(Piece::new(PieceType::Pawn, Color::White)))
            .unwrap();

        // Try to move pawn to 8th rank without promotion
        let missing_promotion_move = Move::new(
            pawn_pos,                      // e7
            Position::new_unchecked(4, 7), // e8 (promotion rank)
            None,                          // missing promotion
        )
        .unwrap();

        let result = board.make_move(missing_promotion_move);
        assert!(result.is_err(), "Missing promotion should fail");

        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(
                    msg.contains("promotion") || msg.contains("required"),
                    "Error should mention promotion required: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }

    #[test]
    fn test_invalid_promotion_rank() {
        let mut board = Board::new();

        // Set up a white pawn not on promotion rank
        let pawn_pos = Position::new_unchecked(4, 4); // e5
        board
            .set_piece(pawn_pos, Some(Piece::new(PieceType::Pawn, Color::White)))
            .unwrap();

        // Try to promote pawn not reaching promotion rank
        let invalid_rank_promotion = Move::new(
            pawn_pos,                      // e5
            Position::new_unchecked(4, 5), // e6 (not promotion rank)
            Some(PieceType::Queen),        // trying to promote
        )
        .unwrap();

        let result = board.make_move(invalid_rank_promotion);
        assert!(result.is_err(), "Promotion on wrong rank should fail");

        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(
                    msg.contains("promotion") && msg.contains("rank"),
                    "Error should mention promotion and rank: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }

    #[test]
    fn test_error_message_quality() {
        let mut board = Board::new();

        // Test that error messages are informative
        let test_cases = [
            (
                Move::new(
                    Position::new_unchecked(4, 3), // e4 (empty)
                    Position::new_unchecked(4, 4), // e5
                    None,
                )
                .unwrap(),
                vec!["No piece", "source", "position"],
            ),
            (
                Move::new(
                    Position::new_unchecked(4, 6), // e7 (black pawn, wrong color)
                    Position::new_unchecked(4, 5), // e6
                    None,
                )
                .unwrap(),
                vec!["Cannot move", "color", "turn"],
            ),
        ];

        for (mv, expected_words) in test_cases.iter() {
            let result = board.make_move(*mv);
            assert!(result.is_err());

            if let Err(ChessError::InvalidMove(msg)) = result {
                let msg_lower = msg.to_lowercase();
                let has_expected_word = expected_words
                    .iter()
                    .any(|word| msg_lower.contains(&word.to_lowercase()));
                assert!(
                    has_expected_word,
                    "Error message '{}' should contain one of: {:?}",
                    msg, expected_words
                );
            }
        }
    }
}

#[cfg(test)]
mod move_validation_placeholder_tests {
    use super::*;

    #[test]
    fn test_is_legal_move_placeholder_behavior() {
        let board = Board::new();

        // Test that is_legal_move returns true for all moves (placeholder implementation)
        let test_moves = [
            // Valid-looking moves
            Move::new(
                Position::new_unchecked(4, 1), // e2
                Position::new_unchecked(4, 3), // e4
                None,
            )
            .unwrap(),
            // Invalid-looking moves (but should still return true in placeholder)
            Move::new(
                Position::new_unchecked(0, 0), // a1
                Position::new_unchecked(7, 7), // h8
                None,
            )
            .unwrap(),
            // Promotion move
            Move::new(
                Position::new_unchecked(4, 6), // e7
                Position::new_unchecked(4, 7), // e8
                Some(PieceType::Queen),
            )
            .unwrap(),
        ];

        for test_move in &test_moves {
            assert!(
                board.is_legal_move(*test_move),
                "Placeholder is_legal_move should return true for move: {}",
                test_move
            );
        }
    }

    #[test]
    fn test_is_legal_move_placeholder_consistency() {
        let board = Board::new();

        // Create the same move multiple times and verify consistent behavior
        let move1 = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        let move2 = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        assert_eq!(move1, move2, "Identical moves should be equal");
        assert_eq!(
            board.is_legal_move(move1),
            board.is_legal_move(move2),
            "is_legal_move should return consistent results for identical moves"
        );

        // Multiple calls should return the same result
        let first_call = board.is_legal_move(move1);
        let second_call = board.is_legal_move(move1);
        assert_eq!(
            first_call, second_call,
            "Multiple calls should return same result"
        );
        assert!(first_call, "Placeholder should return true");
    }
}

#[cfg(test)]
mod move_application_edge_cases {
    use super::*;

    #[test]
    fn test_move_to_same_square() {
        // Try to move a piece to the same square it's already on
        let same_square_move = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 1), // e2 (same position)
            None,
        );

        // This should fail at move creation level
        assert!(
            same_square_move.is_err(),
            "Move to same square should fail during creation"
        );

        match same_square_move.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(
                    msg.contains("same"),
                    "Error should mention same positions: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }

    #[test]
    fn test_board_state_after_failed_move() {
        let mut board = Board::new();
        let initial_active_color = board.active_color();
        let initial_fullmove = board.fullmove_number();
        let initial_halfmove = board.halfmove_clock();

        // Try an invalid move (from empty square)
        let invalid_move = Move::new(
            Position::new_unchecked(4, 3), // e4 (empty)
            Position::new_unchecked(4, 4), // e5
            None,
        )
        .unwrap();

        let result = board.make_move(invalid_move);
        assert!(result.is_err(), "Invalid move should fail");

        // Verify board state unchanged after failed move
        assert_eq!(
            board.active_color(),
            initial_active_color,
            "Active color should be unchanged"
        );
        assert_eq!(
            board.fullmove_number(),
            initial_fullmove,
            "Fullmove number should be unchanged"
        );
        assert_eq!(
            board.halfmove_clock(),
            initial_halfmove,
            "Halfmove clock should be unchanged"
        );

        // Verify piece positions unchanged
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 1)),
            Some(Piece::new(PieceType::Pawn, Color::White)),
            "Original pieces should remain"
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 3)),
            None,
            "Empty squares should remain empty"
        );
    }
}
