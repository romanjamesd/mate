use mate::chess::{Color, Piece, PieceType};

#[cfg(test)]
mod piece_tests {
    use super::*;

    #[test]
    fn test_piece_creation() {
        // Test creation with all piece types and colors
        let white_pawn = Piece::new(PieceType::Pawn, Color::White);
        assert_eq!(white_pawn.piece_type, PieceType::Pawn);
        assert_eq!(white_pawn.color, Color::White);

        let black_king = Piece::new(PieceType::King, Color::Black);
        assert_eq!(black_king.piece_type, PieceType::King);
        assert_eq!(black_king.color, Color::Black);

        // Test creation with all combinations
        for piece_type in [
            PieceType::Pawn,
            PieceType::Rook,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Queen,
            PieceType::King,
        ] {
            for color in [Color::White, Color::Black] {
                let piece = Piece::new(piece_type, color);
                assert_eq!(piece.piece_type, piece_type);
                assert_eq!(piece.color, color);
            }
        }
    }

    #[test]
    fn test_piece_unicode_display_white() {
        // Test white piece unicode symbols
        assert_eq!(Piece::new(PieceType::Pawn, Color::White).to_string(), "♙");
        assert_eq!(Piece::new(PieceType::Rook, Color::White).to_string(), "♖");
        assert_eq!(Piece::new(PieceType::Knight, Color::White).to_string(), "♘");
        assert_eq!(Piece::new(PieceType::Bishop, Color::White).to_string(), "♗");
        assert_eq!(Piece::new(PieceType::Queen, Color::White).to_string(), "♕");
        assert_eq!(Piece::new(PieceType::King, Color::White).to_string(), "♔");
    }

    #[test]
    fn test_piece_unicode_display_black() {
        // Test black piece unicode symbols
        assert_eq!(Piece::new(PieceType::Pawn, Color::Black).to_string(), "♟");
        assert_eq!(Piece::new(PieceType::Rook, Color::Black).to_string(), "♜");
        assert_eq!(Piece::new(PieceType::Knight, Color::Black).to_string(), "♞");
        assert_eq!(Piece::new(PieceType::Bishop, Color::Black).to_string(), "♝");
        assert_eq!(Piece::new(PieceType::Queen, Color::Black).to_string(), "♛");
        assert_eq!(Piece::new(PieceType::King, Color::Black).to_string(), "♚");
    }

    #[test]
    fn test_piece_unicode_display_comprehensive() {
        // Test all piece/color combinations in a structured way
        let white_symbols = ["♙", "♖", "♘", "♗", "♕", "♔"];
        let black_symbols = ["♟", "♜", "♞", "♝", "♛", "♚"];
        let piece_types = [
            PieceType::Pawn,
            PieceType::Rook,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Queen,
            PieceType::King,
        ];

        for (i, piece_type) in piece_types.iter().enumerate() {
            let white_piece = Piece::new(*piece_type, Color::White);
            let black_piece = Piece::new(*piece_type, Color::Black);

            assert_eq!(white_piece.to_string(), white_symbols[i]);
            assert_eq!(black_piece.to_string(), black_symbols[i]);
        }
    }

    #[test]
    fn test_piece_value_delegation() {
        // Test that piece value correctly delegates to piece_type.value()
        for piece_type in [
            PieceType::Pawn,
            PieceType::Rook,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Queen,
            PieceType::King,
        ] {
            for color in [Color::White, Color::Black] {
                let piece = Piece::new(piece_type, color);
                assert_eq!(piece.value(), piece_type.value());
            }
        }
    }

    #[test]
    fn test_piece_value_specific() {
        // Test specific piece values
        assert_eq!(Piece::new(PieceType::Pawn, Color::White).value(), 1);
        assert_eq!(Piece::new(PieceType::Knight, Color::Black).value(), 3);
        assert_eq!(Piece::new(PieceType::Bishop, Color::White).value(), 3);
        assert_eq!(Piece::new(PieceType::Rook, Color::Black).value(), 5);
        assert_eq!(Piece::new(PieceType::Queen, Color::White).value(), 9);
        assert_eq!(Piece::new(PieceType::King, Color::Black).value(), 0);
    }

    #[test]
    fn test_piece_equality_and_hash() {
        // Test that pieces with same type and color are equal
        let piece1 = Piece::new(PieceType::Queen, Color::White);
        let piece2 = Piece::new(PieceType::Queen, Color::White);
        let piece3 = Piece::new(PieceType::Queen, Color::Black);
        let piece4 = Piece::new(PieceType::King, Color::White);

        assert_eq!(piece1, piece2);
        assert_ne!(piece1, piece3);
        assert_ne!(piece1, piece4);

        // Test hash consistency (same pieces should have same hash)
        use std::collections::HashMap;
        let mut map = HashMap::new();
        map.insert(piece1, "value1");
        assert_eq!(map.get(&piece2), Some(&"value1"));
    }

    #[test]
    fn test_piece_serialization() {
        // Test that pieces can be serialized and deserialized
        let original = Piece::new(PieceType::Queen, Color::Black);
        let json = serde_json::to_string(&original).expect("Failed to serialize piece");
        let deserialized: Piece = serde_json::from_str(&json).expect("Failed to deserialize piece");

        assert_eq!(original, deserialized);
        assert_eq!(original.value(), deserialized.value());
        assert_eq!(original.to_string(), deserialized.to_string());
    }

    #[test]
    fn test_piece_debug_format() {
        // Test debug format for development purposes
        let piece = Piece::new(PieceType::Knight, Color::White);
        let debug_str = format!("{piece:?}");

        // Should contain both piece type and color information
        assert!(debug_str.contains("Knight"));
        assert!(debug_str.contains("White"));
    }
}
