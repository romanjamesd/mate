use mate::chess::{ChessError, Move, PieceType, Position};

/// Test valid Move creation with different from/to positions
#[test]
fn test_move_valid_creation() {
    // Test simple move creation
    let from = Position::new(4, 1).unwrap(); // e2
    let to = Position::new(4, 3).unwrap(); // e4

    let chess_move = Move::new(from, to, None).expect("Valid move should be created");
    assert_eq!(chess_move.from, from);
    assert_eq!(chess_move.to, to);
    assert_eq!(chess_move.promotion, None);

    // Test move with promotion
    let from_pawn = Position::new(4, 6).unwrap(); // e7
    let to_pawn = Position::new(4, 7).unwrap(); // e8
    let promotion_move = Move::new(from_pawn, to_pawn, Some(PieceType::Queen))
        .expect("Valid promotion move should be created");

    assert_eq!(promotion_move.from, from_pawn);
    assert_eq!(promotion_move.to, to_pawn);
    assert_eq!(promotion_move.promotion, Some(PieceType::Queen));

    // Test various valid moves
    let test_moves = [
        (
            Position::new(0, 0).unwrap(),
            Position::new(7, 7).unwrap(),
            None,
        ), // a1h8
        (
            Position::new(3, 0).unwrap(),
            Position::new(3, 7).unwrap(),
            None,
        ), // d1d8
        (
            Position::new(1, 6).unwrap(),
            Position::new(1, 7).unwrap(),
            Some(PieceType::Rook),
        ), // b7b8R
    ];

    for (from, to, promotion) in &test_moves {
        let chess_move = Move::new(*from, *to, *promotion).expect("Valid move should be created");
        assert_eq!(chess_move.from, *from);
        assert_eq!(chess_move.to, *to);
        assert_eq!(chess_move.promotion, *promotion);
    }
}

/// Test invalid Move creation when from/to positions are the same
#[test]
fn test_move_invalid_creation_same_positions() {
    let pos = Position::new(4, 4).unwrap(); // e5

    let result = Move::new(pos, pos, None);
    assert!(result.is_err(), "Move with same from/to should fail");

    match result.unwrap_err() {
        ChessError::InvalidMove(msg) => {
            assert!(msg.contains("same"), "Error should mention same positions");
        }
        other => panic!("Expected InvalidMove error, got: {:?}", other),
    }

    // Test with promotion - should still fail
    let result_promotion = Move::new(pos, pos, Some(PieceType::Queen));
    assert!(
        result_promotion.is_err(),
        "Move with same from/to should fail even with promotion"
    );
}

/// Test invalid promotion to King or Pawn
#[test]
fn test_move_invalid_promotion() {
    let from = Position::new(4, 6).unwrap(); // e7
    let to = Position::new(4, 7).unwrap(); // e8

    // Test King promotion
    let king_promotion = Move::new(from, to, Some(PieceType::King));
    assert!(king_promotion.is_err(), "King promotion should be invalid");

    match king_promotion.unwrap_err() {
        ChessError::InvalidMove(msg) => {
            assert!(msg.contains("King"), "Error should mention King");
        }
        other => panic!("Expected InvalidMove error, got: {:?}", other),
    }

    // Test Pawn promotion
    let pawn_promotion = Move::new(from, to, Some(PieceType::Pawn));
    assert!(pawn_promotion.is_err(), "Pawn promotion should be invalid");

    match pawn_promotion.unwrap_err() {
        ChessError::InvalidMove(msg) => {
            assert!(msg.contains("Pawn"), "Error should mention Pawn");
        }
        other => panic!("Expected InvalidMove error, got: {:?}", other),
    }

    // Test valid promotions work
    let valid_promotions = [
        PieceType::Queen,
        PieceType::Rook,
        PieceType::Bishop,
        PieceType::Knight,
    ];
    for piece_type in &valid_promotions {
        let result = Move::new(from, to, Some(*piece_type));
        assert!(
            result.is_ok(),
            "Promotion to {:?} should be valid",
            piece_type
        );
    }
}

/// Test Move helper creation methods
#[test]
fn test_move_creation_helpers() {
    let from = Position::new(4, 1).unwrap(); // e2
    let to = Position::new(4, 3).unwrap(); // e4

    // Test simple() method
    let simple_move = Move::simple(from, to).expect("Simple move should work");
    assert_eq!(simple_move.from, from);
    assert_eq!(simple_move.to, to);
    assert_eq!(simple_move.promotion, None);

    // Test promotion() method
    let from_pawn = Position::new(4, 6).unwrap(); // e7
    let to_pawn = Position::new(4, 7).unwrap(); // e8
    let promotion_move =
        Move::promotion(from_pawn, to_pawn, PieceType::Queen).expect("Promotion move should work");
    assert_eq!(promotion_move.from, from_pawn);
    assert_eq!(promotion_move.to, to_pawn);
    assert_eq!(promotion_move.promotion, Some(PieceType::Queen));

    // Test new_unchecked() method
    let unchecked_move = Move::new_unchecked(from, to, None);
    assert_eq!(unchecked_move.from, from);
    assert_eq!(unchecked_move.to, to);
    assert_eq!(unchecked_move.promotion, None);
}

/// Test Move parsing from algebraic notation
#[test]
fn test_move_parsing_basic() {
    // Test basic move format (e2e4)
    let move_e2e4 = "e2e4".parse::<Move>().expect("Should parse e2e4");
    assert_eq!(move_e2e4.from, Position::new(4, 1).unwrap()); // e2
    assert_eq!(move_e2e4.to, Position::new(4, 3).unwrap()); // e4
    assert_eq!(move_e2e4.promotion, None);

    // Test other basic moves
    let test_moves = [
        (
            "a1h8",
            Position::new(0, 0).unwrap(),
            Position::new(7, 7).unwrap(),
            None,
        ),
        (
            "d1d8",
            Position::new(3, 0).unwrap(),
            Position::new(3, 7).unwrap(),
            None,
        ),
        (
            "h7h5",
            Position::new(7, 6).unwrap(),
            Position::new(7, 4).unwrap(),
            None,
        ),
        (
            "c2c4",
            Position::new(2, 1).unwrap(),
            Position::new(2, 3).unwrap(),
            None,
        ),
    ];

    for (move_str, expected_from, expected_to, expected_promotion) in &test_moves {
        let parsed_move = move_str
            .parse::<Move>()
            .unwrap_or_else(|_| panic!("Should parse {}", move_str));
        assert_eq!(
            parsed_move.from, *expected_from,
            "From position mismatch for {}",
            move_str
        );
        assert_eq!(
            parsed_move.to, *expected_to,
            "To position mismatch for {}",
            move_str
        );
        assert_eq!(
            parsed_move.promotion, *expected_promotion,
            "Promotion mismatch for {}",
            move_str
        );
    }
}

/// Test Move parsing with promotion (e7e8q)
#[test]
fn test_move_parsing_promotion() {
    // Test promotion moves
    let promotion_moves = [
        (
            "e7e8q",
            Position::new(4, 6).unwrap(),
            Position::new(4, 7).unwrap(),
            PieceType::Queen,
        ),
        (
            "e7e8r",
            Position::new(4, 6).unwrap(),
            Position::new(4, 7).unwrap(),
            PieceType::Rook,
        ),
        (
            "e7e8b",
            Position::new(4, 6).unwrap(),
            Position::new(4, 7).unwrap(),
            PieceType::Bishop,
        ),
        (
            "e7e8n",
            Position::new(4, 6).unwrap(),
            Position::new(4, 7).unwrap(),
            PieceType::Knight,
        ),
        (
            "a7a8Q",
            Position::new(0, 6).unwrap(),
            Position::new(0, 7).unwrap(),
            PieceType::Queen,
        ), // Test uppercase
        (
            "h7h8R",
            Position::new(7, 6).unwrap(),
            Position::new(7, 7).unwrap(),
            PieceType::Rook,
        ),
    ];

    for (move_str, expected_from, expected_to, expected_piece) in &promotion_moves {
        let parsed_move = move_str
            .parse::<Move>()
            .unwrap_or_else(|_| panic!("Should parse promotion {}", move_str));
        assert_eq!(
            parsed_move.from, *expected_from,
            "From position mismatch for {}",
            move_str
        );
        assert_eq!(
            parsed_move.to, *expected_to,
            "To position mismatch for {}",
            move_str
        );
        assert_eq!(
            parsed_move.promotion,
            Some(*expected_piece),
            "Promotion piece mismatch for {}",
            move_str
        );
    }
}

/// Test Move parsing for castling (O-O, O-O-O)
#[test]
fn test_move_parsing_castling() {
    // Test kingside castling
    let kingside_variants = ["O-O", "0-0"];
    for variant in &kingside_variants {
        let parsed_move = variant
            .parse::<Move>()
            .unwrap_or_else(|_| panic!("Should parse kingside castling {}", variant));
        assert_eq!(
            parsed_move.from,
            Position::new(4, 0).unwrap(),
            "Kingside from should be e1"
        );
        assert_eq!(
            parsed_move.to,
            Position::new(6, 0).unwrap(),
            "Kingside to should be g1"
        );
        assert_eq!(
            parsed_move.promotion, None,
            "Castling should have no promotion"
        );
    }

    // Test queenside castling
    let queenside_variants = ["O-O-O", "0-0-0"];
    for variant in &queenside_variants {
        let parsed_move = variant
            .parse::<Move>()
            .unwrap_or_else(|_| panic!("Should parse queenside castling {}", variant));
        assert_eq!(
            parsed_move.from,
            Position::new(4, 0).unwrap(),
            "Queenside from should be e1"
        );
        assert_eq!(
            parsed_move.to,
            Position::new(2, 0).unwrap(),
            "Queenside to should be c1"
        );
        assert_eq!(
            parsed_move.promotion, None,
            "Castling should have no promotion"
        );
    }
}

/// Test invalid Move parsing
#[test]
fn test_move_parsing_invalid() {
    let invalid_moves = [
        "",        // Empty string
        "e2",      // Too short
        "e2e4e4",  // Too long
        "e2e9",    // Invalid rank
        "i2e4",    // Invalid file
        "e2i4",    // Invalid destination file
        "e2e4k",   // Invalid promotion (king)
        "e2e4p",   // Invalid promotion (pawn)
        "OO",      // Wrong castling format
        "O-O-O-O", // Invalid castling
        "e2-e4",   // Wrong separator
        "e2 e4",   // Space instead of no separator
    ];

    for invalid_move in &invalid_moves {
        let result = invalid_move.parse::<Move>();
        assert!(
            result.is_err(),
            "Move '{}' should fail to parse",
            invalid_move
        );

        match result.unwrap_err() {
            ChessError::InvalidMove(_)
            | ChessError::InvalidPosition(_)
            | ChessError::InvalidPieceType(_) => {
                // These are expected error types
            }
            other => panic!("Unexpected error type for '{}': {:?}", invalid_move, other),
        }
    }
}

/// Test Move detection methods
#[test]
fn test_move_is_promotion() {
    let from = Position::new(4, 6).unwrap(); // e7
    let to = Position::new(4, 7).unwrap(); // e8

    // Move without promotion
    let simple_move = Move::new(from, to, None).unwrap();
    assert!(
        !simple_move.is_promotion(),
        "Move without promotion should return false"
    );

    // Move with promotion
    let promotion_move = Move::new(from, to, Some(PieceType::Queen)).unwrap();
    assert!(
        promotion_move.is_promotion(),
        "Move with promotion should return true"
    );

    // Test all valid promotion types
    let promotion_pieces = [
        PieceType::Queen,
        PieceType::Rook,
        PieceType::Bishop,
        PieceType::Knight,
    ];
    for piece in &promotion_pieces {
        let move_with_promotion = Move::new(from, to, Some(*piece)).unwrap();
        assert!(
            move_with_promotion.is_promotion(),
            "Move with {:?} promotion should return true",
            piece
        );
    }
}

/// Test Move detection for castling
#[test]
fn test_move_is_castling() {
    // Test kingside castling (e1g1)
    let kingside = Move::new(
        Position::new(4, 0).unwrap(), // e1
        Position::new(6, 0).unwrap(), // g1
        None,
    )
    .unwrap();
    assert!(
        kingside.is_castling(),
        "Kingside castling should be detected"
    );

    // Test queenside castling (e1c1)
    let queenside = Move::new(
        Position::new(4, 0).unwrap(), // e1
        Position::new(2, 0).unwrap(), // c1
        None,
    )
    .unwrap();
    assert!(
        queenside.is_castling(),
        "Queenside castling should be detected"
    );

    // Test black castling (e8g8)
    let black_kingside = Move::new(
        Position::new(4, 7).unwrap(), // e8
        Position::new(6, 7).unwrap(), // g8
        None,
    )
    .unwrap();
    assert!(
        black_kingside.is_castling(),
        "Black kingside castling should be detected"
    );

    // Test non-castling moves
    let regular_move = Move::new(
        Position::new(4, 1).unwrap(), // e2
        Position::new(4, 3).unwrap(), // e4
        None,
    )
    .unwrap();
    assert!(
        !regular_move.is_castling(),
        "Regular move should not be castling"
    );

    let one_square_move = Move::new(
        Position::new(4, 0).unwrap(), // e1
        Position::new(5, 0).unwrap(), // f1
        None,
    )
    .unwrap();
    assert!(
        !one_square_move.is_castling(),
        "One square move should not be castling"
    );

    let vertical_move = Move::new(
        Position::new(4, 0).unwrap(), // e1
        Position::new(4, 2).unwrap(), // e3
        None,
    )
    .unwrap();
    assert!(
        !vertical_move.is_castling(),
        "Vertical move should not be castling"
    );
}

/// Test Move detection for en passant candidate
#[test]
fn test_move_is_en_passant_candidate() {
    // Test diagonal moves that could be en passant
    let en_passant_candidate = Move::new(
        Position::new(4, 4).unwrap(), // e5
        Position::new(3, 5).unwrap(), // d6
        None,
    )
    .unwrap();
    assert!(
        en_passant_candidate.is_en_passant_candidate(),
        "Diagonal one-rank move should be en passant candidate"
    );

    let another_candidate = Move::new(
        Position::new(2, 3).unwrap(), // c4
        Position::new(1, 2).unwrap(), // b3
        None,
    )
    .unwrap();
    assert!(
        another_candidate.is_en_passant_candidate(),
        "Another diagonal move should be en passant candidate"
    );

    // Test moves that are not en passant candidates
    let horizontal_move = Move::new(
        Position::new(4, 4).unwrap(), // e5
        Position::new(5, 4).unwrap(), // f5
        None,
    )
    .unwrap();
    assert!(
        !horizontal_move.is_en_passant_candidate(),
        "Horizontal move should not be en passant candidate"
    );

    let vertical_move = Move::new(
        Position::new(4, 4).unwrap(), // e5
        Position::new(4, 5).unwrap(), // e6
        None,
    )
    .unwrap();
    assert!(
        !vertical_move.is_en_passant_candidate(),
        "Vertical move should not be en passant candidate"
    );

    let two_rank_diagonal = Move::new(
        Position::new(4, 4).unwrap(), // e5
        Position::new(3, 6).unwrap(), // d7
        None,
    )
    .unwrap();
    assert!(
        !two_rank_diagonal.is_en_passant_candidate(),
        "Two-rank diagonal should not be en passant candidate"
    );

    let same_file_move = Move::new(
        Position::new(4, 4).unwrap(), // e5
        Position::new(4, 5).unwrap(), // e6
        None,
    )
    .unwrap();
    assert!(
        !same_file_move.is_en_passant_candidate(),
        "Same file move should not be en passant candidate"
    );
}

/// Test Move display formatting
#[test]
fn test_move_display() {
    // Test basic move display
    let basic_move = Move::new(
        Position::new(4, 1).unwrap(), // e2
        Position::new(4, 3).unwrap(), // e4
        None,
    )
    .unwrap();
    assert_eq!(basic_move.to_string(), "e2e4");

    // Test promotion display
    let promotion_move = Move::new(
        Position::new(4, 6).unwrap(), // e7
        Position::new(4, 7).unwrap(), // e8
        Some(PieceType::Queen),
    )
    .unwrap();
    assert_eq!(promotion_move.to_string(), "e7e8Q");

    // Test other moves
    let test_cases = [
        (
            Position::new(0, 0).unwrap(),
            Position::new(7, 7).unwrap(),
            None,
            "a1h8",
        ),
        (
            Position::new(3, 0).unwrap(),
            Position::new(3, 7).unwrap(),
            Some(PieceType::Rook),
            "d1d8R",
        ),
        (
            Position::new(7, 6).unwrap(),
            Position::new(7, 7).unwrap(),
            Some(PieceType::Knight),
            "h7h8N",
        ),
    ];

    for (from, to, promotion, expected) in &test_cases {
        let chess_move = Move::new(*from, *to, *promotion).unwrap();
        assert_eq!(chess_move.to_string(), *expected);
    }
}

/// Test JSON serialization methods
#[test]
fn test_move_json_serialization() {
    // Test basic move JSON
    let basic_move = Move::new(
        Position::new(4, 1).unwrap(), // e2
        Position::new(4, 3).unwrap(), // e4
        None,
    )
    .unwrap();

    let json = basic_move.to_json();
    assert_eq!(json["from"], "e2");
    assert_eq!(json["to"], "e4");
    assert!(json["promotion"].is_null());

    // Test promotion move JSON
    let promotion_move = Move::new(
        Position::new(4, 6).unwrap(), // e7
        Position::new(4, 7).unwrap(), // e8
        Some(PieceType::Queen),
    )
    .unwrap();

    let promotion_json = promotion_move.to_json();
    assert_eq!(promotion_json["from"], "e7");
    assert_eq!(promotion_json["to"], "e8");
    assert_eq!(promotion_json["promotion"], "Q");
}

/// Test JSON deserialization methods
#[test]
fn test_move_json_deserialization() {
    // Test basic move from JSON
    let basic_json = serde_json::json!({
        "from": "e2",
        "to": "e4"
    });

    let basic_move = Move::from_json(&basic_json).expect("Should parse basic JSON");
    assert_eq!(basic_move.from, Position::new(4, 1).unwrap());
    assert_eq!(basic_move.to, Position::new(4, 3).unwrap());
    assert_eq!(basic_move.promotion, None);

    // Test promotion move from JSON
    let promotion_json = serde_json::json!({
        "from": "e7",
        "to": "e8",
        "promotion": "Q"
    });

    let promotion_move = Move::from_json(&promotion_json).expect("Should parse promotion JSON");
    assert_eq!(promotion_move.from, Position::new(4, 6).unwrap());
    assert_eq!(promotion_move.to, Position::new(4, 7).unwrap());
    assert_eq!(promotion_move.promotion, Some(PieceType::Queen));

    // Test invalid JSON
    let invalid_jsons = [
        serde_json::json!({}),                         // Missing fields
        serde_json::json!({"from": "e2"}),             // Missing to
        serde_json::json!({"to": "e4"}),               // Missing from
        serde_json::json!({"from": "e2", "to": "e2"}), // Same positions
        serde_json::json!({"from": "e2", "to": "e4", "promotion": "K"}), // Invalid promotion
        serde_json::json!({"from": "invalid", "to": "e4"}), // Invalid position
    ];

    for invalid_json in &invalid_jsons {
        let result = Move::from_json(invalid_json);
        assert!(
            result.is_err(),
            "Invalid JSON should fail: {:?}",
            invalid_json
        );
    }
}

/// Test JSON roundtrip consistency
#[test]
fn test_move_json_roundtrip() {
    let test_moves = [
        Move::new(
            Position::new(4, 1).unwrap(),
            Position::new(4, 3).unwrap(),
            None,
        )
        .unwrap(),
        Move::new(
            Position::new(0, 0).unwrap(),
            Position::new(7, 7).unwrap(),
            None,
        )
        .unwrap(),
        Move::new(
            Position::new(4, 6).unwrap(),
            Position::new(4, 7).unwrap(),
            Some(PieceType::Queen),
        )
        .unwrap(),
        Move::new(
            Position::new(1, 6).unwrap(),
            Position::new(1, 7).unwrap(),
            Some(PieceType::Rook),
        )
        .unwrap(),
        Move::new(
            Position::new(2, 6).unwrap(),
            Position::new(2, 7).unwrap(),
            Some(PieceType::Bishop),
        )
        .unwrap(),
        Move::new(
            Position::new(3, 6).unwrap(),
            Position::new(3, 7).unwrap(),
            Some(PieceType::Knight),
        )
        .unwrap(),
    ];

    for original_move in &test_moves {
        let json = original_move.to_json();
        let roundtrip_move = Move::from_json(&json).expect("JSON roundtrip should work");
        assert_eq!(
            *original_move, roundtrip_move,
            "Roundtrip should preserve move"
        );
    }
}

/// Test serde serialization/deserialization
#[test]
fn test_move_serde_serialization() {
    let test_moves = [
        Move::new(
            Position::new(4, 1).unwrap(),
            Position::new(4, 3).unwrap(),
            None,
        )
        .unwrap(),
        Move::new(
            Position::new(4, 6).unwrap(),
            Position::new(4, 7).unwrap(),
            Some(PieceType::Queen),
        )
        .unwrap(),
    ];

    for original_move in &test_moves {
        // Test JSON serialization
        let json_str = serde_json::to_string(original_move).expect("Should serialize to JSON");
        let deserialized_move: Move =
            serde_json::from_str(&json_str).expect("Should deserialize from JSON");
        assert_eq!(
            *original_move, deserialized_move,
            "JSON serde roundtrip should work"
        );

        // Test other serialization formats if needed
        // Could add tests for bincode, etc.
    }
}

/// Test Move trait implementations
#[test]
fn test_move_trait_implementations() {
    let move1 = Move::new(
        Position::new(4, 1).unwrap(),
        Position::new(4, 3).unwrap(),
        None,
    )
    .unwrap();
    let move2 = Move::new(
        Position::new(0, 0).unwrap(),
        Position::new(7, 7).unwrap(),
        None,
    )
    .unwrap();
    let move1_copy = Move::new(
        Position::new(4, 1).unwrap(),
        Position::new(4, 3).unwrap(),
        None,
    )
    .unwrap();

    // Test Debug
    let debug_str = format!("{:?}", move1);
    assert!(debug_str.contains("Move"));
    assert!(debug_str.contains("from"));
    assert!(debug_str.contains("to"));

    // Test Clone
    let move1_clone = move1;
    assert_eq!(move1, move1_clone);

    // Test Copy (implicit test - should compile)
    let move1_copied = move1;
    assert_eq!(move1, move1_copied);

    // Test PartialEq and Eq
    assert_eq!(move1, move1_copy);
    assert_ne!(move1, move2);

    // Test Hash
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert(move1, "first move");
    map.insert(move2, "second move");
    assert_eq!(map.get(&move1_copy), Some(&"first move"));
}

/// Test parsing roundtrip property
#[test]
fn test_move_parsing_roundtrip_property() {
    // Property: move.to_string().parse() == move for all valid moves
    let test_moves = [
        Move::new(
            Position::new(4, 1).unwrap(),
            Position::new(4, 3).unwrap(),
            None,
        )
        .unwrap(),
        Move::new(
            Position::new(0, 0).unwrap(),
            Position::new(7, 7).unwrap(),
            None,
        )
        .unwrap(),
        Move::new(
            Position::new(4, 6).unwrap(),
            Position::new(4, 7).unwrap(),
            Some(PieceType::Queen),
        )
        .unwrap(),
        Move::new(
            Position::new(1, 6).unwrap(),
            Position::new(1, 7).unwrap(),
            Some(PieceType::Rook),
        )
        .unwrap(),
        Move::new(
            Position::new(2, 6).unwrap(),
            Position::new(2, 7).unwrap(),
            Some(PieceType::Bishop),
        )
        .unwrap(),
        Move::new(
            Position::new(3, 6).unwrap(),
            Position::new(3, 7).unwrap(),
            Some(PieceType::Knight),
        )
        .unwrap(),
    ];

    for original_move in &test_moves {
        let move_string = original_move.to_string();
        let parsed_move = move_string
            .parse::<Move>()
            .unwrap_or_else(|_| panic!("Should parse {}", move_string));
        assert_eq!(
            *original_move, parsed_move,
            "move.to_string().parse() should equal original move for {:?}",
            original_move
        );
    }
}

/// Test error handling consistency
#[test]
fn test_move_error_consistency() {
    // Test that all move creation errors use ChessError::InvalidMove
    let pos = Position::new(4, 4).unwrap();

    match Move::new(pos, pos, None) {
        Ok(_) => panic!("Same position move should fail"),
        Err(ChessError::InvalidMove(_)) => {
            // This is expected
        }
        Err(other) => panic!("Expected InvalidMove error, got: {:?}", other),
    }

    // Test invalid promotion errors
    let from = Position::new(4, 6).unwrap();
    let to = Position::new(4, 7).unwrap();

    for invalid_piece in &[PieceType::King, PieceType::Pawn] {
        match Move::new(from, to, Some(*invalid_piece)) {
            Ok(_) => panic!("Invalid promotion should fail"),
            Err(ChessError::InvalidMove(_)) => {
                // This is expected
            }
            Err(other) => panic!("Expected InvalidMove error, got: {:?}", other),
        }
    }
}
