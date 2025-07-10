//! Input Validation Tests
//!
//! Tests for `src/cli/validation.rs` input validation functions
//! Following Phase 1.3 of the testing implementation plan

use mate::chess::Color;
use mate::cli::validation::{InputValidator, ValidationError};
use mate::storage::Database;
use std::sync::Arc;
use tempfile::TempDir;

/// Create a test database for validation testing
/// Returns both the database and the temp directory to keep it alive
fn create_test_database() -> (Database, Arc<TempDir>) {
    let temp_dir = Arc::new(TempDir::new().expect("Failed to create temp dir"));
    let db_path = temp_dir.path().join("test_validation.db");
    let database =
        Database::new_with_path("test_peer", &db_path).expect("Failed to create test database");
    (database, temp_dir)
}

// =============================================================================
// Game ID Validation Tests
// =============================================================================

#[test]
fn test_uuid_format_validation() {
    // Test UUID format validation specifically
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
    let result = validator.validate_uuid_format(valid_uuid);
    assert!(result.is_ok(), "Valid UUID format should pass validation");

    let invalid_formats = vec![
        "not-a-uuid",
        "550e8400-e29b-41d4-a716",                    // Too short
        "550e8400-e29b-41d4-a716-446655440000-extra", // Too long
        "ggge8400-e29b-41d4-a716-446655440000",       // Invalid characters
    ];

    for invalid_uuid in invalid_formats {
        let result = validator.validate_uuid_format(invalid_uuid);
        assert!(
            result.is_err(),
            "Invalid UUID format '{}' should fail validation",
            invalid_uuid
        );
    }
}

#[test]
fn test_empty_game_id_handling() {
    // Test that empty game ID input is handled appropriately
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let test_cases = vec!["", "   ", "\t", "\n"];

    for empty_input in test_cases {
        let result = validator.validate_and_resolve_game_id(empty_input);
        assert!(
            result.is_err(),
            "Empty game ID '{}' should return error",
            empty_input.escape_debug()
        );

        match result.unwrap_err() {
            ValidationError::InvalidGameId(_) => {
                // Expected error type
            }
            other => panic!(
                "Expected InvalidGameId error for empty input, got: {:?}",
                other
            ),
        }
    }
}

#[test]
fn test_game_id_validation_no_active_games() {
    // Test behavior when no active games exist
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let result = validator.validate_and_resolve_game_id("some-uuid");
    assert!(result.is_err(), "Should error when no active games exist");

    match result.unwrap_err() {
        ValidationError::NoActiveGames => {
            // Expected error type
        }
        other => panic!("Expected NoActiveGames error, got: {:?}", other),
    }
}

// =============================================================================
// Address Validation Tests
// =============================================================================

#[test]
fn test_ipv4_address_validation() {
    // Test IPv4 address validation with various valid formats
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let valid_ipv4_addresses = vec![
        "127.0.0.1:8080",
        "192.168.1.1:3000",
        "10.0.0.1:80",
        "255.255.255.255:65535",
        "0.0.0.0:1",
    ];

    for address in valid_ipv4_addresses {
        let result = validator.validate_peer_address(address);
        assert!(
            result.is_ok(),
            "Valid IPv4 address '{}' should pass validation",
            address
        );

        let socket_addr = result.unwrap();
        assert!(socket_addr.is_ipv4(), "Should resolve to IPv4 address");
    }
}

#[test]
fn test_ipv6_address_validation() {
    // Test IPv6 address validation with various valid formats
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let valid_ipv6_addresses = vec![
        "[::1]:8080",
        "[2001:db8::1]:3000",
        // Note: Zone identifiers like %lo0 are not supported by standard socket address parsing
    ];

    for address in valid_ipv6_addresses {
        let result = validator.validate_peer_address(address);
        assert!(
            result.is_ok(),
            "Valid IPv6 address '{}' should pass validation",
            address
        );

        let socket_addr = result.unwrap();
        assert!(socket_addr.is_ipv6(), "Should resolve to IPv6 address");
    }
}

#[test]
fn test_hostname_address_validation() {
    // Test hostname address validation
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let valid_hostnames = vec![
        "localhost:8080",
        "example.com:80",
        "subdomain.example.com:443",
    ];

    for address in valid_hostnames {
        let result = validator.validate_peer_address(address);
        // Note: This may fail in test environments without network access
        // but we test the validation logic
        match result {
            Ok(socket_addr) => {
                assert!(
                    socket_addr.port() > 0,
                    "Should have valid port number for '{}'",
                    address
                );
            }
            Err(ValidationError::InvalidPeerAddress(_)) => {
                // May fail due to DNS resolution in test environment - that's okay
            }
            Err(other) => {
                panic!(
                    "Unexpected error type for hostname '{}': {:?}",
                    address, other
                );
            }
        }
    }
}

#[test]
fn test_port_requirement_enforcement() {
    // Test that port numbers are required and validated
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let addresses_missing_ports = vec!["127.0.0.1", "localhost", "example.com", "[::1]"];

    for address in addresses_missing_ports {
        let result = validator.validate_peer_address(address);
        assert!(
            result.is_err(),
            "Address without port '{}' should fail validation",
            address
        );

        match result.unwrap_err() {
            ValidationError::InvalidPeerAddress(msg) => {
                assert!(
                    msg.contains("port"),
                    "Error message should mention port requirement for '{}'",
                    address
                );
            }
            other => panic!(
                "Expected InvalidPeerAddress error for '{}', got: {:?}",
                address, other
            ),
        }
    }

    let addresses_with_empty_ports = vec!["127.0.0.1:", "localhost:", "[::1]:"];

    for address in addresses_with_empty_ports {
        let result = validator.validate_peer_address(address);
        assert!(
            result.is_err(),
            "Address with empty port '{}' should fail validation",
            address
        );
    }
}

#[test]
fn test_address_validation_edge_cases() {
    // Test edge cases and invalid address formats
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let invalid_addresses = vec![
        "",                     // Empty
        "   ",                  // Whitespace only
        "256.256.256.256:8080", // Invalid IPv4
        "localhost:70000",      // Port too high (port 0 is actually valid)
        "127.0.0.1:abc",        // Non-numeric port
        "not-an-address:8080",  // Invalid hostname format
    ];

    for address in invalid_addresses {
        let result = validator.validate_peer_address(address);
        assert!(
            result.is_err(),
            "Invalid address '{}' should fail validation",
            address
        );

        match result.unwrap_err() {
            ValidationError::InvalidPeerAddress(_) => {
                // Expected error type
            }
            other => panic!(
                "Expected InvalidPeerAddress error for '{}', got: {:?}",
                address, other
            ),
        }
    }
}

// =============================================================================
// Color Validation Tests
// =============================================================================

#[test]
fn test_color_input_normalization() {
    // Test that color inputs are properly normalized
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let white_variations = vec!["white", "WHITE", "White", "WhItE", "w", "W"];

    for input in white_variations {
        let result = validator.validate_color(input);
        assert!(
            result.is_ok(),
            "Valid white color input '{}' should pass validation",
            input
        );

        let color = result.unwrap();
        assert_eq!(
            color,
            Some(Color::White),
            "Input '{}' should normalize to White",
            input
        );
    }

    let black_variations = vec!["black", "BLACK", "Black", "BlAcK", "b", "B"];

    for input in black_variations {
        let result = validator.validate_color(input);
        assert!(
            result.is_ok(),
            "Valid black color input '{}' should pass validation",
            input
        );

        let color = result.unwrap();
        assert_eq!(
            color,
            Some(Color::Black),
            "Input '{}' should normalize to Black",
            input
        );
    }
}

#[test]
fn test_color_case_insensitive_handling() {
    // Test case insensitive color handling specifically
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let test_cases = vec![
        ("WHITE", Some(Color::White)),
        ("white", Some(Color::White)),
        ("White", Some(Color::White)),
        ("wHiTe", Some(Color::White)),
        ("BLACK", Some(Color::Black)),
        ("black", Some(Color::Black)),
        ("Black", Some(Color::Black)),
        ("bLaCk", Some(Color::Black)),
        ("RANDOM", None),
        ("random", None),
        ("Random", None),
        ("rAnDoM", None),
        ("RAND", None),
        ("rand", None),
        ("R", None),
        ("r", None),
    ];

    for (input, expected) in test_cases {
        let result = validator.validate_color(input);
        assert!(
            result.is_ok(),
            "Color input '{}' should pass validation",
            input
        );
        assert_eq!(
            result.unwrap(),
            expected,
            "Input '{}' should normalize to {:?}",
            input,
            expected
        );
    }
}

#[test]
fn test_invalid_color_input_rejection() {
    // Test that invalid color inputs are rejected appropriately
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let invalid_colors = vec![
        "red",
        "blue",
        "green",
        "yellow",
        "purple",
        "orange",
        "pink",
        "gray",
        "light",
        "dark",
        "color",
        "123",
        "!",
        "@#$%",
        "whit",
        "blac",
        "wite",
        "balck", // Common typos
        "w h i t e",
        "b-l-a-c-k", // Spaced/hyphenated
    ];

    for invalid_color in invalid_colors {
        let result = validator.validate_color(invalid_color);
        assert!(
            result.is_err(),
            "Invalid color '{}' should fail validation",
            invalid_color
        );

        match result.unwrap_err() {
            ValidationError::InvalidColor(msg) => {
                assert!(
                    msg.contains(invalid_color),
                    "Error message should contain the invalid input '{}'",
                    invalid_color
                );
                assert!(
                    msg.contains("white") || msg.contains("black") || msg.contains("random"),
                    "Error message should suggest valid options"
                );
            }
            other => panic!(
                "Expected InvalidColor error for '{}', got: {:?}",
                invalid_color, other
            ),
        }
    }
}

#[test]
fn test_color_empty_input_handling() {
    // Test that empty color input defaults to random (None)
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    let empty_inputs = vec!["", "   ", "\t", "\n"];

    for empty_input in empty_inputs {
        let result = validator.validate_color(empty_input);
        assert!(
            result.is_ok(),
            "Empty color input '{}' should default to random",
            empty_input.escape_debug()
        );
        assert_eq!(
            result.unwrap(),
            None,
            "Empty input should result in None (random color)"
        );
    }
}

// =============================================================================
// Integration Tests for Multiple Validation Types
// =============================================================================

#[test]
fn test_validation_error_types_are_appropriate() {
    // Test that validation functions return appropriate error types
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    // Test game ID validation error types
    let game_id_result = validator.validate_and_resolve_game_id("invalid-id");
    assert!(matches!(
        game_id_result.unwrap_err(),
        ValidationError::NoActiveGames
    ));

    // Test address validation error types
    let address_result = validator.validate_peer_address("invalid-address");
    assert!(matches!(
        address_result.unwrap_err(),
        ValidationError::InvalidPeerAddress(_)
    ));

    // Test color validation error types
    let color_result = validator.validate_color("invalid-color");
    assert!(matches!(
        color_result.unwrap_err(),
        ValidationError::InvalidColor(_)
    ));
}

#[test]
fn test_validation_functions_handle_whitespace() {
    // Test that all validation functions properly handle leading/trailing whitespace
    let (database, _temp_dir) = create_test_database();
    let validator = InputValidator::new(&database);

    // Test address validation with whitespace
    let address_result = validator.validate_peer_address("  127.0.0.1:8080  ");
    assert!(
        address_result.is_ok(),
        "Address validation should handle whitespace"
    );

    // Test color validation with whitespace
    let color_result = validator.validate_color("  white  ");
    assert!(
        color_result.is_ok(),
        "Color validation should handle whitespace"
    );
    assert_eq!(color_result.unwrap(), Some(Color::White));
}
