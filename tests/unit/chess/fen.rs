use mate::chess::{Board, ChessError, Color, Piece, PieceType, Position};

#[cfg(test)]
mod fen_parsing_tests {
    use super::*;

    #[test]
    fn test_standard_starting_position_fen() {
        // Test parsing of standard starting position FEN
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let board = Board::from_fen(fen).expect("Failed to parse standard starting position FEN");

        // Verify the board matches the standard starting position
        let expected_board = Board::new();

        // Check all pieces match
        for rank in 0..8 {
            for file in 0..8 {
                let pos = Position::new_unchecked(file, rank);
                assert_eq!(
                    board.get_piece(pos),
                    expected_board.get_piece(pos),
                    "Piece mismatch at position {}",
                    pos
                );
            }
        }

        // Check game state
        assert_eq!(board.active_color(), Color::White);
        assert_eq!(board.fullmove_number(), 1);
        assert_eq!(board.halfmove_clock(), 0);
    }

    #[test]
    fn test_valid_fen_variations() {
        // Test different valid positions
        let test_cases = vec![
            // After 1.e4 e5
            (
                "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
                Color::White,
                2,
                0,
            ),
            // Empty board with black to move
            ("8/8/8/8/8/8/8/8 b - - 50 100", Color::Black, 100, 50),
            // Board with some pieces
            ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", Color::White, 1, 0),
        ];

        for (fen, expected_color, expected_fullmove, expected_halfmove) in test_cases {
            let board = Board::from_fen(fen).expect(&format!("Failed to parse FEN: {}", fen));

            assert_eq!(
                board.active_color(),
                expected_color,
                "Active color mismatch for FEN: {}",
                fen
            );
            assert_eq!(
                board.fullmove_number(),
                expected_fullmove,
                "Fullmove number mismatch for FEN: {}",
                fen
            );
            assert_eq!(
                board.halfmove_clock(),
                expected_halfmove,
                "Halfmove clock mismatch for FEN: {}",
                fen
            );
        }
    }

    #[test]
    fn test_invalid_fen_wrong_field_count() {
        let invalid_fens = vec![
            // Too few fields
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR",
            "",
            " ",
            // Too many fields
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 extra",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1 extra field",
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with wrong field count: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(msg)) = result {
                assert!(
                    msg.contains("exactly 6 fields") || msg.contains("empty"),
                    "Error message should mention field count or empty string. Got: {}",
                    msg
                );
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_piece_placement() {
        let invalid_fens = vec![
            // Wrong number of ranks
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP w KQkq - 0 1", // 7 ranks instead of 8
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR/extra w KQkq - 0 1", // 9 ranks
            // Invalid characters in piece placement
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNX w KQkq - 0 1", // X is invalid
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBN9 w KQkq - 0 1", // 9 is invalid (max is 8)
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with invalid piece placement: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(_)) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_piece_placement_additional() {
        let invalid_fens = vec![
            // Ranks with too many squares
            "rnbqkbnrr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // 9 pieces in rank
            "9/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",         // number > 8
            // Ranks with too few squares
            "rnbqkbn/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // 7 pieces in rank
            "7/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",       // number < 8 total
            // Mixed invalid combinations
            "rnbq2k1nr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // 2 + pieces > 8
            // Invalid zero
            "rnbqkbn0r/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // 0 is invalid
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with invalid piece placement: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(_)) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_active_color() {
        let invalid_fens = vec![
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1", // x is invalid
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR W KQkq - 0 1", // W should be lowercase
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR B KQkq - 0 1", // B should be lowercase
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR white KQkq - 0 1", // full word invalid
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with invalid active color: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(_)) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_move_counters() {
        let invalid_fens = vec![
            // Non-numeric halfmove clock
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - abc 1",
            // Non-numeric fullmove number
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 def",
            // Negative numbers
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - -1 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 -1",
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with invalid move counters: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(_)) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_move_counters_additional() {
        let invalid_fens = vec![
            // Fullmove number must be >= 1
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 0",
            // Floating point numbers
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 1.5 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1.0",
            // Numbers too large (implementation dependent, but test reasonable bounds)
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 999999999999999999 1",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 999999999999999999",
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with invalid move counters: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(_)) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_empty_fields() {
        let invalid_fens = vec![
            " /pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // empty piece placement
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR   KQkq - 0 1", // empty active color
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w   - 0 1", // empty castling rights
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq   0 1", // empty en passant
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -   1", // empty halfmove
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0  ", // empty fullmove
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with empty fields: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(msg)) = result {
                // split_whitespace() will skip empty fields, so we may get field count errors instead
                assert!(
                    msg.contains("cannot be empty") || msg.contains("exactly 6 fields"),
                    "Error message should mention empty field or field count. Got: {}",
                    msg
                );
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_castling_rights() {
        let invalid_fens = vec![
            // Invalid castling characters
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkqX - 0 1", // X is invalid
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w kqKQ - 0 1",  // wrong order
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq1 - 0 1", // number invalid
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KK - 0 1",    // duplicate K
                                                                         // Note: "Qq" is actually valid according to current implementation - it just checks wrong order
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with invalid castling rights: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(_)) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }

    #[test]
    fn test_invalid_fen_en_passant() {
        let invalid_fens = vec![
            // Invalid en passant square format
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e9 0 1", // rank 9 doesn't exist
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq i3 0 1", // file i doesn't exist
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e0 0 1", // rank 0 doesn't exist
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e 0 1",  // incomplete square
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq 3 0 1",  // missing file
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e33 0 1", // too many characters
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq E3 0 1", // uppercase file
        ];

        for fen in invalid_fens {
            let result = Board::from_fen(fen);
            assert!(
                result.is_err(),
                "Expected error for FEN with invalid en passant: '{}'",
                fen
            );

            if let Err(ChessError::InvalidFen(_)) = result {
                // Expected error type
            } else {
                panic!("Expected InvalidFen error for FEN: '{}'", fen);
            }
        }
    }
}

#[cfg(test)]
mod fen_board_state_error_tests {
    use super::*;

    #[test]
    fn test_board_state_validation_placeholder() {
        // These tests document future board state validation requirements
        // The current implementation only validates FEN syntax, not board state validity
        // These tests are expected to pass (no validation yet) but document what should eventually be validated

        let syntactically_valid_but_chess_invalid = vec![
            // Too many kings - these are syntactically valid FEN but invalid chess positions
            "rnbqkknr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // Two black kings
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBKKNBR w KQkq - 0 1", // Two white kings
            // No kings
            "rnbq1bnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQ1BNR w KQkq - 0 1", // No kings at all
            // Too many pieces of one type
            "qqqqqq1q/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // 7 black queens
            // Pawns on impossible ranks
            "rnbqkbnr/pppppppp/8/8/8/8/pppppppp/RNBQKBNR w KQkq - 0 1", // Black pawns on rank 2
            "rnbqkbnr/PPPPPPPP/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // White pawns on rank 8
            "Pnbqkbnr/pppppppp/8/8/8/8/PPPPPPPp/RNBQKBNR w KQkq - 0 1", // Pawns on ranks 1 and 8
        ];

        for fen in syntactically_valid_but_chess_invalid {
            let result = Board::from_fen(fen);
            // Current implementation should parse these successfully since they are syntactically valid
            // In the future, board state validation should be added to catch these cases
            match result {
                Ok(_) => {
                    // Expected for current implementation - no board state validation yet
                    // println!("Note: FEN '{}' parsed successfully but represents invalid chess position", fen);
                }
                Err(ChessError::BoardStateError(_)) => {
                    // If board state validation is added, this would be the correct error
                }
                Err(other) => {
                    // Unexpected error type
                    panic!("Unexpected error for FEN '{}': {:?}", fen, other);
                }
            }
        }
    }

    #[test]
    fn test_future_board_state_validation_requirements() {
        // This test documents what board state validation should eventually check
        // All of these FEN strings are syntactically valid but represent impossible chess positions

        let impossible_positions = vec![
            // Multiple kings of same color
            (
                "rnbqkknr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "Multiple black kings",
            ),
            // Pawns on back ranks
            (
                "rnbqkbnP/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "White pawn on 8th rank",
            ),
            (
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/pNBQKBNR w KQkq - 0 1",
                "Black pawn on 1st rank",
            ),
            // Too many pieces (impossible even with promotions)
            (
                "qqqqqqq1/qqqqqqqq/8/8/8/8/8/8 w - - 0 1",
                "15 black queens impossible",
            ),
            // No kings
            (
                "rnbq1bnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQ1BNR w KQkq - 0 1",
                "No kings on board",
            ),
        ];

        for (fen, description) in impossible_positions {
            let result = Board::from_fen(fen);

            // For now, document that these parse successfully
            // In the future, these should return BoardStateError
            match result {
                Ok(_) => {
                    // Current expected behavior - only syntax validation
                    // println!("TODO: Should validate board state for: {}", description);
                }
                Err(ChessError::BoardStateError(_)) => {
                    // Future expected behavior when board state validation is implemented
                }
                Err(other) => {
                    panic!(
                        "Unexpected error for {} ({}): {:?}",
                        description, fen, other
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod fen_serialization_tests {
    use super::*;

    #[test]
    fn test_fen_round_trip() {
        let test_fens = vec![
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2",
            "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 10 20",
            "8/8/8/8/8/8/8/8 w - - 50 100",
        ];

        for original_fen in test_fens {
            let board = Board::from_fen(original_fen)
                .expect(&format!("Failed to parse FEN: {}", original_fen));
            let serialized_fen = board.to_fen();

            // Parse the serialized FEN to ensure it's valid
            let round_trip_board = Board::from_fen(&serialized_fen).expect(&format!(
                "Failed to parse serialized FEN: {}",
                serialized_fen
            ));

            // Compare board states
            assert_eq!(
                board.active_color(),
                round_trip_board.active_color(),
                "Active color mismatch for FEN: {}",
                original_fen
            );
            assert_eq!(
                board.fullmove_number(),
                round_trip_board.fullmove_number(),
                "Fullmove number mismatch for FEN: {}",
                original_fen
            );
            assert_eq!(
                board.halfmove_clock(),
                round_trip_board.halfmove_clock(),
                "Halfmove clock mismatch for FEN: {}",
                original_fen
            );

            // Compare piece positions
            for rank in 0..8 {
                for file in 0..8 {
                    let pos = Position::new_unchecked(file, rank);
                    assert_eq!(
                        board.get_piece(pos),
                        round_trip_board.get_piece(pos),
                        "Piece mismatch at position {} for FEN: {}",
                        pos,
                        original_fen
                    );
                }
            }
        }
    }

    #[test]
    fn test_starting_position_serialization() {
        let board = Board::new();
        let fen = board.to_fen();

        // The exact FEN should match the standard starting position
        // Note: castling rights and en passant are currently placeholders ("KQkq" and "-")
        let expected_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        assert_eq!(
            fen, expected_fen,
            "Starting position FEN should match standard notation"
        );
    }

    #[test]
    fn test_empty_square_compression() {
        // Create a board with various empty square patterns
        let mut board = Board::new();

        // Clear some squares to test compression
        // Clear the entire 4th rank (rank 3 in 0-indexed)
        for file in 0..8 {
            let pos = Position::new_unchecked(file, 3);
            board.set_piece(pos, None).unwrap();
        }

        // Clear some squares on the 5th rank to create mixed pattern
        board
            .set_piece(Position::new_unchecked(0, 4), None)
            .unwrap(); // a5
        board
            .set_piece(Position::new_unchecked(1, 4), None)
            .unwrap(); // b5
        board
            .set_piece(Position::new_unchecked(2, 4), None)
            .unwrap(); // c5
                       // Leave d5, e5 as they were (empty from new board)
        board
            .set_piece(Position::new_unchecked(5, 4), None)
            .unwrap(); // f5
        board
            .set_piece(Position::new_unchecked(6, 4), None)
            .unwrap(); // g5
        board
            .set_piece(Position::new_unchecked(7, 4), None)
            .unwrap(); // h5

        let fen = board.to_fen();

        // Verify the FEN contains proper compression
        assert!(
            fen.contains("8"),
            "FEN should contain '8' for the completely empty rank"
        );

        // Parse the FEN back to ensure it's valid
        let parsed_board = Board::from_fen(&fen).expect("Failed to parse generated FEN");

        // Verify the boards match
        for rank in 0..8 {
            for file in 0..8 {
                let pos = Position::new_unchecked(file, rank);
                assert_eq!(
                    board.get_piece(pos),
                    parsed_board.get_piece(pos),
                    "Piece mismatch at position {} after empty square compression",
                    pos
                );
            }
        }
    }

    #[test]
    fn test_piece_placement_generation() {
        // Test specific piece placement patterns
        let mut board = Board::new();

        // Clear the board
        for rank in 0..8 {
            for file in 0..8 {
                board
                    .set_piece(Position::new_unchecked(file, rank), None)
                    .unwrap();
            }
        }

        // Place specific pieces to test FEN generation
        board
            .set_piece(
                Position::new_unchecked(0, 0),
                Some(Piece::new(PieceType::Rook, Color::White)),
            )
            .unwrap(); // a1
        board
            .set_piece(
                Position::new_unchecked(7, 0),
                Some(Piece::new(PieceType::Rook, Color::White)),
            )
            .unwrap(); // h1
        board
            .set_piece(
                Position::new_unchecked(4, 0),
                Some(Piece::new(PieceType::King, Color::White)),
            )
            .unwrap(); // e1

        board
            .set_piece(
                Position::new_unchecked(0, 7),
                Some(Piece::new(PieceType::Rook, Color::Black)),
            )
            .unwrap(); // a8
        board
            .set_piece(
                Position::new_unchecked(7, 7),
                Some(Piece::new(PieceType::Rook, Color::Black)),
            )
            .unwrap(); // h8
        board
            .set_piece(
                Position::new_unchecked(4, 7),
                Some(Piece::new(PieceType::King, Color::Black)),
            )
            .unwrap(); // e8

        let fen = board.to_fen();

        // Should start with "r3k2r" for the 8th rank
        assert!(
            fen.starts_with("r3k2r"),
            "FEN should start with 'r3k2r' for the pattern, got: {}",
            fen
        );

        // Should end with piece placement containing "R3K2R" for the 1st rank
        assert!(
            fen.contains("R3K2R"),
            "FEN should contain 'R3K2R' for the white pieces pattern, got: {}",
            fen
        );

        // Verify round-trip
        let parsed_board = Board::from_fen(&fen).expect("Failed to parse generated FEN");
        for rank in 0..8 {
            for file in 0..8 {
                let pos = Position::new_unchecked(file, rank);
                assert_eq!(
                    board.get_piece(pos),
                    parsed_board.get_piece(pos),
                    "Piece mismatch at position {} after piece placement generation",
                    pos
                );
            }
        }
    }
}
