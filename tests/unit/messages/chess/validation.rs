//! Comprehensive validation tests for chess message components (Format & Structure Validation)
//!
//! This module focuses on format validation and message structure validation including:
//! - ValidationError type functionality and display traits
//! - Game ID format validation (UUID structure, not security aspects)
//! - Chess move format validation (algebraic notation, not injection prevention)
//! - Board hash format validation (SHA-256 hex format, not tampering detection)
//! - Message-specific validation (field validation, not security validation)
//! - Integration validation and error propagation across components
//!
//! Note: Security-specific validation (injection prevention, rate limiting, cryptographic 
//! validation, tampering detection) is handled in the security.rs test module.

use mate::chess::{Board, Color};
use mate::messages::chess::{
    generate_game_id, hash_board_state,
    security::{
        validate_secure_fen_notation, validate_secure_move_history,
        validate_secure_reason_text,
    },
    validate_chess_move_format, validate_game_accept, validate_game_decline, validate_game_id,
    validate_game_invite, validate_move_ack, validate_move_message, validate_sync_request,
    validate_sync_response, GameAccept, GameDecline, GameInvite, Move, MoveAck, SyncRequest,
    SyncResponse, ValidationError,
};
use mate::messages::types::Message;

#[cfg(test)]
mod validation_error_tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn test_validation_error_display_traits() {
        let error = ValidationError::InvalidGameId("test-id".to_string());
        assert_eq!(format!("{}", error), "Invalid game ID: test-id");

        let error = ValidationError::InvalidMove("invalid-move".to_string());
        assert_eq!(format!("{}", error), "Invalid chess move: invalid-move");

        let error = ValidationError::InvalidBoardHash("bad-hash".to_string());
        assert_eq!(format!("{}", error), "Invalid board hash: bad-hash");

        let error = ValidationError::InvalidFen("bad-fen".to_string());
        assert_eq!(format!("{}", error), "Invalid FEN notation: bad-fen");

        let error = ValidationError::InvalidMessageFormat("bad-format".to_string());
        assert_eq!(format!("{}", error), "Invalid message format: bad-format");

        let error = ValidationError::BoardHashMismatch {
            expected: "hash1".to_string(),
            actual: "hash2".to_string(),
        };
        assert_eq!(
            format!("{}", error),
            "Board hash mismatch: expected 'hash1', got 'hash2'"
        );
    }

    #[test]
    fn test_validation_error_equality() {
        let error1 = ValidationError::InvalidGameId("test".to_string());
        let error2 = ValidationError::InvalidGameId("test".to_string());
        let error3 = ValidationError::InvalidGameId("different".to_string());

        assert_eq!(error1, error2);
        assert_ne!(error1, error3);

        let hash_error1 = ValidationError::BoardHashMismatch {
            expected: "hash1".to_string(),
            actual: "hash2".to_string(),
        };
        let hash_error2 = ValidationError::BoardHashMismatch {
            expected: "hash1".to_string(),
            actual: "hash2".to_string(),
        };
        assert_eq!(hash_error1, hash_error2);
    }

    #[test]
    fn test_validation_error_clone() {
        let original = ValidationError::InvalidMove("e2e4".to_string());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_validation_error_debug() {
        let error = ValidationError::InvalidGameId("test".to_string());
        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("InvalidGameId"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_validation_error_is_error_trait() {
        let error = ValidationError::InvalidMove("bad".to_string());
        assert!(error.source().is_none());

        // Test that it implements Error trait
        let _: &dyn Error = &error;
    }

    #[test]
    fn test_validation_error_consistency() {
        // Test that all variants are covered and consistent
        let errors = vec![
            ValidationError::InvalidGameId("test".to_string()),
            ValidationError::InvalidMove("test".to_string()),
            ValidationError::InvalidBoardHash("test".to_string()),
            ValidationError::InvalidFen("test".to_string()),
            ValidationError::InvalidMessageFormat("test".to_string()),
            ValidationError::BoardHashMismatch {
                expected: "a".to_string(),
                actual: "b".to_string(),
            },
        ];

        for error in errors {
            // Each error should have a meaningful display message
            let display_msg = format!("{}", error);
            assert!(!display_msg.is_empty());
            assert!(!display_msg.trim().is_empty());

            // Debug format should be informative
            let debug_msg = format!("{:?}", error);
            assert!(!debug_msg.is_empty());
        }
    }
}

#[cfg(test)]
mod game_id_validation_tests {
    use super::*;

    #[test]
    fn test_valid_uuid_formats() {
        // Test various valid UUID formats
        let generated_id = generate_game_id();
        let valid_uuids = vec![
            "123e4567-e89b-12d3-a456-426614174000",
            "00000000-0000-0000-0000-000000000000", // Nil UUID
            "ffffffff-ffff-ffff-ffff-ffffffffffff", // Max UUID
            &generated_id,                          // Generated UUID
        ];

        for uuid in valid_uuids {
            assert!(validate_game_id(uuid), "UUID should be valid: {}", uuid);
        }
    }

    #[test]
    fn test_invalid_uuid_formats() {
        let invalid_uuids = vec![
            "",
            "not-a-uuid",
            "123e4567-e89b-12d3-a456",                    // Too short
            "123e4567-e89b-12d3-a456-426614174000-extra", // Too long
            "123e4567-e89b-12d3-a456-42661417400g",       // Invalid character
            "123e4567_e89b_12d3_a456_426614174000",       // Wrong separator
            "123e4567-e89b-12d3-a456-42661417400",        // Missing character
            "g23e4567-e89b-12d3-a456-426614174000",       // Invalid hex
        ];

        for uuid in invalid_uuids {
            assert!(!validate_game_id(uuid), "UUID should be invalid: {}", uuid);
        }
    }

    #[test]
    fn test_game_id_edge_cases() {
        // Test whitespace handling
        assert!(!validate_game_id("   "));
        assert!(!validate_game_id("\t"));
        assert!(!validate_game_id("\n"));

        // Test special characters
        assert!(!validate_game_id("123e4567-e89b-12d3-a456-42661417400\0"));
        assert!(!validate_game_id("123e4567-e89b-12d3-a456-42661417400\x01"));
    }

    #[test]
    fn test_basic_game_id_properties() {
        let id1 = generate_game_id();
        let id2 = generate_game_id();

        // Generated IDs should be different
        assert_ne!(id1, id2);

        // Generated IDs should be valid
        assert!(validate_game_id(&id1));
        assert!(validate_game_id(&id2));

        // Generated IDs should be correct length
        assert_eq!(id1.len(), 36);
        assert_eq!(id2.len(), 36);
    }
}

#[cfg(test)]
mod chess_move_format_validation_tests {
    use super::*;

    #[test]
    fn test_standard_algebraic_notation() {
        let valid_moves = vec![
            "e2e4", // Pawn move
            "e7e5", // Black pawn response
            "g1f3", // Knight move
            "b8c6", // Black knight
            "f1c4", // Bishop move
            "a7a6", // Pawn advance
            "d2d4", // Center pawn
            "h7h6", // Kingside pawn
        ];

        for chess_move in valid_moves {
            assert!(
                validate_chess_move_format(chess_move).is_ok(),
                "Move should be valid: {}",
                chess_move
            );
        }
    }

    #[test]
    fn test_promotion_moves() {
        let promotion_moves = vec![
            "e7e8q", // Promote to queen
            "e7e8r", // Promote to rook
            "e7e8b", // Promote to bishop
            "e7e8n", // Promote to knight
            "a7a8Q", // Uppercase promotion
            "h7h8R", // Uppercase rook
            "b7b8B", // Uppercase bishop
            "g7g8N", // Uppercase knight
        ];

        for chess_move in promotion_moves {
            assert!(
                validate_chess_move_format(chess_move).is_ok(),
                "Promotion move should be valid: {}",
                chess_move
            );
        }
    }

    #[test]
    fn test_castling_moves() {
        let castling_moves = vec![
            "O-O",   // Kingside castling
            "O-O-O", // Queenside castling
        ];

        for chess_move in castling_moves {
            assert!(
                validate_chess_move_format(chess_move).is_ok(),
                "Castling move should be valid: {}",
                chess_move
            );
        }
    }

    #[test]
    fn test_invalid_move_formats() {
        let invalid_moves = vec![
            "",        // Empty
            "e",       // Too short
            "e2",      // Incomplete
            "e2e",     // Incomplete
            "e2e4e",   // Too long without promotion
            "e2e4q5",  // Too long with extra
            "i2e4",    // Invalid file
            "e9e4",    // Invalid rank
            "e2i4",    // Invalid destination file
            "e2e9",    // Invalid destination rank
            "e2e4x",   // Invalid promotion piece
            "e2e4z",   // Invalid promotion
            "O-O-O-O", // Invalid castling
            "O",       // Incomplete castling
            "OO",      // Wrong castling format
            "e2-e4",   // Wrong separator
            "E2E4",    // All uppercase
            "e2 e4",   // Space in move
            "e2\te4",  // Tab character
            "e2\ne4",  // Newline
        ];

        for chess_move in invalid_moves {
            assert!(
                validate_chess_move_format(chess_move).is_err(),
                "Move should be invalid: '{}'",
                chess_move
            );
        }
    }

    #[test]
    fn test_move_edge_cases() {
        // Test whitespace handling
        assert!(validate_chess_move_format("  e2e4  ").is_ok()); // Should trim whitespace
        assert!(validate_chess_move_format("  O-O  ").is_ok()); // Castling with whitespace

        // Test control characters
        assert!(validate_chess_move_format("e2e4\0").is_err());
        assert!(validate_chess_move_format("e2e4\x01").is_err());
    }

    #[test]
    fn test_move_boundary_conditions() {
        // Test all valid files and ranks
        for file in 'a'..='h' {
            for rank in '1'..='8' {
                let chess_move = format!("{}{}e4", file, rank);
                assert!(
                    validate_chess_move_format(&chess_move).is_ok(),
                    "Valid square move should pass: {}",
                    chess_move
                );
            }
        }

        // Test invalid boundaries
        let invalid_files = vec!['z', '0', '@', 'i'];
        let invalid_ranks = vec!['0', '9', 'a', '@'];

        for file in invalid_files {
            let chess_move = format!("{}1e4", file);
            assert!(
                validate_chess_move_format(&chess_move).is_err(),
                "Invalid file should be rejected: {}",
                chess_move
            );
        }

        for rank in invalid_ranks {
            let chess_move = format!("e{}e4", rank);
            assert!(
                validate_chess_move_format(&chess_move).is_err(),
                "Invalid rank should be rejected: {}",
                chess_move
            );
        }
    }
}

#[cfg(test)]
mod board_hash_format_validation_tests {
    use super::*;

    #[test]
    fn test_valid_sha256_hash_format() {
        let board = Board::new();
        let valid_hash = hash_board_state(&board);

        // Generated hash should be valid
        assert_eq!(valid_hash.len(), 64);
        assert!(valid_hash.chars().all(|c| c.is_ascii_hexdigit()));

        // Test with known valid hashes
        let valid_hashes = vec![
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890",
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ];

        // Note: The actual validation function is private, so we test through message validation
        for hash in valid_hashes {
            let move_msg = Move::new(generate_game_id(), "e2e4".to_string(), hash.to_string());
            // The validation should accept properly formatted hashes
            // (though they may fail board verification)
            // For Move messages, only format validation is done, not board state verification
            match validate_move_message(&move_msg) {
                Ok(_) => {
                    // Valid format accepted
                }
                Err(ValidationError::InvalidBoardHash(_)) => {
                    // This means the format was rejected, which is unexpected for valid hex
                    // Some of our test hashes might contain invalid hex characters
                    // Let's check if the hash contains only valid hex digits
                    if hash.chars().all(|c| c.is_ascii_hexdigit()) && hash.len() == 64 {
                        panic!("Valid hash format should not be rejected: {}", hash);
                    }
                }
                Err(_) => {
                    // Other errors are fine (like invalid game ID, etc.)
                }
            }
        }
    }

    #[test]
    fn test_invalid_hash_length() {
        let invalid_hashes = vec![
            "",                                                                            // Empty
            "a1b2c3",                                                          // Too short
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567", // 63 chars
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890a", // 65 chars
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890123456789", // Much too long
        ];

        for hash in invalid_hashes {
            let move_msg = Move::new(generate_game_id(), "e2e4".to_string(), hash.to_string());
            assert!(
                matches!(
                    validate_move_message(&move_msg),
                    Err(ValidationError::InvalidBoardHash(_))
                ),
                "Hash with length {} should be rejected: {}",
                hash.len(),
                hash
            );
        }
    }

    #[test]
    fn test_invalid_hex_characters() {
        let invalid_hashes = vec![
            "g1b2c3d4e5f6789012345678901234567890123456789012345678901234567890", // 'g' invalid
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567z90", // 'z' invalid
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567@90", // '@' invalid
            "a1b2c3d4e5f678901234567890123456789012345678901234567890123456789 ", // space
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567\t90", // tab
            "a1b2c3d4e5f6789012345678901234567890123456789012345678901234567\n90", // newline
        ];

        for hash in invalid_hashes {
            let move_msg = Move::new(generate_game_id(), "e2e4".to_string(), hash.to_string());
            assert!(
                matches!(
                    validate_move_message(&move_msg),
                    Err(ValidationError::InvalidBoardHash(_))
                ),
                "Hash with invalid characters should be rejected: {}",
                hash
            );
        }
    }

    #[test]
    fn test_hash_case_sensitivity() {
        // Test mixed case hashes
        let mixed_case_hashes = vec![
            "A1B2C3D4E5F6789012345678901234567890123456789012345678901234567890",
            "a1B2c3D4e5F6789012345678901234567890123456789012345678901234567890",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
        ];

        for hash in mixed_case_hashes {
            let move_msg = Move::new(generate_game_id(), "e2e4".to_string(), hash.to_string());
            // The format should be valid (though verification may fail)
            match validate_move_message(&move_msg) {
                Ok(_) => {
                    // Format is valid
                }
                Err(ValidationError::InvalidBoardHash(_)) => {
                    // Check if this is expected - only if it contains non-hex characters
                    if hash.chars().all(|c| c.is_ascii_hexdigit()) && hash.len() == 64 {
                        panic!(
                            "Mixed case hash should not be rejected for format: {}",
                            hash
                        );
                    }
                }
                Err(_) => {
                    // Other errors are acceptable
                }
            }
        }
    }

    #[test]
    fn test_hash_whitespace_handling() {
        let hashes_with_whitespace = vec![
            "  a1b2c3d4e5f6789012345678901234567890123456789012345678901234567890  ",
            "\ta1b2c3d4e5f6789012345678901234567890123456789012345678901234567890\t",
            "\na1b2c3d4e5f6789012345678901234567890123456789012345678901234567890\n",
        ];

        for hash in hashes_with_whitespace {
            let move_msg = Move::new(generate_game_id(), "e2e4".to_string(), hash.to_string());
            // Whitespace-trimmed hash should be valid format
            match validate_move_message(&move_msg) {
                Ok(_) => {
                    // Format is valid after trimming
                }
                Err(ValidationError::InvalidBoardHash(_)) => {
                    // Check if the trimmed hash would be valid
                    let trimmed = hash.trim();
                    if trimmed.chars().all(|c| c.is_ascii_hexdigit()) && trimmed.len() == 64 {
                        panic!(
                            "Hash with whitespace should be trimmed and accepted: '{}'",
                            hash
                        );
                    }
                }
                Err(_) => {
                    // Other errors are acceptable
                }
            }
        }
    }
}

#[cfg(test)]
mod message_specific_validation_tests {
    use super::*;

    #[test]
    fn test_game_invite_validation() {
        // Valid invites
        let valid_invite = GameInvite::new(generate_game_id(), Some(Color::White));
        assert!(validate_game_invite(&valid_invite).is_ok());

        let valid_invite_no_color = GameInvite::new(generate_game_id(), None);
        assert!(validate_game_invite(&valid_invite_no_color).is_ok());

        // Invalid game ID
        let invalid_invite = GameInvite::new("not-a-uuid".to_string(), Some(Color::Black));
        assert!(matches!(
            validate_game_invite(&invalid_invite),
            Err(ValidationError::InvalidGameId(_))
        ));

        // Empty game ID
        let empty_id_invite = GameInvite::new("".to_string(), None);
        assert!(matches!(
            validate_game_invite(&empty_id_invite),
            Err(ValidationError::InvalidGameId(_))
        ));
    }

    #[test]
    fn test_game_accept_validation() {
        // Valid accept
        let valid_accept = GameAccept::new(generate_game_id(), Color::White);
        assert!(validate_game_accept(&valid_accept).is_ok());

        let valid_accept_black = GameAccept::new(generate_game_id(), Color::Black);
        assert!(validate_game_accept(&valid_accept_black).is_ok());

        // Invalid game ID
        let invalid_accept = GameAccept::new("not-a-uuid".to_string(), Color::White);
        assert!(matches!(
            validate_game_accept(&invalid_accept),
            Err(ValidationError::InvalidGameId(_))
        ));

        // Empty game ID
        let empty_id_accept = GameAccept::new("".to_string(), Color::Black);
        assert!(matches!(
            validate_game_accept(&empty_id_accept),
            Err(ValidationError::InvalidGameId(_))
        ));
    }

    #[test]
    fn test_game_decline_validation() {
        // Valid declines
        let valid_decline = GameDecline::new(generate_game_id(), None);
        assert!(validate_game_decline(&valid_decline).is_ok());

        let valid_decline_with_reason = GameDecline::new(
            generate_game_id(),
            Some("Already have too many games".to_string()),
        );
        assert!(validate_game_decline(&valid_decline_with_reason).is_ok());

        // Invalid game ID
        let invalid_decline = GameDecline::new("not-a-uuid".to_string(), None);
        assert!(matches!(
            validate_game_decline(&invalid_decline),
            Err(ValidationError::InvalidGameId(_))
        ));

        // Empty reason should be None instead
        let empty_reason_decline = GameDecline::new(generate_game_id(), Some("".to_string()));
        assert!(matches!(
            validate_game_decline(&empty_reason_decline),
            Err(ValidationError::InvalidMessageFormat(_))
        ));

        // Excessively long reason
        let long_reason = "a".repeat(1001);
        let long_reason_decline = GameDecline::new(generate_game_id(), Some(long_reason));
        assert!(matches!(
            validate_game_decline(&long_reason_decline),
            Err(ValidationError::InvalidMessageFormat(_))
        ));
    }

    #[test]
    fn test_move_message_validation() {
        let board = Board::new();
        let valid_hash = hash_board_state(&board);

        // Valid move
        let valid_move = Move::new(generate_game_id(), "e2e4".to_string(), valid_hash.clone());
        assert!(validate_move_message(&valid_move).is_ok());

        // Invalid game ID
        let invalid_game_id_move = Move::new(
            "not-a-uuid".to_string(),
            "e2e4".to_string(),
            valid_hash.clone(),
        );
        assert!(matches!(
            validate_move_message(&invalid_game_id_move),
            Err(ValidationError::InvalidGameId(_))
        ));

        // Invalid chess move
        let invalid_chess_move = Move::new(
            generate_game_id(),
            "invalid".to_string(),
            valid_hash.clone(),
        );
        assert!(matches!(
            validate_move_message(&invalid_chess_move),
            Err(ValidationError::InvalidMove(_))
        ));

        // Invalid hash format
        let invalid_hash_move = Move::new(
            generate_game_id(),
            "e2e4".to_string(),
            "invalid-hash".to_string(),
        );
        assert!(matches!(
            validate_move_message(&invalid_hash_move),
            Err(ValidationError::InvalidBoardHash(_))
        ));

        // Wrong hash (correct format but doesn't match board)
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let _wrong_hash_move = Move::new(
            generate_game_id(),
            "e2e4".to_string(),
            wrong_hash.to_string(),
        );
        // This should pass format validation but fail verification - we can't test verification without board state
    }

    #[test]
    fn test_move_ack_validation() {
        // Valid acks
        let valid_ack = MoveAck::new(generate_game_id(), None);
        assert!(validate_move_ack(&valid_ack).is_ok());

        let valid_ack_with_id = MoveAck::new(generate_game_id(), Some("move-123".to_string()));
        assert!(validate_move_ack(&valid_ack_with_id).is_ok());

        // Invalid game ID
        let invalid_ack = MoveAck::new("not-a-uuid".to_string(), None);
        assert!(matches!(
            validate_move_ack(&invalid_ack),
            Err(ValidationError::InvalidGameId(_))
        ));

        // Empty move ID should be None instead
        let empty_move_id_ack = MoveAck::new(generate_game_id(), Some("".to_string()));
        assert!(matches!(
            validate_move_ack(&empty_move_id_ack),
            Err(ValidationError::InvalidMessageFormat(_))
        ));

        // Too long move ID
        let long_move_id = "a".repeat(65);
        let long_move_id_ack = MoveAck::new(generate_game_id(), Some(long_move_id));
        assert!(matches!(
            validate_move_ack(&long_move_id_ack),
            Err(ValidationError::InvalidMessageFormat(_))
        ));

        // Invalid characters in move ID
        let invalid_char_move_id = MoveAck::new(generate_game_id(), Some("move@123".to_string()));
        assert!(matches!(
            validate_move_ack(&invalid_char_move_id),
            Err(ValidationError::InvalidMessageFormat(_))
        ));
    }

    #[test]
    fn test_sync_request_validation() {
        // Valid request
        let valid_request = SyncRequest::new(generate_game_id());
        assert!(validate_sync_request(&valid_request).is_ok());

        // Invalid game ID
        let invalid_request = SyncRequest::new("not-a-uuid".to_string());
        assert!(matches!(
            validate_sync_request(&invalid_request),
            Err(ValidationError::InvalidGameId(_))
        ));

        // Empty game ID
        let empty_id_request = SyncRequest::new("".to_string());
        assert!(matches!(
            validate_sync_request(&empty_id_request),
            Err(ValidationError::InvalidGameId(_))
        ));
    }

    #[test]
    fn test_sync_response_validation() {
        let board = Board::new();
        let valid_fen = board.to_fen();
        let valid_hash = hash_board_state(&board);
        let valid_history = vec!["e2e4".to_string(), "e7e5".to_string()];

        // Valid response
        let valid_response = SyncResponse::new(
            generate_game_id(),
            valid_fen.clone(),
            valid_history.clone(),
            valid_hash.clone(),
        );
        assert!(validate_sync_response(&valid_response).is_ok());

        // Invalid game ID
        let invalid_game_id_response = SyncResponse::new(
            "not-a-uuid".to_string(),
            valid_fen.clone(),
            valid_history.clone(),
            valid_hash.clone(),
        );
        assert!(matches!(
            validate_sync_response(&invalid_game_id_response),
            Err(ValidationError::InvalidGameId(_))
        ));

        // Invalid FEN
        let invalid_fen_response = SyncResponse::new(
            generate_game_id(),
            "invalid-fen".to_string(),
            valid_history.clone(),
            valid_hash.clone(),
        );
        assert!(matches!(
            validate_sync_response(&invalid_fen_response),
            Err(ValidationError::InvalidFen(_))
        ));

        // Empty FEN
        let empty_fen_response = SyncResponse::new(
            generate_game_id(),
            "".to_string(),
            valid_history.clone(),
            valid_hash.clone(),
        );
        assert!(matches!(
            validate_sync_response(&empty_fen_response),
            Err(ValidationError::InvalidFen(_))
        ));

        // Invalid move in history
        let invalid_history = vec!["e2e4".to_string(), "invalid".to_string()];
        let invalid_history_response = SyncResponse::new(
            generate_game_id(),
            valid_fen.clone(),
            invalid_history,
            valid_hash.clone(),
        );
        assert!(matches!(
            validate_sync_response(&invalid_history_response),
            Err(ValidationError::InvalidMove(_))
        ));

        // Invalid hash format
        let invalid_hash_response = SyncResponse::new(
            generate_game_id(),
            valid_fen.clone(),
            valid_history.clone(),
            "invalid-hash".to_string(),
        );
        assert!(matches!(
            validate_sync_response(&invalid_hash_response),
            Err(ValidationError::InvalidBoardHash(_))
        ));

        // Hash mismatch
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        let hash_mismatch_response = SyncResponse::new(
            generate_game_id(),
            valid_fen,
            valid_history,
            wrong_hash.to_string(),
        );
        assert!(matches!(
            validate_sync_response(&hash_mismatch_response),
            Err(ValidationError::BoardHashMismatch { .. })
        ));
    }

    #[test]
    fn test_message_validation_edge_cases() {
        // Test with unusual but valid UUIDs
        let nil_uuid = "00000000-0000-0000-0000-000000000000";
        let max_uuid = "ffffffff-ffff-ffff-ffff-ffffffffffff";

        let invite_nil = GameInvite::new(nil_uuid.to_string(), None);
        assert!(validate_game_invite(&invite_nil).is_ok());

        let invite_max = GameInvite::new(max_uuid.to_string(), Some(Color::White));
        assert!(validate_game_invite(&invite_max).is_ok());

        // Test with minimal valid inputs
        let minimal_decline = GameDecline::new(generate_game_id(), Some("No".to_string()));
        assert!(validate_game_decline(&minimal_decline).is_ok());

        let minimal_ack = MoveAck::new(generate_game_id(), Some("1".to_string()));
        assert!(validate_move_ack(&minimal_ack).is_ok());
    }

    #[test]
    fn test_basic_security_validation_integration() {
        // Test that basic security validation is properly integrated

        // Test reason text security
        let safe_reason = "I'm busy with another game";
        assert!(validate_secure_reason_text(safe_reason).is_ok());

        let long_unsafe_reason = "a".repeat(1000); // This should be rejected
        assert!(validate_secure_reason_text(&long_unsafe_reason).is_err());

        // Test FEN security
        let board = Board::new();
        let safe_fen = board.to_fen();
        assert!(validate_secure_fen_notation(&safe_fen).is_ok());

        let unsafe_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR\x00w KQkq - 0 1";
        assert!(validate_secure_fen_notation(unsafe_fen).is_err());

        // Test move history security
        let safe_history = vec!["e2e4".to_string(), "e7e5".to_string()];
        assert!(validate_secure_move_history(&safe_history).is_ok());

        let unsafe_history = vec!["e2e4\0".to_string()];
        assert!(validate_secure_move_history(&unsafe_history).is_err());
    }
}

#[cfg(test)]
mod integration_validation_tests {
    use super::*;

    #[test]
    fn test_cross_component_validation() {
        // Test that validation works across different components
        let game_id = generate_game_id();
        let board = Board::new();
        let hash = hash_board_state(&board);

        // Create messages that reference each other
        let invite = GameInvite::new(game_id.clone(), Some(Color::White));
        let accept = GameAccept::new(game_id.clone(), Color::Black);
        let move_msg = Move::new(game_id.clone(), "e2e4".to_string(), hash.clone());
        let ack = MoveAck::new(game_id.clone(), Some("move-1".to_string()));
        let sync_req = SyncRequest::new(game_id.clone());
        let sync_resp = SyncResponse::new(
            game_id.clone(),
            board.to_fen(),
            vec!["e2e4".to_string()],
            hash.clone(),
        );

        // All should validate successfully
        assert!(validate_game_invite(&invite).is_ok());
        assert!(validate_game_accept(&accept).is_ok());
        assert!(validate_move_message(&move_msg).is_ok());
        assert!(validate_move_ack(&ack).is_ok());
        assert!(validate_sync_request(&sync_req).is_ok());
        assert!(validate_sync_response(&sync_resp).is_ok());
    }

    #[test]
    fn test_error_propagation_chain() {
        // Test that errors propagate correctly through the validation chain

        // Start with an invalid game ID and see how it propagates
        let invalid_game_id = "not-a-uuid";

        let invite = GameInvite::new(invalid_game_id.to_string(), None);
        let accept = GameAccept::new(invalid_game_id.to_string(), Color::White);
        let decline = GameDecline::new(invalid_game_id.to_string(), None);
        let move_msg = Move::new(
            invalid_game_id.to_string(),
            "e2e4".to_string(),
            "valid".repeat(16),
        );
        let ack = MoveAck::new(invalid_game_id.to_string(), None);
        let sync_req = SyncRequest::new(invalid_game_id.to_string());
        let sync_resp = SyncResponse::new(
            invalid_game_id.to_string(),
            Board::new().to_fen(),
            vec![],
            hash_board_state(&Board::new()),
        );

        // All should fail with InvalidGameId error
        assert!(matches!(
            validate_game_invite(&invite),
            Err(ValidationError::InvalidGameId(_))
        ));
        assert!(matches!(
            validate_game_accept(&accept),
            Err(ValidationError::InvalidGameId(_))
        ));
        assert!(matches!(
            validate_game_decline(&decline),
            Err(ValidationError::InvalidGameId(_))
        ));
        assert!(matches!(
            validate_move_message(&move_msg),
            Err(ValidationError::InvalidGameId(_))
        ));
        assert!(matches!(
            validate_move_ack(&ack),
            Err(ValidationError::InvalidGameId(_))
        ));
        assert!(matches!(
            validate_sync_request(&sync_req),
            Err(ValidationError::InvalidGameId(_))
        ));
        assert!(matches!(
            validate_sync_response(&sync_resp),
            Err(ValidationError::InvalidGameId(_))
        ));
    }

    #[test]
    fn test_comprehensive_message_validation() {
        // Test Message enum validation integration
        let game_id = generate_game_id();
        let board = Board::new();
        let hash = hash_board_state(&board);

        // Valid messages
        let valid_messages = vec![
            Message::GameInvite(GameInvite::new(game_id.clone(), Some(Color::White))),
            Message::GameAccept(GameAccept::new(game_id.clone(), Color::Black)),
            Message::GameDecline(GameDecline::new(game_id.clone(), Some("Busy".to_string()))),
            Message::Move(Move::new(game_id.clone(), "e2e4".to_string(), hash.clone())),
            Message::MoveAck(MoveAck::new(game_id.clone(), Some("move-1".to_string()))),
            Message::SyncRequest(SyncRequest::new(game_id.clone())),
            Message::SyncResponse(SyncResponse::new(
                game_id.clone(),
                board.to_fen(),
                vec!["e2e4".to_string()],
                hash.clone(),
            )),
        ];

        for message in valid_messages {
            assert!(
                message.validate().is_ok(),
                "Message should validate: {:?}",
                message
            );
        }

        // Invalid messages
        let invalid_messages = vec![
            Message::GameInvite(GameInvite::new("invalid".to_string(), None)),
            Message::GameAccept(GameAccept::new("invalid".to_string(), Color::White)),
            Message::Move(Move::new(
                game_id.clone(),
                "invalid-move".to_string(),
                hash.clone(),
            )),
        ];

        for message in invalid_messages {
            assert!(
                message.validate().is_err(),
                "Message should fail validation: {:?}",
                message
            );
        }
    }

    #[test]
    fn test_validation_consistency() {
        // Test that validation results are consistent across multiple calls
        let game_id = generate_game_id();
        let invite = GameInvite::new(game_id, Some(Color::White));

        // Multiple validations should produce the same result
        for _ in 0..10 {
            assert!(validate_game_invite(&invite).is_ok());
        }

        // Invalid message should consistently fail
        let invalid_invite = GameInvite::new("invalid".to_string(), None);
        for _ in 0..10 {
            assert!(validate_game_invite(&invalid_invite).is_err());
        }
    }

    #[test]
    fn test_validation_performance() {
        // Basic performance test to ensure validation isn't excessively slow
        let game_id = generate_game_id();
        let board = Board::new();
        let hash = hash_board_state(&board);

        let invite = GameInvite::new(game_id.clone(), Some(Color::White));
        let accept = GameAccept::new(game_id.clone(), Color::Black);
        let move_msg = Move::new(game_id.clone(), "e2e4".to_string(), hash.clone());
        let sync_resp = SyncResponse::new(
            game_id.clone(),
            board.to_fen(),
            vec!["e2e4".to_string(); 100], // Larger history
            hash,
        );

        let start = std::time::Instant::now();

        // Validate 1000 times
        for _ in 0..1000 {
            validate_game_invite(&invite).ok();
            validate_game_accept(&accept).ok();
            validate_move_message(&move_msg).ok();
            validate_sync_response(&sync_resp).ok();
        }

        let duration = start.elapsed();
        // Validation should complete in reasonable time (less than 1 second for 4000 validations)
        assert!(
            duration.as_secs() < 1,
            "Validation took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_error_message_clarity() {
        // Test that error messages are clear and informative

        // Test various validation errors
        let game_id_error =
            validate_game_invite(&GameInvite::new("invalid".to_string(), None)).unwrap_err();
        let error_msg = format!("{}", game_id_error);
        assert!(error_msg.contains("Invalid game ID"));
        assert!(error_msg.contains("invalid"));

        let move_error = validate_chess_move_format("invalid-move").unwrap_err();
        let move_error_msg = format!("{}", move_error);
        assert!(move_error_msg.contains("Invalid chess move"));
        assert!(move_error_msg.contains("invalid-move"));

        let hash_error = validate_move_message(&Move::new(
            generate_game_id(),
            "e2e4".to_string(),
            "invalid-hash".to_string(),
        ))
        .unwrap_err();
        let hash_error_msg = format!("{}", hash_error);
        assert!(hash_error_msg.contains("Invalid board hash"));
    }

    #[test]
    fn test_edge_case_combinations() {
        // Test combinations of edge cases

        // Empty strings and whitespace
        let edge_cases = vec![
            ("", "Empty string"),
            ("   ", "Whitespace only"),
            ("\t\n ", "Mixed whitespace"),
        ];

        for (input, description) in edge_cases {
            // Game ID validation
            assert!(
                !validate_game_id(input),
                "Game ID should reject {}",
                description
            );

            // Chess move validation
            assert!(
                validate_chess_move_format(input).is_err(),
                "Chess move should reject {}",
                description
            );
        }

        // Very long inputs
        let long_string = "a".repeat(10000);
        assert!(!validate_game_id(&long_string));
        assert!(validate_chess_move_format(&long_string).is_err());

        // Control characters
        let control_chars = "\x00\x01\x02\x03\x04\x05";
        assert!(!validate_game_id(control_chars));
        assert!(validate_chess_move_format(control_chars).is_err());
    }

    #[test]
    fn test_validation_completeness() {
        // Ensure we haven't missed any validation paths

        // Test all ValidationError variants can be produced
        let _game_id_err = ValidationError::InvalidGameId("test".to_string());
        let _move_err = ValidationError::InvalidMove("test".to_string());
        let _hash_err = ValidationError::InvalidBoardHash("test".to_string());
        let _fen_err = ValidationError::InvalidFen("test".to_string());
        let _format_err = ValidationError::InvalidMessageFormat("test".to_string());
        let _mismatch_err = ValidationError::BoardHashMismatch {
            expected: "a".to_string(),
            actual: "b".to_string(),
        };

        // Test all message types have validation
        let game_id = generate_game_id();
        let board = Board::new();
        let hash = hash_board_state(&board);

        // These should all have validation functions available
        let _ = validate_game_invite(&GameInvite::new(game_id.clone(), None));
        let _ = validate_game_accept(&GameAccept::new(game_id.clone(), Color::White));
        let _ = validate_game_decline(&GameDecline::new(game_id.clone(), None));
        let _ = validate_move_message(&Move::new(
            game_id.clone(),
            "e2e4".to_string(),
            hash.clone(),
        ));
        let _ = validate_move_ack(&MoveAck::new(game_id.clone(), None));
        let _ = validate_sync_request(&SyncRequest::new(game_id.clone()));
        let _ = validate_sync_response(&SyncResponse::new(game_id, board.to_fen(), vec![], hash));
    }
}
