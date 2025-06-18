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
        
        board.set_piece(knight_pos, Some(Piece::new(PieceType::Knight, Color::White))).unwrap();
        board.set_piece(pawn_pos, Some(Piece::new(PieceType::Pawn, Color::Black))).unwrap();
        
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
        assert_eq!(board.get_piece(knight_pos), None, "Source square should be empty");
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
        ).unwrap();
        
        board.make_move(white_move).unwrap();
        
        // Verify active color switched to Black
        assert_eq!(board.active_color(), Color::Black);
        
        // Make a black move (e7-e5)
        let black_move = Move::new(
            Position::new_unchecked(4, 6), // e7
            Position::new_unchecked(4, 4), // e5
            None,
        ).unwrap();
        
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
        ).unwrap();
        
        board.make_move(white_move).unwrap();
        
        // After white's move, fullmove_number should still be 1
        assert_eq!(board.fullmove_number(), 1, "Fullmove number should not increment after White's move");
        
        // Make a black move (e7-e5)
        let black_move = Move::new(
            Position::new_unchecked(4, 6), // e7
            Position::new_unchecked(4, 4), // e5
            None,
        ).unwrap();
        
        board.make_move(black_move).unwrap();
        
        // After black's move, fullmove_number should increment to 2
        assert_eq!(board.fullmove_number(), 2, "Fullmove number should increment after Black's move");
    }

    #[test]
    fn test_move_validation_empty_square() {
        let mut board = Board::new();
        
        // Try to move from an empty square (e4)
        let empty_square_move = Move::new(
            Position::new_unchecked(4, 3), // e4 (empty)
            Position::new_unchecked(4, 4), // e5
            None,
        ).unwrap();
        
        let result = board.make_move(empty_square_move);
        assert!(result.is_err(), "Move from empty square should fail");
        
        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(msg.contains("No piece") || msg.contains("empty"), 
                       "Error should mention no piece or empty square: {}", msg);
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
        ).unwrap();
        
        assert_eq!(board.active_color(), Color::White);
        
        let result = board.make_move(wrong_color_move);
        assert!(result.is_err(), "Move of wrong color piece should fail");
        
        match result.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(msg.contains("wrong color") || msg.contains("not your") || msg.contains("White") || msg.contains("Black"), 
                       "Error should mention wrong color: {}", msg);
            }
            other => panic!("Expected InvalidMove error, got: {:?}", other),
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
            ).unwrap(),
            // Invalid-looking moves (but should still return true in placeholder)
            Move::new(
                Position::new_unchecked(0, 0), // a1
                Position::new_unchecked(7, 7), // h8
                None,
            ).unwrap(),
            // Promotion move
            Move::new(
                Position::new_unchecked(4, 6), // e7
                Position::new_unchecked(4, 7), // e8
                Some(PieceType::Queen),
            ).unwrap(),
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
        ).unwrap();
        
        let move2 = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        ).unwrap();
        
        assert_eq!(move1, move2, "Identical moves should be equal");
        assert_eq!(
            board.is_legal_move(move1),
            board.is_legal_move(move2),
            "is_legal_move should return consistent results for identical moves"
        );
        
        // Multiple calls should return the same result
        let first_call = board.is_legal_move(move1);
        let second_call = board.is_legal_move(move1);
        assert_eq!(first_call, second_call, "Multiple calls should return same result");
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
        assert!(same_square_move.is_err(), "Move to same square should fail during creation");
        
        match same_square_move.unwrap_err() {
            ChessError::InvalidMove(msg) => {
                assert!(msg.contains("same"), "Error should mention same positions: {}", msg);
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
        ).unwrap();
        
        let result = board.make_move(invalid_move);
        assert!(result.is_err(), "Invalid move should fail");
        
        // Verify board state unchanged after failed move
        assert_eq!(board.active_color(), initial_active_color, "Active color should be unchanged");
        assert_eq!(board.fullmove_number(), initial_fullmove, "Fullmove number should be unchanged");
        assert_eq!(board.halfmove_clock(), initial_halfmove, "Halfmove clock should be unchanged");
        
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