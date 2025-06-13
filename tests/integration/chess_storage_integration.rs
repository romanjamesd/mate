use mate::chess::{ChessError, Color, Move, Piece, PieceType, Position};
use mate::storage::models::PlayerColor;
use serde_json::json;

/// Storage Integration Tests
/// Tests the integration between chess data structures and storage system
#[cfg(test)]
mod storage_integration_tests {
    use super::*;

    /// Test seamless Color ↔ PlayerColor conversion
    #[test]
    fn test_color_player_color_conversion() {
        // Test Color to PlayerColor conversion
        let white_chess = Color::White;
        let white_storage: PlayerColor = white_chess.into();
        assert_eq!(white_storage, PlayerColor::White);

        let black_chess = Color::Black;
        let black_storage: PlayerColor = black_chess.into();
        assert_eq!(black_storage, PlayerColor::Black);

        // Test PlayerColor to Color conversion
        let white_storage = PlayerColor::White;
        let white_chess: Color = white_storage.into();
        assert_eq!(white_chess, Color::White);

        let black_storage = PlayerColor::Black;
        let black_chess: Color = black_storage.into();
        assert_eq!(black_chess, Color::Black);

        // Test bidirectional conversion consistency
        assert_eq!(Color::White, PlayerColor::White.into());
        assert_eq!(Color::Black, PlayerColor::Black.into());
        assert_eq!(PlayerColor::White, Color::White.into());
        assert_eq!(PlayerColor::Black, Color::Black.into());
    }

    /// Test Color ↔ PlayerColor roundtrip conversions
    #[test]
    fn test_color_conversion_roundtrips() {
        let colors = [Color::White, Color::Black];

        for original_color in colors {
            // Color -> PlayerColor -> Color
            let player_color: PlayerColor = original_color.into();
            let converted_back: Color = player_color.into();
            assert_eq!(
                original_color, converted_back,
                "Color roundtrip failed for {:?}",
                original_color
            );
        }

        let player_colors = [PlayerColor::White, PlayerColor::Black];

        for player_color in &player_colors {
            // PlayerColor -> Color -> PlayerColor
            let color: Color = player_color.clone().into();
            let converted_back: PlayerColor = color.into();
            assert_eq!(
                *player_color, converted_back,
                "PlayerColor roundtrip failed for {:?}",
                player_color
            );
        }
    }

    /// Test Move storage - JSON format compatibility with game storage
    #[test]
    fn test_move_json_storage_format() {
        // Test simple move JSON serialization
        let simple_move = Move::simple(
            Position::new(4, 1).unwrap(), // e2
            Position::new(4, 3).unwrap(), // e4
        )
        .unwrap();

        let json = simple_move.to_json();
        let expected = json!({
            "from": "e2",
            "to": "e4"
        });

        assert_eq!(
            json, expected,
            "Simple move JSON format should match expected"
        );

        // Test promotion move JSON serialization
        let promotion_move = Move::promotion(
            Position::new(4, 6).unwrap(), // e7
            Position::new(4, 7).unwrap(), // e8
            PieceType::Queen,
        )
        .unwrap();

        let json = promotion_move.to_json();
        let expected = json!({
            "from": "e7",
            "to": "e8",
            "promotion": "Q"
        });

        assert_eq!(
            json, expected,
            "Promotion move JSON format should match expected"
        );
    }

    /// Test Move JSON roundtrip compatibility
    #[test]
    fn test_move_json_roundtrip() {
        let test_moves = vec![
            // Simple move
            Move::simple(
                Position::new(0, 1).unwrap(), // a2
                Position::new(0, 3).unwrap(), // a4
            )
            .unwrap(),
            // Knight move
            Move::simple(
                Position::new(1, 0).unwrap(), // b1
                Position::new(2, 2).unwrap(), // c3
            )
            .unwrap(),
            // Promotion move
            Move::promotion(
                Position::new(7, 6).unwrap(), // h7
                Position::new(7, 7).unwrap(), // h8
                PieceType::Rook,
            )
            .unwrap(),
        ];

        for original_move in test_moves {
            // Move -> JSON -> Move
            let json = original_move.to_json();
            let parsed_move = Move::from_json(&json).expect("Failed to parse move from JSON");

            assert_eq!(
                original_move, parsed_move,
                "Move JSON roundtrip failed for {:?}",
                original_move
            );
        }
    }

    /// Test Move JSON parsing with various formats
    #[test]
    fn test_move_json_parsing_compatibility() {
        // Test standard JSON format
        let json = json!({
            "from": "d2",
            "to": "d4"
        });
        let parsed_move = Move::from_json(&json).unwrap();
        let expected_move = Move::simple(
            Position::new(3, 1).unwrap(), // d2
            Position::new(3, 3).unwrap(), // d4
        )
        .unwrap();
        assert_eq!(parsed_move, expected_move);

        // Test JSON with promotion
        let json = json!({
            "from": "c7",
            "to": "c8",
            "promotion": "N"
        });
        let parsed_move = Move::from_json(&json).unwrap();
        let expected_move = Move::promotion(
            Position::new(2, 6).unwrap(), // c7
            Position::new(2, 7).unwrap(), // c8
            PieceType::Knight,
        )
        .unwrap();
        assert_eq!(parsed_move, expected_move);
    }

    /// Test Move JSON error handling
    #[test]
    fn test_move_json_error_handling() {
        // Test missing 'from' field
        let json = json!({
            "to": "e4"
        });
        assert!(
            Move::from_json(&json).is_err(),
            "Should fail with missing 'from' field"
        );

        // Test missing 'to' field
        let json = json!({
            "from": "e2"
        });
        assert!(
            Move::from_json(&json).is_err(),
            "Should fail with missing 'to' field"
        );

        // Test invalid position
        let json = json!({
            "from": "z9",
            "to": "e4"
        });
        assert!(
            Move::from_json(&json).is_err(),
            "Should fail with invalid position"
        );

        // Test invalid promotion piece
        let json = json!({
            "from": "e7",
            "to": "e8",
            "promotion": "K"
        });
        assert!(
            Move::from_json(&json).is_err(),
            "Should fail with invalid promotion piece"
        );
    }

    /// Test type compatibility with existing storage system
    #[test]
    fn test_type_compatibility_with_storage() {
        // Test that all chess types can be serialized with serde (used by storage)

        // Test Color serialization
        let color = Color::White;
        let serialized = serde_json::to_string(&color).unwrap();
        let deserialized: Color = serde_json::from_str(&serialized).unwrap();
        assert_eq!(color, deserialized);

        // Test PieceType serialization
        let piece_type = PieceType::Queen;
        let serialized = serde_json::to_string(&piece_type).unwrap();
        let deserialized: PieceType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(piece_type, deserialized);

        // Test Piece serialization
        let piece = Piece::new(PieceType::Knight, Color::Black);
        let serialized = serde_json::to_string(&piece).unwrap();
        let deserialized: Piece = serde_json::from_str(&serialized).unwrap();
        assert_eq!(piece, deserialized);

        // Test Position serialization
        let position = Position::new(3, 4).unwrap(); // d5
        let serialized = serde_json::to_string(&position).unwrap();
        let deserialized: Position = serde_json::from_str(&serialized).unwrap();
        assert_eq!(position, deserialized);

        // Test Move serialization
        let move_obj = Move::simple(
            Position::new(4, 1).unwrap(), // e2
            Position::new(4, 3).unwrap(), // e4
        )
        .unwrap();
        let serialized = serde_json::to_string(&move_obj).unwrap();
        let deserialized: Move = serde_json::from_str(&serialized).unwrap();
        assert_eq!(move_obj, deserialized);
    }

    /// Test PlayerColor compatibility with chess Color
    #[test]
    fn test_player_color_chess_compatibility() {
        // Test that we can use PlayerColor and Color interchangeably
        let player_white = PlayerColor::White;
        let chess_white = Color::White;

        // Convert and compare
        let converted_chess: Color = player_white.into();
        assert_eq!(converted_chess, chess_white);

        let converted_player: PlayerColor = chess_white.into();
        assert_eq!(converted_player, PlayerColor::White);

        // Test with black
        let player_black = PlayerColor::Black;
        let chess_black = Color::Black;

        let converted_chess: Color = player_black.into();
        assert_eq!(converted_chess, chess_black);

        let converted_player: PlayerColor = chess_black.into();
        assert_eq!(converted_player, PlayerColor::Black);
    }

    /// Test complete storage integration scenario
    #[test]
    fn test_complete_storage_integration_scenario() {
        // Simulate a complete game storage scenario

        // 1. Store player color preference
        let player_color = Color::White;
        let storage_color: PlayerColor = player_color.into();

        // 2. Create and store moves in JSON format
        let moves = vec![
            Move::simple(Position::new(4, 1).unwrap(), Position::new(4, 3).unwrap()).unwrap(), // e2e4
            Move::simple(Position::new(4, 6).unwrap(), Position::new(4, 4).unwrap()).unwrap(), // e7e5
            Move::simple(Position::new(6, 0).unwrap(), Position::new(5, 2).unwrap()).unwrap(), // g1f3
        ];

        let mut stored_moves = Vec::new();
        for move_obj in &moves {
            let json = move_obj.to_json();
            stored_moves.push(json);
        }

        // 3. Retrieve and reconstruct moves from storage
        let mut reconstructed_moves = Vec::new();
        for json in &stored_moves {
            let move_obj = Move::from_json(json).unwrap();
            reconstructed_moves.push(move_obj);
        }

        // 4. Verify integrity
        assert_eq!(
            moves, reconstructed_moves,
            "Moves should survive storage roundtrip"
        );

        // 5. Verify color conversion worked
        let retrieved_chess_color: Color = storage_color.into();
        assert_eq!(
            player_color, retrieved_chess_color,
            "Color should survive storage conversion"
        );
    }

    /// Test error consistency across storage integration
    #[test]
    fn test_storage_integration_error_consistency() {
        // Test that all storage-related errors are properly typed as ChessError

        // Invalid JSON structure should return ChessError
        let invalid_json = json!({
            "invalid": "structure"
        });
        match Move::from_json(&invalid_json) {
            Err(ChessError::InvalidMove(_)) => {} // Expected
            Err(other) => panic!("Expected InvalidMove error, got {:?}", other),
            Ok(_) => panic!("Expected error for invalid JSON"),
        }

        // Invalid position in JSON should return ChessError
        let invalid_pos_json = json!({
            "from": "invalid",
            "to": "e4"
        });
        match Move::from_json(&invalid_pos_json) {
            Err(ChessError::InvalidPosition(_)) => {} // Expected from Position parsing
            Err(other) => panic!("Expected InvalidPosition error, got {:?}", other),
            Ok(_) => panic!("Expected error for invalid position"),
        }
    }
}
