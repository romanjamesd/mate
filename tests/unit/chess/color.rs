use mate::chess::{ChessError, Color};
use mate::storage::models::PlayerColor;

/// Test parsing valid color inputs
#[test]
fn test_color_parse_valid_inputs() {
    // Test full names (case insensitive)
    assert_eq!("white".parse::<Color>().unwrap(), Color::White);
    assert_eq!("WHITE".parse::<Color>().unwrap(), Color::White);
    assert_eq!("White".parse::<Color>().unwrap(), Color::White);
    assert_eq!("WhItE".parse::<Color>().unwrap(), Color::White);

    assert_eq!("black".parse::<Color>().unwrap(), Color::Black);
    assert_eq!("BLACK".parse::<Color>().unwrap(), Color::Black);
    assert_eq!("Black".parse::<Color>().unwrap(), Color::Black);
    assert_eq!("BlAcK".parse::<Color>().unwrap(), Color::Black);

    // Test abbreviations (case insensitive)
    assert_eq!("w".parse::<Color>().unwrap(), Color::White);
    assert_eq!("W".parse::<Color>().unwrap(), Color::White);

    assert_eq!("b".parse::<Color>().unwrap(), Color::Black);
    assert_eq!("B".parse::<Color>().unwrap(), Color::Black);
}

/// Test parsing invalid color inputs returns ChessError::InvalidColor
#[test]
fn test_color_parse_invalid_inputs() {
    let invalid_inputs = [
        "",
        "red",
        "blue",
        "green",
        "wh",
        "bl",
        "whit",
        "blac",
        "x",
        "1",
        "white ",
        " white",
        "white\n",
        "black\t",
        "w h i t e",
        "b-l-a-c-k",
    ];

    for input in &invalid_inputs {
        let result = input.parse::<Color>();
        assert!(result.is_err(), "Input '{}' should fail to parse", input);

        match result.unwrap_err() {
            ChessError::InvalidColor(msg) => {
                assert!(
                    msg.contains(input),
                    "Error message should contain the invalid input"
                );
                assert!(
                    msg.contains("Expected 'white' or 'black'"),
                    "Error message should be descriptive"
                );
            }
            other => panic!("Expected InvalidColor error, got: {:?}", other),
        }
    }
}

/// Test Display trait output
#[test]
fn test_color_display_output() {
    assert_eq!(Color::White.to_string(), "White");
    assert_eq!(Color::Black.to_string(), "Black");

    // Test format! macro usage
    assert_eq!(format!("{}", Color::White), "White");
    assert_eq!(format!("{}", Color::Black), "Black");
}

/// Test bidirectional conversion with storage::PlayerColor
#[test]
fn test_color_player_color_conversion() {
    // Test From<PlayerColor> for Color
    assert_eq!(Color::from(PlayerColor::White), Color::White);
    assert_eq!(Color::from(PlayerColor::Black), Color::Black);

    // Test From<Color> for PlayerColor
    assert_eq!(PlayerColor::from(Color::White), PlayerColor::White);
    assert_eq!(PlayerColor::from(Color::Black), PlayerColor::Black);

    // Test roundtrip conversions
    let colors = [Color::White, Color::Black];
    for &color in &colors {
        let player_color = PlayerColor::from(color);
        let back_to_color = Color::from(player_color);
        assert_eq!(
            color, back_to_color,
            "Roundtrip conversion should preserve value"
        );
    }

    let player_colors = [PlayerColor::White, PlayerColor::Black];
    for player_color in &player_colors {
        let color = Color::from(player_color.clone());
        let back_to_player_color = PlayerColor::from(color);
        assert_eq!(
            *player_color, back_to_player_color,
            "Roundtrip conversion should preserve value"
        );
    }
}

/// Test opposite method functionality
#[test]
fn test_color_opposite_method() {
    assert_eq!(Color::White.opposite(), Color::Black);
    assert_eq!(Color::Black.opposite(), Color::White);

    // Test double opposite returns original
    assert_eq!(Color::White.opposite().opposite(), Color::White);
    assert_eq!(Color::Black.opposite().opposite(), Color::Black);
}

/// Test Color enum implements required traits
#[test]
fn test_color_trait_implementations() {
    let white = Color::White;
    let black = Color::Black;

    // Test Debug
    assert!(format!("{:?}", white).contains("White"));
    assert!(format!("{:?}", black).contains("Black"));

    // Test Clone
    let white_clone = white.clone();
    assert_eq!(white, white_clone);

    // Test Copy (implicit test - should compile)
    let white_copy = white;
    assert_eq!(white, white_copy);

    // Test PartialEq
    assert_eq!(white, Color::White);
    assert_ne!(white, Color::Black);

    // Test Eq (can't test directly, but PartialEq + Eq allows HashMap usage)
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert(white, "white piece");
    map.insert(black, "black piece");
    assert_eq!(map.get(&Color::White), Some(&"white piece"));

    // Test Hash (implicit through HashMap usage above)

    // Test serialization/deserialization
    let serialized = serde_json::to_string(&white).expect("Serialization should work");
    let deserialized: Color =
        serde_json::from_str(&serialized).expect("Deserialization should work");
    assert_eq!(white, deserialized);
}

/// Test parsing roundtrip consistency
#[test]
fn test_color_parsing_roundtrip() {
    let test_cases = [(Color::White, "white"), (Color::Black, "black")];

    for (color, _canonical_str) in &test_cases {
        // Test that display -> parse preserves the color
        let displayed = color.to_string();
        let parsed = displayed
            .parse::<Color>()
            .expect("Display output should parse back");
        assert_eq!(
            *color, parsed,
            "Display -> parse roundtrip should preserve value"
        );
    }

    // Test various valid inputs all parse correctly
    let valid_white_inputs = ["white", "WHITE", "White", "w", "W"];
    for input in &valid_white_inputs {
        assert_eq!(input.parse::<Color>().unwrap(), Color::White);
    }

    let valid_black_inputs = ["black", "BLACK", "Black", "b", "B"];
    for input in &valid_black_inputs {
        assert_eq!(input.parse::<Color>().unwrap(), Color::Black);
    }
}

/// Test error message quality and consistency
#[test]
fn test_color_error_messages() {
    let invalid_input = "purple";
    let result = invalid_input.parse::<Color>();

    assert!(result.is_err());
    let error = result.unwrap_err();

    // Test error display
    let error_string = error.to_string();
    assert!(error_string.to_lowercase().contains("invalid color"));
    assert!(error_string.contains("purple"));

    // Test error is descriptive
    match error {
        ChessError::InvalidColor(msg) => {
            assert!(msg.contains("Expected 'white' or 'black'"));
            assert!(msg.contains("purple"));
        }
        _ => panic!("Expected InvalidColor variant"),
    }
}
