use mate::chess::{ChessError, Position};

/// Test valid Position creation with coordinates 0-7
#[test]
fn test_position_valid_creation() {
    // Test all valid coordinates (0-7)
    for file in 0..8 {
        for rank in 0..8 {
            let pos = Position::new(file, rank).expect("Valid coordinates should create position");
            assert_eq!(pos.file, file);
            assert_eq!(pos.rank, rank);
        }
    }

    // Test specific valid positions
    let pos_a1 = Position::new(0, 0).unwrap();
    assert_eq!(pos_a1.file, 0);
    assert_eq!(pos_a1.rank, 0);

    let pos_h8 = Position::new(7, 7).unwrap();
    assert_eq!(pos_h8.file, 7);
    assert_eq!(pos_h8.rank, 7);

    let pos_e4 = Position::new(4, 3).unwrap();
    assert_eq!(pos_e4.file, 4);
    assert_eq!(pos_e4.rank, 3);
}

/// Test invalid Position creation with coordinates > 7
#[test]
fn test_position_invalid_creation() {
    // Test invalid file coordinates
    let invalid_files = [8, 9, 10, 255];
    for file in &invalid_files {
        let result = Position::new(*file, 0);
        assert!(result.is_err(), "File {} should be invalid", file);

        match result.unwrap_err() {
            ChessError::InvalidPosition(msg) => {
                assert!(msg.contains("File must be 0-7"));
                assert!(msg.contains(&file.to_string()));
            }
            other => panic!("Expected InvalidPosition error, got: {:?}", other),
        }
    }

    // Test invalid rank coordinates
    let invalid_ranks = [8, 9, 10, 255];
    for rank in &invalid_ranks {
        let result = Position::new(0, *rank);
        assert!(result.is_err(), "Rank {} should be invalid", rank);

        match result.unwrap_err() {
            ChessError::InvalidPosition(msg) => {
                assert!(msg.contains("Rank must be 0-7"));
                assert!(msg.contains(&rank.to_string()));
            }
            other => panic!("Expected InvalidPosition error, got: {:?}", other),
        }
    }

    // Test both file and rank invalid (should fail on file first)
    let result = Position::new(8, 8);
    assert!(result.is_err());
    match result.unwrap_err() {
        ChessError::InvalidPosition(msg) => {
            assert!(msg.contains("File must be 0-7"));
        }
        other => panic!("Expected InvalidPosition error, got: {:?}", other),
    }
}

/// Test algebraic notation parsing from "a1" to "h8"
#[test]
fn test_position_algebraic_notation_parsing() {
    // Test all valid algebraic positions
    let files = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
    let ranks = ['1', '2', '3', '4', '5', '6', '7', '8'];

    for (file_idx, file_char) in files.iter().enumerate() {
        for (rank_idx, rank_char) in ranks.iter().enumerate() {
            let algebraic = format!("{file_char}{rank_char}");
            let pos = algebraic
                .parse::<Position>()
                .unwrap_or_else(|_| panic!("Should parse {}", algebraic));

            assert_eq!(pos.file, file_idx as u8);
            assert_eq!(pos.rank, rank_idx as u8);
        }
    }

    // Test specific positions
    assert_eq!(
        "a1".parse::<Position>().unwrap(),
        Position::new(0, 0).unwrap()
    );
    assert_eq!(
        "h8".parse::<Position>().unwrap(),
        Position::new(7, 7).unwrap()
    );
    assert_eq!(
        "e4".parse::<Position>().unwrap(),
        Position::new(4, 3).unwrap()
    );
    assert_eq!(
        "d5".parse::<Position>().unwrap(),
        Position::new(3, 4).unwrap()
    );
}

/// Test algebraic notation display
#[test]
fn test_position_algebraic_notation_display() {
    // Test specific positions
    assert_eq!(Position::new(0, 0).unwrap().to_string(), "a1");
    assert_eq!(Position::new(7, 7).unwrap().to_string(), "h8");
    assert_eq!(Position::new(4, 3).unwrap().to_string(), "e4");
    assert_eq!(Position::new(3, 4).unwrap().to_string(), "d5");

    // Test all positions display correctly
    let files = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h'];
    let ranks = ['1', '2', '3', '4', '5', '6', '7', '8'];

    for (file_idx, file_char) in files.iter().enumerate() {
        for (rank_idx, rank_char) in ranks.iter().enumerate() {
            let pos = Position::new(file_idx as u8, rank_idx as u8).unwrap();
            let expected = format!("{file_char}{rank_char}");
            assert_eq!(pos.to_string(), expected);
        }
    }
}

/// Test FromStr/Display roundtrip consistency
#[test]
fn test_position_fromstr_display_roundtrip() {
    // Test that parse -> display -> parse preserves the position
    let test_positions = [
        "a1", "a8", "h1", "h8", "e4", "d5", "c3", "f6", "b2", "g7", "a4", "h5", "e1", "d8", "c6",
        "f3",
    ];

    for pos_str in &test_positions {
        let parsed = pos_str
            .parse::<Position>()
            .unwrap_or_else(|_| panic!("Should parse {}", pos_str));
        let displayed = parsed.to_string();
        assert_eq!(*pos_str, displayed, "Parse -> display should be consistent");

        let re_parsed = displayed
            .parse::<Position>()
            .expect("Display output should re-parse");
        assert_eq!(
            parsed, re_parsed,
            "Parse -> display -> parse should preserve position"
        );
    }

    // Test all valid positions
    for file in 0..8 {
        for rank in 0..8 {
            let pos = Position::new(file, rank).unwrap();
            let displayed = pos.to_string();
            let re_parsed = displayed.parse::<Position>().unwrap();
            assert_eq!(pos, re_parsed, "Roundtrip should preserve position");
        }
    }
}

/// Test invalid algebraic notation parsing
#[test]
fn test_position_invalid_algebraic_parsing() {
    let invalid_inputs = [
        "",     // Empty string
        "a",    // Too short
        "a12",  // Too long
        "i1",   // Invalid file
        "a9",   // Invalid rank
        "a0",   // Invalid rank (0)
        "z8",   // Invalid file
        "A1",   // Should work (case insensitive for files), but let's test lowercase
        " a1",  // Leading space
        "a1 ",  // Trailing space
        "a 1",  // Space in middle
        "11",   // No file letter
        "aa",   // No rank number
        "1a",   // Reversed order
        "e4e4", // Move notation
    ];

    for input in &invalid_inputs {
        let result = input.parse::<Position>();
        if input == &"A1" {
            // This should actually work due to case insensitive parsing
            continue;
        }
        assert!(result.is_err(), "Input '{}' should fail to parse", input);

        match result.unwrap_err() {
            ChessError::InvalidPosition(msg) => {
                // Just ensure we get a descriptive error message
                assert!(!msg.is_empty(), "Error message should not be empty");
            }
            other => panic!("Expected InvalidPosition error, got: {:?}", other),
        }
    }
}

/// Test helper method: same_rank
#[test]
fn test_position_same_rank() {
    let a1 = Position::new(0, 0).unwrap();
    let h1 = Position::new(7, 0).unwrap();
    let a8 = Position::new(0, 7).unwrap();
    let e4 = Position::new(4, 3).unwrap();
    let d4 = Position::new(3, 3).unwrap();

    // Same rank tests
    assert!(a1.same_rank(&h1), "a1 and h1 should be on same rank");
    assert!(e4.same_rank(&d4), "e4 and d4 should be on same rank");
    assert!(
        a1.same_rank(&a1),
        "Position should be on same rank as itself"
    );

    // Different rank tests
    assert!(!a1.same_rank(&a8), "a1 and a8 should be on different ranks");
    assert!(!e4.same_rank(&a1), "e4 and a1 should be on different ranks");
}

/// Test helper method: same_file
#[test]
fn test_position_same_file() {
    let a1 = Position::new(0, 0).unwrap();
    let a8 = Position::new(0, 7).unwrap();
    let h1 = Position::new(7, 0).unwrap();
    let e4 = Position::new(4, 3).unwrap();
    let e7 = Position::new(4, 6).unwrap();

    // Same file tests
    assert!(a1.same_file(&a8), "a1 and a8 should be on same file");
    assert!(e4.same_file(&e7), "e4 and e7 should be on same file");
    assert!(
        a1.same_file(&a1),
        "Position should be on same file as itself"
    );

    // Different file tests
    assert!(!a1.same_file(&h1), "a1 and h1 should be on different files");
    assert!(!e4.same_file(&a1), "e4 and a1 should be on different files");
}

/// Test helper method: same_diagonal
#[test]
fn test_position_same_diagonal() {
    let a1 = Position::new(0, 0).unwrap();
    let b2 = Position::new(1, 1).unwrap();
    let c3 = Position::new(2, 2).unwrap();
    let h8 = Position::new(7, 7).unwrap();

    let a8 = Position::new(0, 7).unwrap();
    let b7 = Position::new(1, 6).unwrap();
    let g2 = Position::new(6, 1).unwrap();
    let h1 = Position::new(7, 0).unwrap();

    let e4 = Position::new(4, 3).unwrap();
    let e5 = Position::new(4, 4).unwrap(); // Not diagonal

    // Same diagonal tests (main diagonal)
    assert!(
        a1.same_diagonal(&b2),
        "a1 and b2 should be on same diagonal"
    );
    assert!(
        a1.same_diagonal(&c3),
        "a1 and c3 should be on same diagonal"
    );
    assert!(
        a1.same_diagonal(&h8),
        "a1 and h8 should be on same diagonal"
    );
    assert!(
        b2.same_diagonal(&h8),
        "b2 and h8 should be on same diagonal"
    );

    // Same diagonal tests (anti-diagonal)
    assert!(
        a8.same_diagonal(&b7),
        "a8 and b7 should be on same diagonal"
    );
    assert!(
        a8.same_diagonal(&h1),
        "a8 and h1 should be on same diagonal"
    );
    assert!(
        g2.same_diagonal(&h1),
        "g2 and h1 should be on same diagonal"
    );

    // Same position
    assert!(
        a1.same_diagonal(&a1),
        "Position should be on same diagonal as itself"
    );

    // Different diagonal tests
    assert!(
        !a1.same_diagonal(&a8),
        "a1 and a8 should not be on same diagonal"
    );
    assert!(
        !e4.same_diagonal(&e5),
        "e4 and e5 should not be on same diagonal"
    );
    // Note: a1 (0,0) and e5 (4,4) ARE on the same diagonal since |0-4| == |0-4|
    // Let's use a1 and e4 instead: a1(0,0) and e4(4,3) -> |0-4| != |0-3|
    let e4_pos = Position::new(4, 3).unwrap();
    assert!(
        !a1.same_diagonal(&e4_pos),
        "a1 and e4 should not be on same diagonal"
    );
}

/// Test helper method: distance
#[test]
fn test_position_distance() {
    let a1 = Position::new(0, 0).unwrap();
    let a2 = Position::new(0, 1).unwrap();
    let b1 = Position::new(1, 0).unwrap();
    let b2 = Position::new(1, 1).unwrap();
    let h8 = Position::new(7, 7).unwrap();
    let e4 = Position::new(4, 3).unwrap();

    // Distance to self
    assert_eq!(a1.distance(&a1), 0, "Distance to self should be 0");

    // Adjacent positions
    assert_eq!(a1.distance(&a2), 1, "Distance a1->a2 should be 1");
    assert_eq!(a1.distance(&b1), 1, "Distance a1->b1 should be 1");
    assert_eq!(a1.distance(&b2), 2, "Distance a1->b2 should be 2 (1+1)");

    // Longer distances
    assert_eq!(a1.distance(&h8), 14, "Distance a1->h8 should be 14 (7+7)");
    assert_eq!(a1.distance(&e4), 7, "Distance a1->e4 should be 7 (4+3)");

    // Symmetry test
    assert_eq!(
        a1.distance(&h8),
        h8.distance(&a1),
        "Distance should be symmetric"
    );
    assert_eq!(
        e4.distance(&a1),
        a1.distance(&e4),
        "Distance should be symmetric"
    );
}

/// Test Position bounds validation consistency
#[test]
fn test_position_bounds_consistency() {
    // All valid positions should be within 0-7 range
    for file in 0..8 {
        for rank in 0..8 {
            let pos = Position::new(file, rank).unwrap();
            assert!(pos.file <= 7, "File should be within bounds");
            assert!(pos.rank <= 7, "Rank should be within bounds");
            assert!(pos.file < 8, "File should be less than 8");
            assert!(pos.rank < 8, "Rank should be less than 8");
        }
    }

    // Test boundary values specifically
    let corner_positions = [
        Position::new(0, 0).unwrap(), // a1
        Position::new(0, 7).unwrap(), // a8
        Position::new(7, 0).unwrap(), // h1
        Position::new(7, 7).unwrap(), // h8
    ];

    for pos in &corner_positions {
        assert!(
            pos.file <= 7 && pos.rank <= 7,
            "Corner positions should be valid"
        );
    }
}

/// Test property-based parsing roundtrips
#[test]
fn test_position_parsing_roundtrip_property() {
    // Property: pos.to_string().parse() == pos for all valid positions
    for file in 0..8 {
        for rank in 0..8 {
            let pos = Position::new(file, rank).unwrap();
            let pos_string = pos.to_string();
            let parsed_pos = pos_string
                .parse::<Position>()
                .unwrap_or_else(|_| panic!("Should parse {}", pos_string));

            assert_eq!(
                pos, parsed_pos,
                "pos.to_string().parse() should equal original pos for {:?}",
                pos
            );
        }
    }
}

/// Test Position trait implementations
#[test]
fn test_position_trait_implementations() {
    let a1 = Position::new(0, 0).unwrap();
    let e4 = Position::new(4, 3).unwrap();

    // Test Debug
    let debug_str = format!("{a1:?}");
    assert!(debug_str.contains("Position"));
    assert!(debug_str.contains("file"));
    assert!(debug_str.contains("rank"));

    // Test Clone
    let a1_clone = a1;
    assert_eq!(a1, a1_clone);

    // Test Copy (implicit test - should compile)
    let a1_copy = a1;
    assert_eq!(a1, a1_copy);

    // Test PartialEq and Eq
    assert_eq!(a1, Position::new(0, 0).unwrap());
    assert_ne!(a1, e4);

    // Test Hash (through HashMap usage)
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert(a1, "a1 square");
    map.insert(e4, "e4 square");
    assert_eq!(map.get(&Position::new(0, 0).unwrap()), Some(&"a1 square"));

    // Test serialization/deserialization
    let serialized = serde_json::to_string(&a1).expect("Serialization should work");
    let deserialized: Position =
        serde_json::from_str(&serialized).expect("Deserialization should work");
    assert_eq!(a1, deserialized);
}

/// Test error handling consistency
#[test]
fn test_position_error_consistency() {
    // Test that all parsing errors use ChessError::InvalidPosition
    let invalid_positions = ["", "a", "a12", "i1", "a9", "z8"];

    for input in &invalid_positions {
        match input.parse::<Position>() {
            Ok(_) => panic!("Input '{}' should have failed", input),
            Err(ChessError::InvalidPosition(_)) => {
                // This is what we expect
            }
            Err(other) => panic!(
                "Expected InvalidPosition error for '{}', got: {:?}",
                input, other
            ),
        }
    }

    // Test that creation errors use ChessError::InvalidPosition
    let invalid_coords = [(8, 0), (0, 8), (8, 8), (255, 0), (0, 255)];

    for (file, rank) in &invalid_coords {
        match Position::new(*file, *rank) {
            Ok(_) => panic!("Coordinates ({}, {}) should have failed", file, rank),
            Err(ChessError::InvalidPosition(_)) => {
                // This is what we expect
            }
            Err(other) => panic!(
                "Expected InvalidPosition error for ({}, {}), got: {:?}",
                file, rank, other
            ),
        }
    }
}

/// Test char conversion methods
#[test]
fn test_position_char_conversion() {
    let test_cases = [
        (0, 0, 'a', '1'), // a1
        (4, 3, 'e', '4'), // e4
        (7, 7, 'h', '8'), // h8
        (3, 4, 'd', '5'), // d5
    ];

    for (file, rank, expected_file_char, expected_rank_char) in &test_cases {
        let pos = Position::new(*file, *rank).unwrap();
        assert_eq!(pos.file_char(), *expected_file_char);
        assert_eq!(pos.rank_char(), *expected_rank_char);
    }

    // Test all positions
    for file in 0..8 {
        for rank in 0..8 {
            let pos = Position::new(file, rank).unwrap();
            let expected_file_char = (file + b'a') as char;
            let expected_rank_char = (rank + b'1') as char;

            assert_eq!(pos.file_char(), expected_file_char);
            assert_eq!(pos.rank_char(), expected_rank_char);
        }
    }
}

/// Test from_chars constructor
#[test]
fn test_position_from_chars() {
    // Test valid chars
    let valid_cases = [
        ('a', '1', 0, 0),
        ('h', '8', 7, 7),
        ('e', '4', 4, 3),
        ('A', '1', 0, 0), // Test case insensitive files
        ('H', '8', 7, 7),
    ];

    for (file_char, rank_char, expected_file, expected_rank) in &valid_cases {
        let pos = Position::from_chars(*file_char, *rank_char).unwrap();
        assert_eq!(pos.file, *expected_file);
        assert_eq!(pos.rank, *expected_rank);
    }

    // Test invalid file chars
    let invalid_file_chars = ['i', 'z', '1', '@', ' '];
    for file_char in &invalid_file_chars {
        let result = Position::from_chars(*file_char, '1');
        assert!(
            result.is_err(),
            "File char '{}' should be invalid",
            file_char
        );
        match result.unwrap_err() {
            ChessError::InvalidPosition(msg) => {
                assert!(msg.contains("Invalid file"));
                assert!(msg.contains("Must be a-h"));
            }
            other => panic!("Expected InvalidPosition error, got: {:?}", other),
        }
    }

    // Test invalid rank chars
    let invalid_rank_chars = ['0', '9', 'a', '@', ' '];
    for rank_char in &invalid_rank_chars {
        let result = Position::from_chars('a', *rank_char);
        assert!(
            result.is_err(),
            "Rank char '{}' should be invalid",
            rank_char
        );
        match result.unwrap_err() {
            ChessError::InvalidPosition(msg) => {
                assert!(msg.contains("Invalid rank"));
                assert!(msg.contains("Must be 1-8"));
            }
            other => panic!("Expected InvalidPosition error, got: {:?}", other),
        }
    }
}
