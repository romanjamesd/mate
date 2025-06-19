use mate::chess::{Board, Color, Move, Piece, PieceType, Position};

#[cfg(test)]
mod hash_consistency_tests {
    use super::*;

    #[test]
    fn test_identical_positions_produce_identical_hashes() {
        // Create two identical boards using Board::new()
        let board1 = Board::new();
        let board2 = Board::new();

        // Both should have identical hashes
        let hash1 = board1.hash_state();
        let hash2 = board2.hash_state();

        assert_eq!(
            hash1, hash2,
            "Two boards with identical starting positions should have identical hashes"
        );

        // Test with a more complex position - apply the same move to both boards
        let mut board3 = Board::new();
        let mut board4 = Board::new();

        let pawn_move = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        board3.make_move(pawn_move).unwrap();
        board4.make_move(pawn_move).unwrap();

        let hash3 = board3.hash_state();
        let hash4 = board4.hash_state();

        assert_eq!(
            hash3, hash4,
            "Two boards with identical positions after same move should have identical hashes"
        );
    }

    #[test]
    fn test_different_positions_produce_different_hashes() {
        let starting_board = Board::new();
        let mut moved_board = Board::new();

        // Make a move to create a different position
        let pawn_move = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        moved_board.make_move(pawn_move).unwrap();

        let starting_hash = starting_board.hash_state();
        let moved_hash = moved_board.hash_state();

        assert_ne!(
            starting_hash, moved_hash,
            "Starting position and position after one move should have different hashes"
        );

        // Test with different moves producing different hashes
        let mut board_move1 = Board::new();
        let mut board_move2 = Board::new();

        let move1 = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        let move2 = Move::new(
            Position::new_unchecked(3, 1), // d2
            Position::new_unchecked(3, 3), // d4
            None,
        )
        .unwrap();

        board_move1.make_move(move1).unwrap();
        board_move2.make_move(move2).unwrap();

        let hash_move1 = board_move1.hash_state();
        let hash_move2 = board_move2.hash_state();

        assert_ne!(
            hash_move1, hash_move2,
            "Different moves should produce different board hashes"
        );
    }

    #[test]
    fn test_hash_stability() {
        let board = Board::new();

        // Call hash_state multiple times and verify consistent results
        let hash1 = board.hash_state();
        let hash2 = board.hash_state();
        let hash3 = board.hash_state();

        assert_eq!(hash1, hash2, "Multiple calls should return same hash");
        assert_eq!(hash2, hash3, "Multiple calls should return same hash");
        assert_eq!(hash1, hash3, "Multiple calls should return same hash");

        // Test stability after modifications that don't change the hash
        let mut mutable_board = Board::new();
        let initial_hash = mutable_board.hash_state();

        // Make a move and then undo-like operation (different piece placement)
        let move_out = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        mutable_board.make_move(move_out).unwrap();
        let moved_hash = mutable_board.hash_state();

        // Hash should be different after move
        assert_ne!(initial_hash, moved_hash, "Hash should change after move");

        // Multiple calls on moved board should be stable
        let moved_hash2 = mutable_board.hash_state();
        assert_eq!(
            moved_hash, moved_hash2,
            "Hash should be stable on moved board"
        );
    }
}

#[cfg(test)]
mod hash_state_inclusion_tests {
    use super::*;

    #[test]
    fn test_hash_includes_active_color() {
        // Create two boards with different active colors
        // Start with same position but different player to move
        let mut board_white = Board::new();
        let mut board_black = Board::new();

        // Make a move with white, then with black to get different active colors
        let white_move = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        board_white.make_move(white_move).unwrap();
        // board_white now has Black to move

        let black_move = Move::new(
            Position::new_unchecked(4, 6), // e7
            Position::new_unchecked(4, 4), // e5
            None,
        )
        .unwrap();

        board_black.make_move(white_move).unwrap(); // White moves
        board_black.make_move(black_move).unwrap(); // Black moves
                                                    // board_black now has White to move

        // At this point, both boards should have the same pieces but different active colors
        assert_eq!(board_white.active_color(), Color::Black);
        assert_eq!(board_black.active_color(), Color::White);

        let hash_white_turn = board_white.hash_state();
        let hash_black_turn = board_black.hash_state();

        assert_ne!(
            hash_white_turn, hash_black_turn,
            "Boards with different active colors should have different hashes"
        );
    }

    #[test]
    fn test_hash_includes_move_counters() {
        // Test that move counters affect hash by comparing boards at different stages
        let board_early = Board::new();
        let mut board_later = Board::new();

        // Early board: fullmove = 1, halfmove = 0
        assert_eq!(board_early.fullmove_number(), 1);
        assert_eq!(board_early.halfmove_clock(), 0);

        // Later board: make some moves to advance move counters
        let white_move = Move::new(
            Position::new_unchecked(4, 1), // e2
            Position::new_unchecked(4, 3), // e4
            None,
        )
        .unwrap();

        let black_move = Move::new(
            Position::new_unchecked(4, 6), // e7
            Position::new_unchecked(4, 4), // e5
            None,
        )
        .unwrap();

        board_later.make_move(white_move).unwrap(); // fullmove still 1, Black to move
        board_later.make_move(black_move).unwrap(); // fullmove becomes 2, White to move

        assert_eq!(board_later.fullmove_number(), 2);
        assert_eq!(board_later.active_color(), Color::White);

        // Compare hashes - they should be different due to different move counters and positions
        let hash_early = board_early.hash_state();
        let hash_later = board_later.hash_state();

        assert_ne!(
            hash_early, hash_later,
            "Boards with different move counters and positions should have different hashes"
        );

        // Test halfmove clock effect
        let mut board_with_pawn_moves = Board::new();
        let mut board_with_piece_moves = Board::new();

        // Board 1: Make a pawn move (resets halfmove clock)
        board_with_pawn_moves.make_move(white_move).unwrap();

        // Board 2: Make a knight move (increments halfmove clock)
        let knight_move = Move::new(
            Position::new_unchecked(6, 0), // g1
            Position::new_unchecked(5, 2), // f3
            None,
        )
        .unwrap();

        board_with_piece_moves.make_move(knight_move).unwrap();

        // Both boards should have different halfmove clocks
        // Pawn move resets to 0, piece move increments to 1
        assert_eq!(board_with_pawn_moves.halfmove_clock(), 0);
        assert_eq!(board_with_piece_moves.halfmove_clock(), 1);

        let hash_pawn = board_with_pawn_moves.hash_state();
        let hash_piece = board_with_piece_moves.hash_state();

        assert_ne!(
            hash_pawn, hash_piece,
            "Boards with different halfmove clocks should have different hashes"
        );
    }

    #[test]
    fn test_hash_includes_piece_positions() {
        let mut board1 = Board::new();
        let mut board2 = Board::new();

        // Modify piece positions directly using set_piece
        // Place an extra white queen on an empty square in board1
        board1
            .set_piece(
                Position::new_unchecked(4, 4), // e5
                Some(Piece::new(PieceType::Queen, Color::White)),
            )
            .unwrap();

        // board2 remains with starting position
        let hash1 = board1.hash_state();
        let hash2 = board2.hash_state();

        assert_ne!(
            hash1, hash2,
            "Boards with different piece positions should have different hashes"
        );

        // Test that identical piece modifications produce identical hashes
        board2
            .set_piece(
                Position::new_unchecked(4, 4), // e5
                Some(Piece::new(PieceType::Queen, Color::White)),
            )
            .unwrap();

        let hash1_after = board1.hash_state();
        let hash2_after = board2.hash_state();

        assert_eq!(
            hash1_after, hash2_after,
            "Boards with identical piece modifications should have identical hashes"
        );
    }
}

#[cfg(test)]
mod hash_practical_usage_tests {
    use super::*;

    #[test]
    fn test_hash_for_position_comparison() {
        // Test that hash can be used for position comparison
        let starting_position = Board::new();
        let mut board = Board::new();

        // Make some moves
        let moves = [
            Move::new(
                Position::new_unchecked(4, 1), // e2
                Position::new_unchecked(4, 3), // e4
                None,
            )
            .unwrap(),
            Move::new(
                Position::new_unchecked(4, 6), // e7
                Position::new_unchecked(4, 4), // e5
                None,
            )
            .unwrap(),
            Move::new(
                Position::new_unchecked(6, 0), // g1
                Position::new_unchecked(5, 2), // f3
                None,
            )
            .unwrap(),
        ];

        let starting_hash = starting_position.hash_state();
        let mut position_hashes = Vec::new();

        for mov in moves {
            board.make_move(mov).unwrap();
            position_hashes.push(board.hash_state());
        }

        // All hashes should be different from starting position
        for (i, &hash) in position_hashes.iter().enumerate() {
            assert_ne!(
                starting_hash,
                hash,
                "Position after move {} should have different hash from starting position",
                i + 1
            );
        }

        // All position hashes should be different from each other
        for i in 0..position_hashes.len() {
            for j in i + 1..position_hashes.len() {
                assert_ne!(
                    position_hashes[i],
                    position_hashes[j],
                    "Positions after moves {} and {} should have different hashes",
                    i + 1,
                    j + 1
                );
            }
        }
    }

    #[test]
    fn test_hash_for_integrity_checking() {
        let mut board = Board::new();
        let initial_hash = board.hash_state();

        // Make a series of moves
        let moves = [
            Move::new(
                Position::new_unchecked(4, 1), // e2
                Position::new_unchecked(4, 3), // e4
                None,
            )
            .unwrap(),
            Move::new(
                Position::new_unchecked(3, 6), // d7
                Position::new_unchecked(3, 5), // d6
                None,
            )
            .unwrap(),
        ];

        let mut expected_hashes = Vec::new();

        for mov in &moves {
            board.make_move(*mov).unwrap();
            expected_hashes.push(board.hash_state());
        }

        // Create a new board and apply the same moves
        let mut verification_board = Board::new();
        assert_eq!(
            verification_board.hash_state(),
            initial_hash,
            "Fresh board should have same hash as initial board"
        );

        for (i, mov) in moves.iter().enumerate() {
            verification_board.make_move(*mov).unwrap();
            assert_eq!(
                verification_board.hash_state(),
                expected_hashes[i],
                "Board hash should match expected hash after move {}",
                i + 1
            );
        }
    }

    #[test]
    fn test_hash_collision_resistance() {
        // Test that minor differences produce different hashes
        let mut board1 = Board::new();
        let mut board2 = Board::new();

        // Make same moves but in different order (if possible)
        // Move 1: e2-e4, d7-d6
        let sequence1 = [
            Move::new(
                Position::new_unchecked(4, 1), // e2
                Position::new_unchecked(4, 3), // e4
                None,
            )
            .unwrap(),
            Move::new(
                Position::new_unchecked(3, 6), // d7
                Position::new_unchecked(3, 5), // d6
                None,
            )
            .unwrap(),
        ];

        // Move 2: d2-d4, e7-e6
        let sequence2 = [
            Move::new(
                Position::new_unchecked(3, 1), // d2
                Position::new_unchecked(3, 3), // d4
                None,
            )
            .unwrap(),
            Move::new(
                Position::new_unchecked(4, 6), // e7
                Position::new_unchecked(4, 5), // e6
                None,
            )
            .unwrap(),
        ];

        for mov in sequence1 {
            board1.make_move(mov).unwrap();
        }

        for mov in sequence2 {
            board2.make_move(mov).unwrap();
        }

        let hash1 = board1.hash_state();
        let hash2 = board2.hash_state();

        assert_ne!(
            hash1, hash2,
            "Different move sequences should produce different hashes"
        );
    }
}
