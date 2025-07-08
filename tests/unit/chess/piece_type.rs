use mate::chess::{ChessError, PieceType};

/// Test parsing valid piece type inputs
#[test]
fn test_piece_type_parse_valid_inputs() {
    // Test single character inputs (case insensitive)
    assert_eq!("P".parse::<PieceType>().unwrap(), PieceType::Pawn);
    assert_eq!("p".parse::<PieceType>().unwrap(), PieceType::Pawn);

    assert_eq!("R".parse::<PieceType>().unwrap(), PieceType::Rook);
    assert_eq!("r".parse::<PieceType>().unwrap(), PieceType::Rook);

    assert_eq!("N".parse::<PieceType>().unwrap(), PieceType::Knight);
    assert_eq!("n".parse::<PieceType>().unwrap(), PieceType::Knight);

    assert_eq!("B".parse::<PieceType>().unwrap(), PieceType::Bishop);
    assert_eq!("b".parse::<PieceType>().unwrap(), PieceType::Bishop);

    assert_eq!("Q".parse::<PieceType>().unwrap(), PieceType::Queen);
    assert_eq!("q".parse::<PieceType>().unwrap(), PieceType::Queen);

    assert_eq!("K".parse::<PieceType>().unwrap(), PieceType::King);
    assert_eq!("k".parse::<PieceType>().unwrap(), PieceType::King);

    // Test full names (case insensitive)
    assert_eq!("PAWN".parse::<PieceType>().unwrap(), PieceType::Pawn);
    assert_eq!("pawn".parse::<PieceType>().unwrap(), PieceType::Pawn);
    assert_eq!("Pawn".parse::<PieceType>().unwrap(), PieceType::Pawn);
    assert_eq!("PaWn".parse::<PieceType>().unwrap(), PieceType::Pawn);

    assert_eq!("ROOK".parse::<PieceType>().unwrap(), PieceType::Rook);
    assert_eq!("rook".parse::<PieceType>().unwrap(), PieceType::Rook);
    assert_eq!("Rook".parse::<PieceType>().unwrap(), PieceType::Rook);
    assert_eq!("RoOk".parse::<PieceType>().unwrap(), PieceType::Rook);

    assert_eq!("KNIGHT".parse::<PieceType>().unwrap(), PieceType::Knight);
    assert_eq!("knight".parse::<PieceType>().unwrap(), PieceType::Knight);
    assert_eq!("Knight".parse::<PieceType>().unwrap(), PieceType::Knight);
    assert_eq!("KnIgHt".parse::<PieceType>().unwrap(), PieceType::Knight);

    assert_eq!("BISHOP".parse::<PieceType>().unwrap(), PieceType::Bishop);
    assert_eq!("bishop".parse::<PieceType>().unwrap(), PieceType::Bishop);
    assert_eq!("Bishop".parse::<PieceType>().unwrap(), PieceType::Bishop);
    assert_eq!("BiShOp".parse::<PieceType>().unwrap(), PieceType::Bishop);

    assert_eq!("QUEEN".parse::<PieceType>().unwrap(), PieceType::Queen);
    assert_eq!("queen".parse::<PieceType>().unwrap(), PieceType::Queen);
    assert_eq!("Queen".parse::<PieceType>().unwrap(), PieceType::Queen);
    assert_eq!("QuEeN".parse::<PieceType>().unwrap(), PieceType::Queen);

    assert_eq!("KING".parse::<PieceType>().unwrap(), PieceType::King);
    assert_eq!("king".parse::<PieceType>().unwrap(), PieceType::King);
    assert_eq!("King".parse::<PieceType>().unwrap(), PieceType::King);
    assert_eq!("KiNg".parse::<PieceType>().unwrap(), PieceType::King);
}

/// Test parsing invalid piece type inputs returns ChessError::InvalidPieceType
#[test]
fn test_piece_type_parse_invalid_inputs() {
    let invalid_inputs = [
        "", "X", "Z", "1", "2", "pawn ", " pawn", "pawn\n", "rook\t", "p a w n", "r-o-o-k",
        "knight1", "bishop2", "queeen", "kingg", "peasant", "castle", "horse", "tower", "minister",
        "monarch",
    ];

    for input in &invalid_inputs {
        let result = input.parse::<PieceType>();
        assert!(result.is_err(), "Input '{}' should fail to parse", input);

        match result.unwrap_err() {
            ChessError::InvalidPieceType(msg) => {
                assert!(
                    msg.contains(input),
                    "Error message should contain the invalid input '{}'. Got: {}",
                    input,
                    msg
                );
                assert!(
                    msg.contains("Expected one of: P, R, N, B, Q, K"),
                    "Error message should list valid options. Got: {}",
                    msg
                );
            }
            other => panic!("Expected InvalidPieceType error, got: {:?}", other),
        }
    }
}

/// Test Display trait output for single characters
#[test]
fn test_piece_type_display_output() {
    assert_eq!(PieceType::Pawn.to_string(), "P");
    assert_eq!(PieceType::Rook.to_string(), "R");
    assert_eq!(PieceType::Knight.to_string(), "N");
    assert_eq!(PieceType::Bishop.to_string(), "B");
    assert_eq!(PieceType::Queen.to_string(), "Q");
    assert_eq!(PieceType::King.to_string(), "K");

    // Test format! macro usage
    assert_eq!(format!("{pawn}", pawn = PieceType::Pawn), "P");
    assert_eq!(format!("{rook}", rook = PieceType::Rook), "R");
    assert_eq!(format!("{knight}", knight = PieceType::Knight), "N");
    assert_eq!(format!("{bishop}", bishop = PieceType::Bishop), "B");
    assert_eq!(format!("{queen}", queen = PieceType::Queen), "Q");
    assert_eq!(format!("{king}", king = PieceType::King), "K");
}

/// Test piece values according to standard chess values
#[test]
fn test_piece_type_values() {
    assert_eq!(PieceType::Pawn.value(), 1);
    assert_eq!(PieceType::Knight.value(), 3);
    assert_eq!(PieceType::Bishop.value(), 3);
    assert_eq!(PieceType::Rook.value(), 5);
    assert_eq!(PieceType::Queen.value(), 9);
    assert_eq!(PieceType::King.value(), 0); // King is invaluable
}

/// Test PieceType enum implements required traits
#[test]
fn test_piece_type_trait_implementations() {
    let pawn = PieceType::Pawn;
    let queen = PieceType::Queen;

    // Test Debug
    assert!(format!("{pawn:?}").contains("Pawn"));
    assert!(format!("{queen:?}").contains("Queen"));

    // Test Clone
    let pawn_clone = pawn;
    assert_eq!(pawn, pawn_clone);

    // Test Copy (implicit test - should compile)
    let pawn_copy = pawn;
    assert_eq!(pawn, pawn_copy);

    // Test PartialEq
    assert_eq!(pawn, PieceType::Pawn);
    assert_ne!(pawn, PieceType::Queen);

    // Test Eq (can't test directly, but PartialEq + Eq allows HashMap usage)
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert(pawn, "pawn piece");
    map.insert(queen, "queen piece");
    assert_eq!(map.get(&PieceType::Pawn), Some(&"pawn piece"));
    assert_eq!(map.get(&PieceType::Queen), Some(&"queen piece"));

    // Test Hash (implicit through HashMap usage above)

    // Test serialization/deserialization
    let serialized = serde_json::to_string(&pawn).expect("Serialization should work");
    let deserialized: PieceType =
        serde_json::from_str(&serialized).expect("Deserialization should work");
    assert_eq!(pawn, deserialized);
}

/// Test parsing roundtrip consistency
#[test]
fn test_piece_type_parsing_roundtrip() {
    let all_pieces = [
        PieceType::Pawn,
        PieceType::Rook,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Queen,
        PieceType::King,
    ];

    for piece_type in &all_pieces {
        // Test that display -> parse preserves the piece type
        let displayed = piece_type.to_string();
        let parsed = displayed
            .parse::<PieceType>()
            .expect("Display output should parse back");
        assert_eq!(
            *piece_type, parsed,
            "Display -> parse roundtrip should preserve value"
        );
    }

    // Test various valid inputs all parse correctly
    let test_cases = [
        (PieceType::Pawn, vec!["P", "p", "PAWN", "pawn", "Pawn"]),
        (PieceType::Rook, vec!["R", "r", "ROOK", "rook", "Rook"]),
        (
            PieceType::Knight,
            vec!["N", "n", "KNIGHT", "knight", "Knight"],
        ),
        (
            PieceType::Bishop,
            vec!["B", "b", "BISHOP", "bishop", "Bishop"],
        ),
        (PieceType::Queen, vec!["Q", "q", "QUEEN", "queen", "Queen"]),
        (PieceType::King, vec!["K", "k", "KING", "king", "King"]),
    ];

    for (expected_piece, inputs) in &test_cases {
        for input in inputs {
            assert_eq!(
                input.parse::<PieceType>().unwrap(),
                *expected_piece,
                "Input '{}' should parse to {:?}",
                input,
                expected_piece
            );
        }
    }
}

/// Test error message quality and consistency
#[test]
fn test_piece_type_error_messages() {
    let invalid_input = "dragon";
    let result = invalid_input.parse::<PieceType>();

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Test error display
    let error_string = error.to_string();
    assert!(error_string.contains("Invalid piece type"));
    assert!(error_string.contains("dragon"));

    // Test that error is descriptive and actionable
    match error {
        ChessError::InvalidPieceType(msg) => {
            assert!(msg.contains("Expected one of: P, R, N, B, Q, K"));
            assert!(msg.contains("dragon"));
        }
        _ => panic!("Expected InvalidPieceType error"),
    }
}

/// Test all piece types have unique display values
#[test]
fn test_piece_type_unique_display_values() {
    use std::collections::HashSet;

    let all_pieces = [
        PieceType::Pawn,
        PieceType::Rook,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Queen,
        PieceType::King,
    ];

    let display_values: HashSet<String> =
        all_pieces.iter().map(|piece| piece.to_string()).collect();

    assert_eq!(
        display_values.len(),
        all_pieces.len(),
        "All piece types should have unique display values"
    );

    // Verify the exact display values
    assert!(display_values.contains("P"));
    assert!(display_values.contains("R"));
    assert!(display_values.contains("N"));
    assert!(display_values.contains("B"));
    assert!(display_values.contains("Q"));
    assert!(display_values.contains("K"));
}

/// Test piece type value consistency and expected ranges
#[test]
fn test_piece_type_value_consistency() {
    let all_pieces = [
        PieceType::Pawn,
        PieceType::Rook,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Queen,
        PieceType::King,
    ];

    // Test that all values are in expected range (0-9)
    for piece in &all_pieces {
        let value = piece.value();
        assert!(
            value <= 9,
            "Piece value should be <= 9, got {} for {:?}",
            value,
            piece
        );
    }

    // Test relative piece values (chess theory)
    assert!(PieceType::Queen.value() > PieceType::Rook.value());
    assert!(PieceType::Rook.value() > PieceType::Bishop.value());
    assert!(PieceType::Rook.value() > PieceType::Knight.value());
    assert!(PieceType::Bishop.value() > PieceType::Pawn.value());
    assert!(PieceType::Knight.value() > PieceType::Pawn.value());
    assert_eq!(PieceType::Bishop.value(), PieceType::Knight.value()); // Equal value
    assert_eq!(PieceType::King.value(), 0); // Special case - invaluable
}
