use mate::chess::{Board, Color, Piece, PieceType, Position};

#[cfg(test)]
mod board_creation_tests {
    use super::*;

    #[test]
    fn test_board_new_starting_position() {
        let board = Board::new();

        // Test starting position white pieces on ranks 1-2
        // White back rank (rank 0 in 0-indexed)
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

        // White pawns on rank 1 (rank 1 in 0-indexed)
        for file in 0..8 {
            assert_eq!(
                board.get_piece(Position::new_unchecked(file, 1)),
                Some(Piece::new(PieceType::Pawn, Color::White))
            );
        }
    }

    #[test]
    fn test_board_new_starting_position_black_pieces() {
        let board = Board::new();

        // Test starting position black pieces on ranks 7-8
        // Black back rank (rank 7 in 0-indexed)
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

        // Black pawns on rank 6 (rank 6 in 0-indexed)
        for file in 0..8 {
            assert_eq!(
                board.get_piece(Position::new_unchecked(file, 6)),
                Some(Piece::new(PieceType::Pawn, Color::Black))
            );
        }
    }

    #[test]
    fn test_board_new_empty_squares() {
        let board = Board::new();

        // Test empty squares on ranks 3-6 (ranks 2-5 in 0-indexed)
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
    }

    #[test]
    fn test_board_new_game_state() {
        let board = Board::new();

        // Test game state initialization
        assert_eq!(board.active_color(), Color::White);
        assert_eq!(board.fullmove_number(), 1);
        assert_eq!(board.halfmove_clock(), 0);
    }

    #[test]
    fn test_board_default_same_as_new() {
        let board_new = Board::new();
        let board_default = Board::default();

        // Verify default() produces same result as new()
        assert_eq!(board_new.active_color(), board_default.active_color());
        assert_eq!(board_new.fullmove_number(), board_default.fullmove_number());
        assert_eq!(board_new.halfmove_clock(), board_default.halfmove_clock());

        // Check all squares are identical
        for rank in 0..8 {
            for file in 0..8 {
                let pos = Position::new_unchecked(file, rank);
                assert_eq!(
                    board_new.get_piece(pos),
                    board_default.get_piece(pos),
                    "Pieces should match at position {}",
                    pos
                );
            }
        }
    }
}

#[cfg(test)]
mod board_access_tests {
    use super::*;

    #[test]
    fn test_get_piece_starting_position() {
        let board = Board::new();

        // Test getting pieces from starting position returns correct pieces
        let white_king_pos = Position::new_unchecked(4, 0); // e1
        let white_king = board.get_piece(white_king_pos);
        assert_eq!(white_king, Some(Piece::new(PieceType::King, Color::White)));

        let black_queen_pos = Position::new_unchecked(3, 7); // d8
        let black_queen = board.get_piece(black_queen_pos);
        assert_eq!(
            black_queen,
            Some(Piece::new(PieceType::Queen, Color::Black))
        );

        let empty_pos = Position::new_unchecked(4, 4); // e5
        let empty_square = board.get_piece(empty_pos);
        assert_eq!(empty_square, None);
    }

    #[test]
    fn test_set_piece_on_empty_square() {
        let mut board = Board::new();
        let pos = Position::new_unchecked(4, 4); // e5

        // Verify square is initially empty
        assert_eq!(board.get_piece(pos), None);

        // Set piece on empty square
        let piece = Piece::new(PieceType::Knight, Color::White);
        let result = board.set_piece(pos, Some(piece));
        assert!(result.is_ok());

        // Verify piece was placed correctly
        assert_eq!(board.get_piece(pos), Some(piece));
    }

    #[test]
    fn test_set_piece_to_none_clears_square() {
        let mut board = Board::new();
        let pos = Position::new_unchecked(4, 0); // e1 - has white king initially

        // Verify square has piece initially
        assert!(board.get_piece(pos).is_some());

        // Clear the square
        let result = board.set_piece(pos, None);
        assert!(result.is_ok());

        // Verify square is now empty
        assert_eq!(board.get_piece(pos), None);
    }

    #[test]
    fn test_set_piece_replaces_existing() {
        let mut board = Board::new();
        let pos = Position::new_unchecked(0, 0); // a1 - has white rook initially

        // Verify initial piece
        assert_eq!(
            board.get_piece(pos),
            Some(Piece::new(PieceType::Rook, Color::White))
        );

        // Replace with different piece
        let new_piece = Piece::new(PieceType::Queen, Color::Black);
        let result = board.set_piece(pos, Some(new_piece));
        assert!(result.is_ok());

        // Verify piece was replaced
        assert_eq!(board.get_piece(pos), Some(new_piece));
    }

    #[test]
    fn test_bounds_checking_get_piece() {
        let board = Board::new();

        // Test valid bounds return pieces or None
        assert!(board.get_piece(Position::new_unchecked(0, 0)).is_some()); // a1
        assert!(board.get_piece(Position::new_unchecked(7, 7)).is_some()); // h8
        assert!(board.get_piece(Position::new_unchecked(4, 4)).is_none()); // e5

        // Test invalid bounds return None (positions are validated at creation)
        // Note: Position::new() validates bounds, so we test the board's get_piece bounds checking
        // by creating positions that would be out of bounds if passed directly

        // Since Position::new_unchecked bypasses validation, we can test board's internal bounds checking
        let invalid_file_pos = Position { file: 8, rank: 0 };
        assert_eq!(board.get_piece(invalid_file_pos), None);

        let invalid_rank_pos = Position { file: 0, rank: 8 };
        assert_eq!(board.get_piece(invalid_rank_pos), None);

        let invalid_both_pos = Position { file: 9, rank: 9 };
        assert_eq!(board.get_piece(invalid_both_pos), None);
    }

    #[test]
    fn test_bounds_checking_set_piece() {
        let mut board = Board::new();
        let piece = Piece::new(PieceType::Knight, Color::White);

        // Test valid bounds work
        let valid_pos = Position::new_unchecked(3, 3);
        assert!(board.set_piece(valid_pos, Some(piece)).is_ok());

        // Test invalid bounds return error
        let invalid_file_pos = Position { file: 8, rank: 0 };
        let result = board.set_piece(invalid_file_pos, Some(piece));
        assert!(result.is_err());

        let invalid_rank_pos = Position { file: 0, rank: 8 };
        let result = board.set_piece(invalid_rank_pos, Some(piece));
        assert!(result.is_err());

        let invalid_both_pos = Position { file: 9, rank: 9 };
        let result = board.set_piece(invalid_both_pos, Some(piece));
        assert!(result.is_err());
    }

    #[test]
    fn test_comprehensive_starting_position_verification() {
        let board = Board::new();

        // Define expected starting position by rank
        let expected_pieces = [
            // Rank 0 (rank 1): White back rank
            [
                Some(Piece::new(PieceType::Rook, Color::White)), // a1
                Some(Piece::new(PieceType::Knight, Color::White)), // b1
                Some(Piece::new(PieceType::Bishop, Color::White)), // c1
                Some(Piece::new(PieceType::Queen, Color::White)), // d1
                Some(Piece::new(PieceType::King, Color::White)), // e1
                Some(Piece::new(PieceType::Bishop, Color::White)), // f1
                Some(Piece::new(PieceType::Knight, Color::White)), // g1
                Some(Piece::new(PieceType::Rook, Color::White)), // h1
            ],
            // Rank 1 (rank 2): White pawns
            [Some(Piece::new(PieceType::Pawn, Color::White)); 8],
            // Ranks 2-5 (ranks 3-6): Empty squares
            [None; 8],
            [None; 8],
            [None; 8],
            [None; 8],
            // Rank 6 (rank 7): Black pawns
            [Some(Piece::new(PieceType::Pawn, Color::Black)); 8],
            // Rank 7 (rank 8): Black back rank
            [
                Some(Piece::new(PieceType::Rook, Color::Black)), // a8
                Some(Piece::new(PieceType::Knight, Color::Black)), // b8
                Some(Piece::new(PieceType::Bishop, Color::Black)), // c8
                Some(Piece::new(PieceType::Queen, Color::Black)), // d8
                Some(Piece::new(PieceType::King, Color::Black)), // e8
                Some(Piece::new(PieceType::Bishop, Color::Black)), // f8
                Some(Piece::new(PieceType::Knight, Color::Black)), // g8
                Some(Piece::new(PieceType::Rook, Color::Black)), // h8
            ],
        ];

        // Verify every square matches expected starting position
        for rank in 0..8 {
            for file in 0..8 {
                let pos = Position::new_unchecked(file, rank);
                let actual_piece = board.get_piece(pos);
                let expected_piece = expected_pieces[rank as usize][file as usize];

                assert_eq!(
                    actual_piece, expected_piece,
                    "Piece mismatch at {} (file: {}, rank: {}). Expected: {:?}, Got: {:?}",
                    pos, file, rank, expected_piece, actual_piece
                );
            }
        }
    }
}
