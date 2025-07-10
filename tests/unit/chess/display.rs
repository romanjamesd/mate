use mate::chess::{Board, Color, Piece, PieceType, Position};

#[cfg(test)]
mod ascii_display_tests {
    use super::*;

    #[test]
    fn test_starting_position_display() {
        let board = Board::new();
        let ascii = board.to_ascii();

        // Verify the display contains the correct Unicode piece symbols
        // White pieces (should appear on ranks 1-2, which are bottom of the display)
        assert!(
            ascii.contains("♔"),
            "Display should contain white king symbol ♔"
        );
        assert!(
            ascii.contains("♕"),
            "Display should contain white queen symbol ♕"
        );
        assert!(
            ascii.contains("♖"),
            "Display should contain white rook symbol ♖"
        );
        assert!(
            ascii.contains("♗"),
            "Display should contain white bishop symbol ♗"
        );
        assert!(
            ascii.contains("♘"),
            "Display should contain white knight symbol ♘"
        );
        assert!(
            ascii.contains("♙"),
            "Display should contain white pawn symbol ♙"
        );

        // Black pieces (should appear on ranks 7-8, which are top of the display)
        assert!(
            ascii.contains("♚"),
            "Display should contain black king symbol ♚"
        );
        assert!(
            ascii.contains("♛"),
            "Display should contain black queen symbol ♛"
        );
        assert!(
            ascii.contains("♜"),
            "Display should contain black rook symbol ♜"
        );
        assert!(
            ascii.contains("♝"),
            "Display should contain black bishop symbol ♝"
        );
        assert!(
            ascii.contains("♞"),
            "Display should contain black knight symbol ♞"
        );
        assert!(
            ascii.contains("♟"),
            "Display should contain black pawn symbol ♟"
        );

        // Verify coordinate labels are present
        assert!(
            ascii.contains("a b c d e f g h"),
            "Display should contain file labels a-h"
        );
        for rank in 1..=8 {
            assert!(
                ascii.contains(&rank.to_string()),
                "Display should contain rank label {}",
                rank
            );
        }
    }

    #[test]
    fn test_board_orientation() {
        let board = Board::new();
        let ascii = board.to_ascii();

        let lines: Vec<&str> = ascii.lines().collect();

        // Verify rank 8 appears at the top (after file labels)
        // The structure should be:
        // Line 0: "  a b c d e f g h"
        // Line 1: "8 ♜ ♞ ♝ ♛ ♚ ♝ ♞ ♜ 8"
        // Line 2: "7 ♟ ♟ ♟ ♟ ♟ ♟ ♟ ♟ 7"
        // ...
        // Line 8: "1 ♖ ♘ ♗ ♕ ♔ ♗ ♘ ♖ 1"
        // Line 9: "  a b c d e f g h"

        assert_eq!(lines.len(), 10, "Display should have exactly 10 lines");

        // Check top file labels
        assert_eq!(
            lines[0], "  a b c d e f g h",
            "First line should be file labels"
        );

        // Check rank 8 (black back rank) is second line
        assert!(
            lines[1].starts_with("8 "),
            "Second line should start with rank 8"
        );
        assert!(
            lines[1].ends_with(" 8"),
            "Second line should end with rank 8"
        );
        assert!(lines[1].contains("♚"), "Rank 8 should contain black king ♚");

        // Check rank 1 (white back rank) is near the bottom
        assert!(
            lines[8].starts_with("1 "),
            "Ninth line should start with rank 1"
        );
        assert!(
            lines[8].ends_with(" 1"),
            "Ninth line should end with rank 1"
        );
        assert!(lines[8].contains("♔"), "Rank 1 should contain white king ♔");

        // Check bottom file labels
        assert_eq!(
            lines[9], "  a b c d e f g h",
            "Last line should be file labels"
        );
    }

    #[test]
    fn test_coordinate_labels() {
        let board = Board::new();
        let ascii = board.to_ascii();

        // Verify files a-h are properly labeled
        let expected_files = vec!['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
        for file in expected_files {
            assert!(
                ascii.contains(file),
                "Display should contain file label '{}'",
                file
            );
        }

        // Verify ranks 1-8 are properly labeled
        for rank in 1..=8 {
            // Each rank should appear twice (left and right side)
            let rank_str = rank.to_string();
            let rank_count = ascii.matches(&rank_str).count();
            assert!(
                rank_count >= 2,
                "Rank {} should appear at least twice (left and right labels), found {} occurrences",
                rank,
                rank_count
            );
        }

        // Verify the structure includes proper spacing
        let lines: Vec<&str> = ascii.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if i == 0 || i == lines.len() - 1 {
                // File label lines should start with two spaces
                assert!(
                    line.starts_with("  "),
                    "File label line should start with two spaces: '{}'",
                    line
                );
            } else {
                // Rank lines should start with rank number and space
                let rank_num = 9 - i; // Since rank 8 is line 1, rank 7 is line 2, etc.
                assert!(
                    line.starts_with(&format!("{rank_num} ")),
                    "Rank line should start with '{rank_num}': '{line}'"
                );
            }
        }
    }

    #[test]
    fn test_empty_squares() {
        let mut board = Board::new();

        // Clear the entire board to test empty square display
        for rank in 0..8 {
            for file in 0..8 {
                board
                    .set_piece(Position::new_unchecked(file, rank), None)
                    .unwrap();
            }
        }

        let ascii = board.to_ascii();

        // Verify empty squares are displayed as dots
        assert!(
            ascii.contains("."),
            "Empty squares should be displayed as dots (.)"
        );

        // Count dots - should have 8 dots per rank, 8 ranks = 64 dots total
        let dot_count = ascii.matches('.').count();
        assert_eq!(
            dot_count, 64,
            "Should have exactly 64 dots for empty board, found {}",
            dot_count
        );

        // Verify the coordinate labels are still present
        assert!(
            ascii.contains("a b c d e f g h"),
            "File labels should still be present on empty board"
        );
        for rank in 1..=8 {
            assert!(
                ascii.contains(&rank.to_string()),
                "Rank labels should still be present on empty board"
            );
        }
    }

    #[test]
    fn test_mixed_board_display() {
        let mut board = Board::new();

        // Clear the board
        for rank in 0..8 {
            for file in 0..8 {
                board
                    .set_piece(Position::new_unchecked(file, rank), None)
                    .unwrap();
            }
        }

        // Place specific pieces to test display consistency
        board
            .set_piece(
                Position::new_unchecked(0, 0),
                Some(Piece::new(PieceType::Rook, Color::White)),
            )
            .unwrap(); // a1
        board
            .set_piece(
                Position::new_unchecked(4, 0),
                Some(Piece::new(PieceType::King, Color::White)),
            )
            .unwrap(); // e1
        board
            .set_piece(
                Position::new_unchecked(7, 7),
                Some(Piece::new(PieceType::Rook, Color::Black)),
            )
            .unwrap(); // h8
        board
            .set_piece(
                Position::new_unchecked(3, 3),
                Some(Piece::new(PieceType::Queen, Color::Black)),
            )
            .unwrap(); // d4

        let ascii = board.to_ascii();

        // Verify specific pieces appear in correct positions
        let lines: Vec<&str> = ascii.lines().collect();

        // Check rank 8 (line 1) contains black rook at h8
        assert!(lines[1].contains("♜"), "Rank 8 should contain black rook ♜");

        // Check rank 1 (line 8) contains white pieces
        assert!(lines[8].contains("♖"), "Rank 1 should contain white rook ♖");
        assert!(lines[8].contains("♔"), "Rank 1 should contain white king ♔");

        // Check rank 4 (line 5) contains black queen
        assert!(
            lines[5].contains("♛"),
            "Rank 4 should contain black queen ♛"
        );

        // Verify empty squares are still dots
        assert!(
            ascii.contains("."),
            "Empty squares should still be displayed as dots"
        );

        // Verify coordinate labels are maintained
        assert!(
            ascii.contains("a b c d e f g h"),
            "File labels should be maintained"
        );
    }

    #[test]
    fn test_display_consistency() {
        // Test that multiple calls to to_ascii() produce identical results
        let board = Board::new();

        let ascii1 = board.to_ascii();
        let ascii2 = board.to_ascii();

        assert_eq!(
            ascii1, ascii2,
            "Multiple calls to to_ascii() should produce identical results"
        );

        // Test that display is consistent after no-op operations
        let board_copy = board.clone();
        let ascii3 = board_copy.to_ascii();

        assert_eq!(
            ascii1, ascii3,
            "Display should be consistent after cloning board"
        );
    }

    #[test]
    fn test_piece_symbol_accuracy() {
        let mut board = Board::new();

        // Clear the board and place one piece of each type and color
        for rank in 0..8 {
            for file in 0..8 {
                board
                    .set_piece(Position::new_unchecked(file, rank), None)
                    .unwrap();
            }
        }

        // Test white pieces
        let white_pieces = [
            (PieceType::King, "♔"),
            (PieceType::Queen, "♕"),
            (PieceType::Rook, "♖"),
            (PieceType::Bishop, "♗"),
            (PieceType::Knight, "♘"),
            (PieceType::Pawn, "♙"),
        ];

        for (i, (piece_type, expected_symbol)) in white_pieces.iter().enumerate() {
            board
                .set_piece(
                    Position::new_unchecked(i as u8, 0),
                    Some(Piece::new(*piece_type, Color::White)),
                )
                .unwrap();

            let ascii = board.to_ascii();
            assert!(
                ascii.contains(expected_symbol),
                "Display should contain white {} symbol {}",
                piece_type,
                expected_symbol
            );
        }

        // Test black pieces
        let black_pieces = [
            (PieceType::King, "♚"),
            (PieceType::Queen, "♛"),
            (PieceType::Rook, "♜"),
            (PieceType::Bishop, "♝"),
            (PieceType::Knight, "♞"),
            (PieceType::Pawn, "♟"),
        ];

        for (i, (piece_type, expected_symbol)) in black_pieces.iter().enumerate() {
            board
                .set_piece(
                    Position::new_unchecked(i as u8, 7),
                    Some(Piece::new(*piece_type, Color::Black)),
                )
                .unwrap();

            let ascii = board.to_ascii();
            assert!(
                ascii.contains(expected_symbol),
                "Display should contain black {} symbol {}",
                piece_type,
                expected_symbol
            );
        }
    }

    #[test]
    fn test_display_spacing() {
        let board = Board::new();
        let ascii = board.to_ascii();
        let lines: Vec<&str> = ascii.lines().collect();

        // Test that each rank line has proper spacing between pieces
        for (i, line) in lines.iter().enumerate() {
            if i == 0 || i == lines.len() - 1 {
                // Skip file label lines
                continue;
            }

            // Each rank line should have the format: "N s s s s s s s s N"
            // where N is rank number, s is symbol or dot, and spaces separate them
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(
                parts.len(),
                10,
                "Each rank line should have 10 parts (rank + 8 squares + rank), got {} in line: '{}'",
                parts.len(),
                line
            );

            // First and last parts should be rank numbers
            let rank_num = 9 - i;
            assert_eq!(
                parts[0],
                rank_num.to_string(),
                "First part should be rank number"
            );
            assert_eq!(
                parts[9],
                rank_num.to_string(),
                "Last part should be rank number"
            );

            // Middle 8 parts should be either piece symbols or dots
            for (j, part) in parts[1..9].iter().enumerate() {
                assert_eq!(
                    part.chars().count(),
                    1,
                    "Each square should be represented by exactly one character at position {} in line: '{}'",
                    j,
                    line
                );
            }
        }
    }
}
