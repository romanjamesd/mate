use mate::chess::{ChessError, Color, Move, Piece, PieceType, Position};
use serde_json;

#[cfg(test)]
mod serde_compatibility_tests {
    use super::*;

    #[test]
    fn test_color_serialize_deserialize_roundtrip() {
        let colors = vec![Color::White, Color::Black];

        for color in colors {
            // Test JSON serialization roundtrip
            let serialized = serde_json::to_string(&color).expect("Failed to serialize Color");
            let deserialized: Color =
                serde_json::from_str(&serialized).expect("Failed to deserialize Color");
            assert_eq!(color, deserialized);

            // Test binary serialization roundtrip with bincode
            let binary_serialized =
                bincode::serialize(&color).expect("Failed to serialize Color to binary");
            let binary_deserialized: Color = bincode::deserialize(&binary_serialized)
                .expect("Failed to deserialize Color from binary");
            assert_eq!(color, binary_deserialized);
        }
    }

    #[test]
    fn test_piece_type_serialize_deserialize_roundtrip() {
        let piece_types = vec![
            PieceType::Pawn,
            PieceType::Rook,
            PieceType::Knight,
            PieceType::Bishop,
            PieceType::Queen,
            PieceType::King,
        ];

        for piece_type in piece_types {
            // Test JSON serialization roundtrip
            let serialized =
                serde_json::to_string(&piece_type).expect("Failed to serialize PieceType");
            let deserialized: PieceType =
                serde_json::from_str(&serialized).expect("Failed to deserialize PieceType");
            assert_eq!(piece_type, deserialized);

            // Test binary serialization roundtrip
            let binary_serialized =
                bincode::serialize(&piece_type).expect("Failed to serialize PieceType to binary");
            let binary_deserialized: PieceType = bincode::deserialize(&binary_serialized)
                .expect("Failed to deserialize PieceType from binary");
            assert_eq!(piece_type, binary_deserialized);
        }
    }

    #[test]
    fn test_piece_serialize_deserialize_roundtrip() {
        let pieces = vec![
            Piece::new(PieceType::Pawn, Color::White),
            Piece::new(PieceType::Rook, Color::Black),
            Piece::new(PieceType::Knight, Color::White),
            Piece::new(PieceType::Bishop, Color::Black),
            Piece::new(PieceType::Queen, Color::White),
            Piece::new(PieceType::King, Color::Black),
        ];

        for piece in pieces {
            // Test JSON serialization roundtrip
            let serialized = serde_json::to_string(&piece).expect("Failed to serialize Piece");
            let deserialized: Piece =
                serde_json::from_str(&serialized).expect("Failed to deserialize Piece");
            assert_eq!(piece, deserialized);

            // Test binary serialization roundtrip
            let binary_serialized =
                bincode::serialize(&piece).expect("Failed to serialize Piece to binary");
            let binary_deserialized: Piece = bincode::deserialize(&binary_serialized)
                .expect("Failed to deserialize Piece from binary");
            assert_eq!(piece, binary_deserialized);
        }
    }

    #[test]
    fn test_position_serialize_deserialize_roundtrip() {
        let positions = vec![
            Position::new(0, 0).unwrap(), // a1
            Position::new(7, 7).unwrap(), // h8
            Position::new(3, 4).unwrap(), // d5
            Position::new(4, 3).unwrap(), // e4
        ];

        for position in positions {
            // Test JSON serialization roundtrip
            let serialized =
                serde_json::to_string(&position).expect("Failed to serialize Position");
            let deserialized: Position =
                serde_json::from_str(&serialized).expect("Failed to deserialize Position");
            assert_eq!(position, deserialized);

            // Test binary serialization roundtrip
            let binary_serialized =
                bincode::serialize(&position).expect("Failed to serialize Position to binary");
            let binary_deserialized: Position = bincode::deserialize(&binary_serialized)
                .expect("Failed to deserialize Position from binary");
            assert_eq!(position, binary_deserialized);
        }
    }

    #[test]
    fn test_move_serialize_deserialize_roundtrip() {
        let moves = vec![
            Move::simple(Position::new(4, 1).unwrap(), Position::new(4, 3).unwrap()).unwrap(), // e2e4
            Move::promotion(
                Position::new(0, 6).unwrap(),
                Position::new(0, 7).unwrap(),
                PieceType::Queen,
            )
            .unwrap(), // a7a8q
            Move::simple(Position::new(4, 0).unwrap(), Position::new(6, 0).unwrap()).unwrap(), // kingside castling pattern
        ];

        for move_obj in moves {
            // Test JSON serialization roundtrip
            let serialized = serde_json::to_string(&move_obj).expect("Failed to serialize Move");
            let deserialized: Move =
                serde_json::from_str(&serialized).expect("Failed to deserialize Move");
            assert_eq!(move_obj, deserialized);

            // Test binary serialization roundtrip
            let binary_serialized =
                bincode::serialize(&move_obj).expect("Failed to serialize Move to binary");
            let binary_deserialized: Move = bincode::deserialize(&binary_serialized)
                .expect("Failed to deserialize Move from binary");
            assert_eq!(move_obj, binary_deserialized);
        }
    }

    #[test]
    fn test_json_format_compatibility() {
        // Test JSON format matches expected storage format
        let color = Color::White;
        let piece_type = PieceType::Queen;
        let piece = Piece::new(PieceType::King, Color::Black);
        let position = Position::new(4, 4).unwrap(); // e5
        let move_obj =
            Move::simple(Position::new(4, 1).unwrap(), Position::new(4, 3).unwrap()).unwrap(); // e2e4

        // Test that JSON output is human-readable and consistent
        let color_json = serde_json::to_string(&color).unwrap();
        assert!(color_json.contains("White"));

        let piece_type_json = serde_json::to_string(&piece_type).unwrap();
        assert!(piece_type_json.contains("Queen"));

        let piece_json = serde_json::to_string(&piece).unwrap();
        assert!(piece_json.contains("King") && piece_json.contains("Black"));

        let position_json = serde_json::to_string(&position).unwrap();
        // Position should serialize with file and rank fields
        assert!(position_json.contains("file") && position_json.contains("rank"));

        let move_json = serde_json::to_string(&move_obj).unwrap();
        // Move should serialize with from and to fields
        assert!(move_json.contains("from") && move_json.contains("to"));
    }

    #[test]
    fn test_move_json_format_compatibility() {
        // Test Move's custom JSON format for storage compatibility
        let simple_move =
            Move::simple(Position::new(4, 1).unwrap(), Position::new(4, 3).unwrap()).unwrap(); // e2e4
        let promotion_move = Move::promotion(
            Position::new(0, 6).unwrap(),
            Position::new(0, 7).unwrap(),
            PieceType::Queen,
        )
        .unwrap(); // a7a8q

        // Test to_json() format
        let simple_json = simple_move.to_json();
        assert_eq!(simple_json["from"], "e2");
        assert_eq!(simple_json["to"], "e4");
        assert!(simple_json.get("promotion").is_none());

        let promotion_json = promotion_move.to_json();
        assert_eq!(promotion_json["from"], "a7");
        assert_eq!(promotion_json["to"], "a8");
        assert_eq!(promotion_json["promotion"], "Q");

        // Test from_json() roundtrip
        let simple_parsed = Move::from_json(&simple_json).unwrap();
        assert_eq!(simple_move, simple_parsed);

        let promotion_parsed = Move::from_json(&promotion_json).unwrap();
        assert_eq!(promotion_move, promotion_parsed);
    }

    #[test]
    fn test_error_handling_invalid_json() {
        // Test that invalid JSON returns appropriate ChessError

        // Invalid JSON strings should fail gracefully
        let invalid_json_strings = vec![
            "{",                          // incomplete JSON
            "null",                       // null value
            "42",                         // wrong type
            "\"invalid\"",                // invalid string
            "{\"wrong\": \"structure\"}", // wrong structure
        ];

        for invalid_json in invalid_json_strings {
            // Test Color deserialization errors
            let color_result: Result<Color, _> = serde_json::from_str(invalid_json);
            assert!(color_result.is_err());

            // Test PieceType deserialization errors
            let piece_type_result: Result<PieceType, _> = serde_json::from_str(invalid_json);
            assert!(piece_type_result.is_err());

            // Test Piece deserialization errors
            let piece_result: Result<Piece, _> = serde_json::from_str(invalid_json);
            assert!(piece_result.is_err());

            // Test Position deserialization errors
            let position_result: Result<Position, _> = serde_json::from_str(invalid_json);
            assert!(position_result.is_err());

            // Test Move deserialization errors
            let move_result: Result<Move, _> = serde_json::from_str(invalid_json);
            assert!(move_result.is_err());
        }
    }

    #[test]
    fn test_move_from_json_error_handling() {
        // Test Move::from_json with invalid JSON structures
        let invalid_move_jsons = vec![
            serde_json::json!({}),                              // missing fields
            serde_json::json!({"from": "e2"}),                  // missing to
            serde_json::json!({"to": "e4"}),                    // missing from
            serde_json::json!({"from": "invalid", "to": "e4"}), // invalid from
            serde_json::json!({"from": "e2", "to": "invalid"}), // invalid to
            serde_json::json!({"from": "e2", "to": "e4", "promotion": "invalid"}), // invalid promotion
            serde_json::json!({"from": 42, "to": "e4"}), // wrong type for from
            serde_json::json!({"from": "e2", "to": 42}), // wrong type for to
        ];

        for invalid_json in invalid_move_jsons {
            let result = Move::from_json(&invalid_json);
            assert!(result.is_err());

            // Verify it returns ChessError
            match result.unwrap_err() {
                ChessError::InvalidMove(_)
                | ChessError::InvalidPosition(_)
                | ChessError::InvalidPieceType(_) => {
                    // Expected error types
                }
                _ => panic!("Unexpected error type"),
            }
        }
    }

    #[test]
    fn test_all_types_serde_compatibility() {
        // Comprehensive test ensuring all chess types work together in serialization
        let chess_data = ChessDataCollection {
            color: Color::White,
            piece_type: PieceType::Queen,
            piece: Piece::new(PieceType::King, Color::Black),
            position: Position::new(4, 4).unwrap(),
            move_obj: Move::simple(Position::new(4, 1).unwrap(), Position::new(4, 3).unwrap())
                .unwrap(),
            optional_move: Some(
                Move::promotion(
                    Position::new(0, 6).unwrap(),
                    Position::new(0, 7).unwrap(),
                    PieceType::Queen,
                )
                .unwrap(),
            ),
            colors: vec![Color::White, Color::Black],
            positions: vec![Position::new(0, 0).unwrap(), Position::new(7, 7).unwrap()],
        };

        // Test JSON roundtrip
        let json_serialized =
            serde_json::to_string(&chess_data).expect("Failed to serialize ChessDataCollection");
        let json_deserialized: ChessDataCollection = serde_json::from_str(&json_serialized)
            .expect("Failed to deserialize ChessDataCollection");
        assert_eq!(chess_data, json_deserialized);

        // Test binary roundtrip
        let binary_serialized = bincode::serialize(&chess_data)
            .expect("Failed to serialize ChessDataCollection to binary");
        let binary_deserialized: ChessDataCollection = bincode::deserialize(&binary_serialized)
            .expect("Failed to deserialize ChessDataCollection from binary");
        assert_eq!(chess_data, binary_deserialized);
    }

    // Helper struct for comprehensive testing
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct ChessDataCollection {
        color: Color,
        piece_type: PieceType,
        piece: Piece,
        position: Position,
        move_obj: Move,
        optional_move: Option<Move>,
        colors: Vec<Color>,
        positions: Vec<Position>,
    }
}
