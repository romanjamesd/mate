use mate::chess::{Board, Color, Move, Piece, PieceType, Position};

/// Chess Board Integration Tests
/// Tests the complete chess board functionality including FEN parsing,
/// ASCII display, move application, and board state hashing
#[cfg(test)]
mod chess_board_integration_tests {
    use super::*;

    /// Test comprehensive starting position correctness
    /// Validates all aspects of the initial board state
    #[test]
    fn test_starting_position_correctness() {
        let board = Board::new();

        // Validate starting position - White pieces on ranks 1-2
        assert_eq!(
            board.get_piece(Position::new_unchecked(0, 0)), // a1
            Some(Piece::new(PieceType::Rook, Color::White))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(1, 0)), // b1
            Some(Piece::new(PieceType::Knight, Color::White))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(2, 0)), // c1
            Some(Piece::new(PieceType::Bishop, Color::White))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(3, 0)), // d1
            Some(Piece::new(PieceType::Queen, Color::White))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 0)), // e1
            Some(Piece::new(PieceType::King, Color::White))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(5, 0)), // f1
            Some(Piece::new(PieceType::Bishop, Color::White))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(6, 0)), // g1
            Some(Piece::new(PieceType::Knight, Color::White))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(7, 0)), // h1
            Some(Piece::new(PieceType::Rook, Color::White))
        );

        // White pawns on rank 2
        for file in 0..8 {
            assert_eq!(
                board.get_piece(Position::new_unchecked(file, 1)),
                Some(Piece::new(PieceType::Pawn, Color::White))
            );
        }

        // Validate starting position - Black pieces on ranks 7-8
        assert_eq!(
            board.get_piece(Position::new_unchecked(0, 7)), // a8
            Some(Piece::new(PieceType::Rook, Color::Black))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(1, 7)), // b8
            Some(Piece::new(PieceType::Knight, Color::Black))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(2, 7)), // c8
            Some(Piece::new(PieceType::Bishop, Color::Black))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(3, 7)), // d8
            Some(Piece::new(PieceType::Queen, Color::Black))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 7)), // e8
            Some(Piece::new(PieceType::King, Color::Black))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(5, 7)), // f8
            Some(Piece::new(PieceType::Bishop, Color::Black))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(6, 7)), // g8
            Some(Piece::new(PieceType::Knight, Color::Black))
        );
        assert_eq!(
            board.get_piece(Position::new_unchecked(7, 7)), // h8
            Some(Piece::new(PieceType::Rook, Color::Black))
        );

        // Black pawns on rank 7
        for file in 0..8 {
            assert_eq!(
                board.get_piece(Position::new_unchecked(file, 6)),
                Some(Piece::new(PieceType::Pawn, Color::Black))
            );
        }

        // Validate empty squares on ranks 3-6
        for rank in 2..6 {
            for file in 0..8 {
                assert_eq!(
                    board.get_piece(Position::new_unchecked(file, rank)),
                    None,
                    "Square at file {} rank {} should be empty",
                    file,
                    rank
                );
            }
        }

        // Validate game state
        assert_eq!(board.active_color(), Color::White);
        assert_eq!(board.fullmove_number(), 1);
        assert_eq!(board.halfmove_clock(), 0);
    }

    /// Test FEN parsing for multiple well-known chess positions
    #[test]
    fn test_fen_parsing_standard_positions() {
        // Test starting position FEN
        let starting_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let board = Board::from_fen(starting_fen).expect("Failed to parse starting position FEN");

        // Verify it matches Board::new()
        let new_board = Board::new();
        assert_eq!(board.active_color(), new_board.active_color());
        assert_eq!(board.fullmove_number(), new_board.fullmove_number());
        assert_eq!(board.halfmove_clock(), new_board.halfmove_clock());

        // Check all pieces match
        for rank in 0..8 {
            for file in 0..8 {
                let pos = Position::new_unchecked(file, rank);
                assert_eq!(
                    board.get_piece(pos),
                    new_board.get_piece(pos),
                    "Pieces should match at position {}",
                    pos
                );
            }
        }

        // Test position after 1.e4
        let e4_fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        let e4_board = Board::from_fen(e4_fen).expect("Failed to parse e4 position FEN");

        // Verify pawn is on e4
        assert_eq!(
            e4_board.get_piece(Position::new_unchecked(4, 3)), // e4
            Some(Piece::new(PieceType::Pawn, Color::White))
        );
        // Verify e2 is empty
        assert_eq!(
            e4_board.get_piece(Position::new_unchecked(4, 1)), // e2
            None
        );
        // Verify active color is Black
        assert_eq!(e4_board.active_color(), Color::Black);

        // Test position after 1.e4 e5
        let e4_e5_fen = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2";
        let e4_e5_board = Board::from_fen(e4_e5_fen).expect("Failed to parse e4 e5 position FEN");

        // Verify both pawns are in position
        assert_eq!(
            e4_e5_board.get_piece(Position::new_unchecked(4, 3)), // e4
            Some(Piece::new(PieceType::Pawn, Color::White))
        );
        assert_eq!(
            e4_e5_board.get_piece(Position::new_unchecked(4, 4)), // e5
            Some(Piece::new(PieceType::Pawn, Color::Black))
        );
        // Verify active color is White and fullmove is 2
        assert_eq!(e4_e5_board.active_color(), Color::White);
        assert_eq!(e4_e5_board.fullmove_number(), 2);

        // Test empty board position
        let empty_fen = "8/8/8/8/8/8/8/8 w - - 0 1";
        let empty_board = Board::from_fen(empty_fen).expect("Failed to parse empty board FEN");

        // Verify all squares are empty
        for rank in 0..8 {
            for file in 0..8 {
                let pos = Position::new_unchecked(file, rank);
                assert_eq!(
                    empty_board.get_piece(pos),
                    None,
                    "Square {} should be empty",
                    pos
                );
            }
        }
    }

    /// Test ASCII display clarity and formatting
    #[test]
    fn test_ascii_display_clarity() {
        let board = Board::new();
        let ascii = board.to_ascii();

        // Verify the ASCII display contains expected elements
        assert!(ascii.contains("8"), "Display should show rank 8");
        assert!(ascii.contains("1"), "Display should show rank 1");
        assert!(ascii.contains("a"), "Display should show file a");
        assert!(ascii.contains("h"), "Display should show file h");

        // Verify it contains Unicode piece symbols
        assert!(ascii.contains("♜"), "Should contain black rook symbol");
        assert!(ascii.contains("♚"), "Should contain black king symbol");
        assert!(ascii.contains("♖"), "Should contain white rook symbol");
        assert!(ascii.contains("♔"), "Should contain white king symbol");
        assert!(ascii.contains("♟"), "Should contain black pawn symbol");
        assert!(ascii.contains("♙"), "Should contain white pawn symbol");

        // Verify board orientation (rank 8 should appear before rank 1)
        let rank_8_pos = ascii.find("8").expect("Rank 8 should be in display");
        let rank_1_pos = ascii.find("1").expect("Rank 1 should be in display");
        assert!(
            rank_8_pos < rank_1_pos,
            "Rank 8 should appear before rank 1"
        );

        // Test display of a position with some moves
        let e4_fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        let e4_board = Board::from_fen(e4_fen).expect("Failed to parse e4 position");
        let e4_ascii = e4_board.to_ascii();

        // Verify the moved pawn appears in the display
        assert!(e4_ascii.contains("♙"), "Should contain white pawn symbol");

        // The display should be different from starting position
        assert_ne!(ascii, e4_ascii, "Display should change after moves");
    }

    /// Test move application and board state updates
    #[test]
    fn test_move_application_updates() {
        let mut board = Board::new();

        // Test simple pawn move e2-e4
        let e2_e4 = Move::simple(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
        )
        .expect("Failed to create e2-e4 move");

        // Apply the move
        let result = board.make_move(e2_e4);
        assert!(result.is_ok(), "Move e2-e4 should be valid");

        // Verify piece moved
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 3)), // e4
            Some(Piece::new(PieceType::Pawn, Color::White))
        );
        // Verify source square is empty
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 1)), // e2
            None
        );

        // Verify active color switched to Black
        assert_eq!(board.active_color(), Color::Black);

        // Test Black's response e7-e5
        let e7_e5 = Move::simple(
            Position::new_unchecked(4, 6), // e7
            Position::new_unchecked(4, 4), // e5
        )
        .expect("Failed to create e7-e5 move");

        let result = board.make_move(e7_e5);
        assert!(result.is_ok(), "Move e7-e5 should be valid");

        // Verify Black pawn moved
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 4)), // e5
            Some(Piece::new(PieceType::Pawn, Color::Black))
        );
        // Verify source square is empty
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 6)), // e7
            None
        );

        // Verify active color switched back to White
        assert_eq!(board.active_color(), Color::White);
        // Verify fullmove number incremented
        assert_eq!(board.fullmove_number(), 2);

        // Test capture move - White pawn captures Black pawn
        let capture_move = Move::simple(
            Position::new_unchecked(4, 3), // e4
            Position::new_unchecked(4, 4), // e5 (capture)
        )
        .expect("Failed to create capture move");

        let result = board.make_move(capture_move);
        assert!(result.is_ok(), "Capture move should be valid");

        // Verify White pawn is now on e5
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 4)), // e5
            Some(Piece::new(PieceType::Pawn, Color::White))
        );
        // Verify e4 is now empty
        assert_eq!(
            board.get_piece(Position::new_unchecked(4, 3)), // e4
            None
        );
        // Verify halfmove clock reset due to capture
        assert_eq!(board.halfmove_clock(), 0);
    }

    /// Test board state hashing for position comparison and integrity
    #[test]
    fn test_board_state_hashing() {
        // Test identical positions produce identical hashes
        let board1 = Board::new();
        let board2 = Board::new();

        assert_eq!(
            board1.hash_state(),
            board2.hash_state(),
            "Identical starting positions should have same hash"
        );

        // Test hash stability - multiple calls should return same value
        let hash1 = board1.hash_state();
        let hash2 = board1.hash_state();
        assert_eq!(hash1, hash2, "Hash should be stable across multiple calls");

        // Test different positions produce different hashes
        let mut board3 = Board::new();
        let e2_e4 = Move::simple(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
        )
        .expect("Failed to create move");

        board3.make_move(e2_e4).expect("Move should succeed");

        assert_ne!(
            board1.hash_state(),
            board3.hash_state(),
            "Different positions should have different hashes"
        );

        // Test that hash includes all state components
        let board4 = Board::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")
            .expect("Failed to parse FEN");
        let board5 = Board::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e3 0 1")
            .expect("Failed to parse FEN");

        // Same position but different active color should produce different hashes
        assert_ne!(
            board4.hash_state(),
            board5.hash_state(),
            "Different active colors should produce different hashes"
        );

        // Test hash can be used for position comparison
        let starting_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let board_from_fen = Board::from_fen(starting_fen).expect("Failed to parse starting FEN");
        let board_from_new = Board::new();

        assert_eq!(
            board_from_fen.hash_state(),
            board_from_new.hash_state(),
            "FEN-parsed starting position should match Board::new() hash"
        );
    }

    /// Test round-trip FEN conversion integrity
    #[test]
    fn test_fen_roundtrip_integrity() {
        let test_positions = vec![
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // Starting position
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1", // After 1.e4
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2", // After 1.e4 e5
            "8/8/8/8/8/8/8/8 w - - 0 1",                                // Empty board
            "r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1",                     // Castling test position
        ];

        for original_fen in test_positions {
            // Parse FEN to board
            let board = Board::from_fen(original_fen)
                .unwrap_or_else(|_| panic!("Failed to parse FEN: {}", original_fen));

            // Convert back to FEN
            let roundtrip_fen = board.to_fen();

            // Parse the roundtrip FEN
            let roundtrip_board = Board::from_fen(&roundtrip_fen)
                .unwrap_or_else(|_| panic!("Failed to parse roundtrip FEN: {}", roundtrip_fen));

            // Verify boards are equivalent via hash comparison
            assert_eq!(
                board.hash_state(),
                roundtrip_board.hash_state(),
                "FEN roundtrip failed for: {}\nGot: {}",
                original_fen,
                roundtrip_fen
            );
        }
    }

    /// Test comprehensive board validation across all systems
    #[test]
    fn test_comprehensive_board_validation() {
        // Create a board and perform various operations
        let mut board = Board::new();

        // Verify initial state is consistent across all access methods
        let starting_hash = board.hash_state();
        let starting_fen = board.to_fen();
        let starting_ascii = board.to_ascii();

        // Make some moves and verify consistency
        let moves = [
            Move::simple(Position::new_unchecked(4, 1), Position::new_unchecked(4, 3)).unwrap(), // e2-e4
            Move::simple(Position::new_unchecked(4, 6), Position::new_unchecked(4, 4)).unwrap(), // e7-e5
            Move::simple(Position::new_unchecked(6, 0), Position::new_unchecked(5, 2)).unwrap(), // Ng1-f3
            Move::simple(Position::new_unchecked(1, 7), Position::new_unchecked(2, 5)).unwrap(), // Nb8-c6
        ];

        for (i, mv) in moves.iter().enumerate() {
            let prev_hash = board.hash_state();
            board
                .make_move(*mv)
                .unwrap_or_else(|_| panic!("Move {} should be valid", i));
            let new_hash = board.hash_state();

            // Verify hash changed after move
            assert_ne!(prev_hash, new_hash, "Hash should change after move {}", i);

            // Verify FEN can be generated
            let fen = board.to_fen();
            assert!(!fen.is_empty(), "FEN should not be empty after move {}", i);

            // Verify ASCII display can be generated
            let ascii = board.to_ascii();
            assert!(
                !ascii.is_empty(),
                "ASCII display should not be empty after move {}",
                i
            );

            // Verify roundtrip consistency
            let roundtrip_board = Board::from_fen(&fen)
                .unwrap_or_else(|_| panic!("FEN roundtrip should work after move {}", i));
            assert_eq!(
                board.hash_state(),
                roundtrip_board.hash_state(),
                "Roundtrip consistency failed after move {}",
                i
            );
        }

        // Verify final state is different from starting state
        assert_ne!(
            starting_hash,
            board.hash_state(),
            "Final hash should differ from start"
        );
        assert_ne!(
            starting_fen,
            board.to_fen(),
            "Final FEN should differ from start"
        );
        assert_ne!(
            starting_ascii,
            board.to_ascii(),
            "Final ASCII should differ from start"
        );
    }
}
