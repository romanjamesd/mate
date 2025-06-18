//! Chess Error Consistency Integration Tests
//!
//! Tests for Chess Error Consistency as specified in tests.md:
//! - All parsing errors use ChessError enum consistently
//! - Error messages are descriptive and actionable  
//! - Error display is human-readable format

use mate::chess::{ChessError, Color, Move, PieceType, Position};
use std::collections::HashSet;

/// Test that all parsing errors use ChessError enum consistently
#[test]
fn test_all_parsing_errors_use_chess_error_consistently() {
    println!("Testing that all parsing errors use ChessError enum consistently");

    // Test Color parsing errors
    let invalid_colors = [
        "red",
        "blue",
        "green",
        "yellow",
        "",
        "1",
        "true",
        "false",
        "null",
        "undefined",
    ];

    for invalid_color in &invalid_colors {
        match invalid_color.parse::<Color>() {
            Ok(_) => panic!("Expected parsing '{}' to fail", invalid_color),
            Err(e) => {
                assert!(
                    matches!(e, ChessError::InvalidColor(_)),
                    "Color parsing should return ChessError::InvalidColor, got: {:?}",
                    e
                );
            }
        }
    }

    // Test PieceType parsing errors
    let invalid_piece_types = [
        "X",
        "Y",
        "Z",
        "",
        "PIECE",
        "INVALID",
        "1",
        "true",
        "false",
        "null",
        "undefined",
        "piece",
        "rook ",
        " knight",
    ];

    for invalid_piece_type in &invalid_piece_types {
        match invalid_piece_type.parse::<PieceType>() {
            Ok(_) => panic!("Expected parsing '{}' to fail", invalid_piece_type),
            Err(e) => {
                assert!(
                    matches!(e, ChessError::InvalidPieceType(_)),
                    "PieceType parsing should return ChessError::InvalidPieceType, got: {:?}",
                    e
                );
            }
        }
    }

    // Test Position parsing errors
    let invalid_positions = [
        "", "a", "1", "a0", "a9", "i1", "h9", "z5", "a11", "aa", "11", "abc", "a1b", "1a",
        "invalid", "null",
    ];

    for invalid_position in &invalid_positions {
        match invalid_position.parse::<Position>() {
            Ok(_) => panic!("Expected parsing '{}' to fail", invalid_position),
            Err(e) => {
                assert!(
                    matches!(e, ChessError::InvalidPosition(_)),
                    "Position parsing should return ChessError::InvalidPosition, got: {:?}",
                    e
                );
            }
        }
    }

    // Test Move parsing errors
    let invalid_moves = [
        "", "e2", "e2e", "e2e2", "e2e9", "i2e4", "e2i4", "e2e4x", "e2e4qq", "O-O-O-O", "0-0-0-0",
        "invalid", "null", "e2e4k", "e2e4p",
    ];

    for invalid_move in &invalid_moves {
        match invalid_move.parse::<Move>() {
            Ok(_) => panic!("Expected parsing '{}' to fail", invalid_move),
            Err(e) => {
                assert!(
                    matches!(e, ChessError::InvalidMove(_))
                        || matches!(e, ChessError::InvalidPosition(_))
                        || matches!(e, ChessError::InvalidPieceType(_)),
                    "Move parsing should return a ChessError variant, got: {:?}",
                    e
                );
            }
        }
    }

    // Test Position creation errors
    let invalid_coordinates = [(8, 0), (0, 8), (9, 9), (255, 255)];

    for (file, rank) in &invalid_coordinates {
        match Position::new(*file, *rank) {
            Ok(_) => panic!("Expected creating position ({}, {}) to fail", file, rank),
            Err(e) => {
                assert!(
                    matches!(e, ChessError::InvalidPosition(_)),
                    "Position creation should return ChessError::InvalidPosition, got: {:?}",
                    e
                );
            }
        }
    }

    // Test Move creation errors
    let same_position = Position::new(0, 0).unwrap();
    match Move::new(same_position, same_position, None) {
        Ok(_) => panic!("Expected creating move with same from/to to fail"),
        Err(e) => {
            assert!(
                matches!(e, ChessError::InvalidMove(_)),
                "Move with same from/to should return ChessError::InvalidMove, got: {:?}",
                e
            );
        }
    }

    // Test invalid promotion errors
    let from = Position::new(0, 6).unwrap();
    let to = Position::new(0, 7).unwrap();
    let invalid_promotions = [PieceType::King, PieceType::Pawn];

    for invalid_promotion in &invalid_promotions {
        match Move::new(from, to, Some(*invalid_promotion)) {
            Ok(_) => panic!("Expected promotion to {:?} to fail", invalid_promotion),
            Err(e) => {
                assert!(
                    matches!(e, ChessError::InvalidMove(_)),
                    "Invalid promotion should return ChessError::InvalidMove, got: {:?}",
                    e
                );
            }
        }
    }

    println!("✅ All parsing errors use ChessError enum consistently");
}

/// Test that error messages are descriptive and actionable
#[test]
fn test_error_messages_are_descriptive_and_actionable() {
    println!("Testing that error messages are descriptive and actionable");

    // Test Color error messages
    match "invalid_color".parse::<Color>() {
        Err(ChessError::InvalidColor(msg)) => {
            assert!(
                msg.contains("invalid_color"),
                "Error message should include the invalid input: {}",
                msg
            );
            assert!(
                msg.contains("Expected") || msg.contains("got"),
                "Error message should be descriptive: {}",
                msg
            );
            assert!(
                msg.len() > 10,
                "Error message should be sufficiently detailed: {}",
                msg
            );
        }
        _ => panic!("Expected ChessError::InvalidColor"),
    }

    // Test PieceType error messages
    match "X".parse::<PieceType>() {
        Err(ChessError::InvalidPieceType(msg)) => {
            assert!(
                msg.contains("X"),
                "Error message should include the invalid input: {}",
                msg
            );
            assert!(
                msg.contains("Expected") || msg.contains("got"),
                "Error message should be descriptive: {}",
                msg
            );
            assert!(
                msg.contains("P")
                    || msg.contains("R")
                    || msg.contains("N")
                    || msg.contains("B")
                    || msg.contains("Q")
                    || msg.contains("K"),
                "Error message should suggest valid options: {}",
                msg
            );
            assert!(
                msg.len() > 10,
                "Error message should be sufficiently detailed: {}",
                msg
            );
        }
        _ => panic!("Expected ChessError::InvalidPieceType"),
    }

    // Test Position error messages
    match "z9".parse::<Position>() {
        Err(ChessError::InvalidPosition(msg)) => {
            assert!(
                msg.contains("z") || msg.contains("9"),
                "Error message should include the invalid part: {}",
                msg
            );
            assert!(
                msg.len() > 10,
                "Error message should be sufficiently detailed: {}",
                msg
            );
        }
        _ => panic!("Expected ChessError::InvalidPosition"),
    }

    // Test Position creation error messages
    match Position::new(8, 0) {
        Err(ChessError::InvalidPosition(msg)) => {
            assert!(
                msg.contains("8") || msg.contains("File"),
                "Error message should mention the invalid coordinate: {}",
                msg
            );
            assert!(
                msg.contains("0-7") || msg.contains("must be"),
                "Error message should specify the valid range: {}",
                msg
            );
            assert!(
                msg.len() > 10,
                "Error message should be sufficiently detailed: {}",
                msg
            );
        }
        _ => panic!("Expected ChessError::InvalidPosition"),
    }

    // Test Move error messages
    match "invalid_move".parse::<Move>() {
        Err(ChessError::InvalidMove(msg)) => {
            assert!(
                msg.contains("invalid_move") || msg.contains("format"),
                "Error message should be specific: {}",
                msg
            );
            assert!(
                msg.contains("Expected") || msg.contains("e2e4") || msg.contains("O-O"),
                "Error message should provide examples: {}",
                msg
            );
            assert!(
                msg.len() > 20,
                "Error message should be sufficiently detailed: {}",
                msg
            );
        }
        _ => panic!("Expected ChessError::InvalidMove"),
    }

    // Test Move creation error messages
    let same_pos = Position::new(0, 0).unwrap();
    match Move::new(same_pos, same_pos, None) {
        Err(ChessError::InvalidMove(msg)) => {
            assert!(
                msg.contains("same") || msg.contains("cannot"),
                "Error message should explain the problem: {}",
                msg
            );
            assert!(
                msg.len() > 10,
                "Error message should be sufficiently detailed: {}",
                msg
            );
        }
        _ => panic!("Expected ChessError::InvalidMove"),
    }

    // Test invalid promotion error messages
    let from = Position::new(0, 6).unwrap();
    let to = Position::new(0, 7).unwrap();
    match Move::new(from, to, Some(PieceType::King)) {
        Err(ChessError::InvalidMove(msg)) => {
            assert!(
                msg.contains("King") || msg.contains("Pawn") || msg.contains("promote"),
                "Error message should explain promotion rules: {}",
                msg
            );
            assert!(
                msg.len() > 10,
                "Error message should be sufficiently detailed: {}",
                msg
            );
        }
        _ => panic!("Expected ChessError::InvalidMove"),
    }

    println!("✅ All error messages are descriptive and actionable");
}

/// Test that error display is human-readable format
#[test]
fn test_error_display_is_human_readable() {
    println!("Testing that error display is human-readable format");

    // Test that all error variants display properly
    let errors = [
        ChessError::InvalidColor("test color error".to_string()),
        ChessError::InvalidPieceType("test piece type error".to_string()),
        ChessError::InvalidPosition("test position error".to_string()),
        ChessError::InvalidMove("test move error".to_string()),
        ChessError::InvalidFen("test fen error".to_string()),
        ChessError::BoardStateError("test board state error".to_string()),
    ];

    let mut displayed_messages = HashSet::new();

    for error in &errors {
        let display_string = format!("{}", error);

        // Test basic readability requirements
        assert!(
            !display_string.is_empty(),
            "Error display should not be empty"
        );
        assert!(
            !display_string.starts_with("InvalidColor")
                && !display_string.starts_with("InvalidPieceType")
                && !display_string.starts_with("InvalidPosition")
                && !display_string.starts_with("InvalidMove")
                && !display_string.starts_with("InvalidFen")
                && !display_string.starts_with("BoardStateError"),
            "Error display should not start with enum variant name: {}",
            display_string
        );

        // Test that display includes some context (very basic check)
        assert!(
            !display_string.trim().is_empty(),
            "Error display should not be empty or just whitespace: '{}'",
            display_string
        );

        // Test message length is reasonable
        assert!(
            display_string.len() >= 10,
            "Error display should be descriptive enough: {}",
            display_string
        );
        assert!(
            display_string.len() <= 200,
            "Error display should not be too verbose: {}",
            display_string
        );

        // Test no special characters that would break terminal display
        assert!(
            !display_string.contains('\0'),
            "Error display should not contain null characters"
        );
        assert!(
            !display_string.contains('\x1b'),
            "Error display should not contain escape sequences"
        );

        // Test uniqueness of error messages
        assert!(
            displayed_messages.insert(display_string.clone()),
            "Error displays should be unique: {}",
            display_string
        );

        // Test that all error types can be matched (ensures no dead code in match arms)
        match error {
            ChessError::InvalidColor(_msg) => {
                // Error type is correctly matched - no specific content assertions needed
            }
            ChessError::InvalidPieceType(_msg) => {
                // Error type is correctly matched - no specific content assertions needed
            }
            ChessError::InvalidPosition(_msg) => {
                // Error type is correctly matched - no specific content assertions needed
            }
            ChessError::InvalidMove(_msg) => {
                // Error type is correctly matched - no specific content assertions needed
            }
            ChessError::InvalidFen(_msg) => {
                // Error type is correctly matched - no specific content assertions needed
            }
            ChessError::BoardStateError(_msg) => {
                // Error type is correctly matched - no specific content assertions needed
            }
        }
    }

    // Test Debug trait also produces readable output
    for error in &errors {
        let debug_string = format!("{:?}", error);
        assert!(!debug_string.is_empty(), "Error debug should not be empty");
        assert!(
            debug_string.len() >= 10,
            "Error debug should be descriptive: {}",
            debug_string
        );
    }

    // Test std::error::Error trait
    for error in &errors {
        let error_trait: &dyn std::error::Error = error;
        let source = error_trait.source();
        assert!(
            source.is_none(),
            "ChessError should not have a source error"
        );
    }

    println!("✅ All error displays are human-readable");
}

/// Test comprehensive error consistency across all chess types
#[test]
fn test_comprehensive_error_consistency() {
    println!("Testing comprehensive error consistency across all chess types");

    // Collect all error types that should be returned
    let mut color_errors = Vec::new();
    let mut piece_type_errors = Vec::new();
    let mut position_errors = Vec::new();
    let mut move_errors = Vec::new();

    // Generate comprehensive test cases for each type
    let test_inputs = [
        "",
        " ",
        "  ",
        "invalid",
        "null",
        "undefined",
        "123",
        "true",
        "false",
        "INVALID",
        "Invalid",
        "test",
        "xyz",
        "[]",
        "{}",
        "()",
        "!",
        "@",
        "#",
        "$",
        "%",
        "^",
        "&",
        "*",
        "+",
        "=",
        "|",
        "\\",
        "/",
        "?",
        "<",
        ">",
        ".",
        ",",
        ";",
        ":",
        "\"",
        "'",
        "`",
        "~",
        "\n",
        "\t",
        "\r",
    ];

    // Test all inputs against all types
    for input in &test_inputs {
        // Color parsing
        if let Err(e) = input.parse::<Color>() {
            color_errors.push(e);
        }

        // PieceType parsing
        if let Err(e) = input.parse::<PieceType>() {
            piece_type_errors.push(e);
        }

        // Position parsing
        if let Err(e) = input.parse::<Position>() {
            position_errors.push(e);
        }

        // Move parsing
        if let Err(e) = input.parse::<Move>() {
            move_errors.push(e);
        }
    }

    // Verify all errors are properly categorized
    for error in &color_errors {
        assert!(
            matches!(error, ChessError::InvalidColor(_)),
            "All color parsing errors should be InvalidColor: {:?}",
            error
        );
    }

    for error in &piece_type_errors {
        assert!(
            matches!(error, ChessError::InvalidPieceType(_)),
            "All piece type parsing errors should be InvalidPieceType: {:?}",
            error
        );
    }

    for error in &position_errors {
        assert!(
            matches!(error, ChessError::InvalidPosition(_)),
            "All position parsing errors should be InvalidPosition: {:?}",
            error
        );
    }

    for error in &move_errors {
        assert!(
            matches!(error, ChessError::InvalidMove(_))
                || matches!(error, ChessError::InvalidPosition(_))
                || matches!(error, ChessError::InvalidPieceType(_)),
            "All move parsing errors should be valid ChessError variants: {:?}",
            error
        );
    }

    // Test that all errors implement required traits
    for error in color_errors
        .iter()
        .chain(piece_type_errors.iter())
        .chain(position_errors.iter())
        .chain(move_errors.iter())
    {
        // Test Clone
        let _cloned = error.clone();

        // Test PartialEq
        assert_eq!(error, error);

        // Test Debug
        let debug_str = format!("{:?}", error);
        assert!(!debug_str.is_empty());

        // Test Display
        let display_str = format!("{}", error);
        assert!(!display_str.is_empty());

        // Test std::error::Error
        let _: &dyn std::error::Error = error;
    }

    println!("✅ Comprehensive error consistency test passed");
    println!("   - {} color errors tested", color_errors.len());
    println!("   - {} piece type errors tested", piece_type_errors.len());
    println!("   - {} position errors tested", position_errors.len());
    println!("   - {} move errors tested", move_errors.len());
}
