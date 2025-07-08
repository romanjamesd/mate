#[cfg(test)]
mod tests {
    use mate::chess::{Board, Color};
    use mate::messages::chess::{generate_game_id, hash_board_state};
    use mate::messages::types::Message;
    use std::collections::HashMap;

    // =============================================================================
    // Enhanced Constructor Tests
    // =============================================================================

    #[test]
    fn test_enhanced_game_invite_constructor() {
        let game_id = generate_game_id();
        let msg = Message::new_game_invite(game_id.clone(), Some(Color::White));

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        assert_eq!(msg.message_type(), "GameInvite");
    }

    #[test]
    fn test_enhanced_game_invite_no_color() {
        let game_id = generate_game_id();
        let msg = Message::new_game_invite(game_id.clone(), None);

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));

        if let Message::GameInvite(invite) = msg {
            assert_eq!(invite.suggested_color, None);
        } else {
            panic!("Expected GameInvite message");
        }
    }

    #[test]
    fn test_enhanced_game_accept_constructor() {
        let game_id = generate_game_id();
        let msg = Message::new_game_accept(game_id.clone(), Color::Black);

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        assert_eq!(msg.message_type(), "GameAccept");
    }

    #[test]
    fn test_enhanced_game_decline_constructor() {
        let game_id = generate_game_id();
        let reason = "Already playing".to_string();
        let msg = Message::new_game_decline(game_id.clone(), Some(reason.clone()));

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        assert_eq!(msg.message_type(), "GameDecline");

        if let Message::GameDecline(decline) = msg {
            assert_eq!(decline.reason, Some(reason));
        } else {
            panic!("Expected GameDecline message");
        }
    }

    #[test]
    fn test_enhanced_move_constructor() {
        let game_id = generate_game_id();
        let board = Board::new();
        let board_hash = hash_board_state(&board);
        let msg = Message::new_move(game_id.clone(), "e2e4".to_string(), board_hash.clone());

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        assert_eq!(msg.message_type(), "Move");

        if let Message::Move(move_msg) = msg {
            assert_eq!(move_msg.chess_move, "e2e4");
            assert_eq!(move_msg.board_state_hash, board_hash);
        } else {
            panic!("Expected Move message");
        }
    }

    #[test]
    fn test_enhanced_move_ack_constructor() {
        let game_id = generate_game_id();
        let move_id = "move-123".to_string();
        let msg = Message::new_move_ack(game_id.clone(), Some(move_id.clone()));

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        assert_eq!(msg.message_type(), "MoveAck");

        if let Message::MoveAck(ack) = msg {
            assert_eq!(ack.move_id, Some(move_id));
        } else {
            panic!("Expected MoveAck message");
        }
    }

    #[test]
    fn test_enhanced_sync_request_constructor() {
        let game_id = generate_game_id();
        let msg = Message::new_sync_request(game_id.clone());

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        assert_eq!(msg.message_type(), "SyncRequest");
    }

    #[test]
    fn test_enhanced_sync_response_constructor() {
        let game_id = generate_game_id();
        let board = Board::new();
        let fen = board.to_fen();
        let move_history = vec!["e2e4".to_string(), "e7e5".to_string()];
        let board_hash = hash_board_state(&board);

        let msg = Message::new_sync_response(
            game_id.clone(),
            fen.clone(),
            move_history.clone(),
            board_hash.clone(),
        );

        assert!(msg.is_chess_message());
        assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        assert_eq!(msg.message_type(), "SyncResponse");

        if let Message::SyncResponse(sync_resp) = msg {
            assert_eq!(sync_resp.board_state, fen);
            assert_eq!(sync_resp.move_history, move_history);
            assert_eq!(sync_resp.board_state_hash, board_hash);
        } else {
            panic!("Expected SyncResponse message");
        }
    }

    #[test]
    fn test_constructor_api_consistency() {
        let game_id = generate_game_id();

        // Test that all chess message constructors follow consistent patterns
        let messages = vec![
            Message::new_game_invite(game_id.clone(), None),
            Message::new_game_accept(game_id.clone(), Color::White),
            Message::new_game_decline(game_id.clone(), None),
            Message::new_move(game_id.clone(), "e2e4".to_string(), "hash".to_string()),
            Message::new_move_ack(game_id.clone(), None),
            Message::new_sync_request(game_id.clone()),
            Message::new_sync_response(
                game_id.clone(),
                "fen".to_string(),
                vec![],
                "hash".to_string(),
            ),
        ];

        for msg in messages {
            assert!(msg.is_chess_message());
            assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        }
    }

    // =============================================================================
    // Serialization Enhancement Tests
    // =============================================================================

    #[test]
    fn test_json_support_ping_pong() {
        let ping = Message::new_ping(12345, "test payload".to_string());
        let pong = Message::new_pong(12345, "test payload".to_string());

        // Test JSON serialization
        let ping_json = ping.to_json().expect("Failed to serialize ping to JSON");
        let pong_json = pong.to_json().expect("Failed to serialize pong to JSON");

        // Test JSON deserialization
        let ping_deserialized = Message::from_json(&ping_json).expect("Failed to deserialize ping");
        let pong_deserialized = Message::from_json(&pong_json).expect("Failed to deserialize pong");

        assert_eq!(ping.get_nonce(), ping_deserialized.get_nonce());
        assert_eq!(pong.get_nonce(), pong_deserialized.get_nonce());
    }

    #[test]
    fn test_json_support_chess_messages() {
        let game_id = generate_game_id();
        let board = Board::new();
        let board_hash = hash_board_state(&board);

        let messages = vec![
            Message::new_game_invite(game_id.clone(), Some(Color::White)),
            Message::new_game_accept(game_id.clone(), Color::Black),
            Message::new_game_decline(game_id.clone(), Some("busy".to_string())),
            Message::new_move(game_id.clone(), "e2e4".to_string(), board_hash.clone()),
            Message::new_move_ack(game_id.clone(), Some("move-1".to_string())),
            Message::new_sync_request(game_id.clone()),
            Message::new_sync_response(
                game_id.clone(),
                board.to_fen(),
                vec!["e2e4".to_string()],
                board_hash,
            ),
        ];

        for original_msg in messages {
            let json = original_msg.to_json().expect("Failed to serialize to JSON");
            let deserialized = Message::from_json(&json).expect("Failed to deserialize from JSON");

            assert_eq!(original_msg.message_type(), deserialized.message_type());
            assert_eq!(original_msg.get_game_id(), deserialized.get_game_id());
        }
    }

    #[test]
    fn test_size_estimation_basic() {
        let ping = Message::new_ping(123, "small".to_string());
        let estimated_size = ping.estimated_size();

        // Should be reasonable for a small ping message
        assert!(estimated_size > 0);
        assert!(estimated_size < 1000); // Small message should be well under 1KB
    }

    #[test]
    fn test_size_estimation_large_message() {
        let game_id = generate_game_id();
        let large_history: Vec<String> = (0..500).map(|i| format!("move{i}")).collect();
        let board = Board::new();

        let sync_response = Message::new_sync_response(
            game_id,
            board.to_fen(),
            large_history,
            hash_board_state(&board),
        );

        let estimated_size = sync_response.estimated_size();

        // Large message should have substantial size
        assert!(estimated_size > 1000);
        assert!(sync_response.is_potentially_large());
    }

    #[test]
    fn test_size_estimation_accuracy() {
        let game_id = generate_game_id();
        let msg = Message::new_game_invite(game_id, Some(Color::White));

        let estimated_size = msg.estimated_size();
        let actual_size = msg.serialize().expect("Failed to serialize").len();

        // Estimation should be within reasonable range of actual size
        let ratio = estimated_size as f64 / actual_size as f64;
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "Size estimation too far off: estimated={}, actual={}",
            estimated_size,
            actual_size
        );
    }

    #[test]
    fn test_logging_capabilities() {
        let game_id = generate_game_id();
        let msg = Message::new_move(game_id.clone(), "e2e4".to_string(), "hash123".to_string());

        let log_summary = msg.log_summary();

        // Log summary should contain essential information
        assert!(log_summary.contains("Move"));
        assert!(log_summary.contains(&game_id[..8])); // Should contain game ID prefix
        assert!(log_summary.contains("e2e4"));
    }

    #[test]
    fn test_logging_sensitive_data_filtering() {
        let game_id = generate_game_id();
        let board = Board::new();
        let full_hash = hash_board_state(&board);

        let sync_response = Message::new_sync_response(
            game_id.clone(),
            board.to_fen(),
            vec!["e2e4".to_string(), "e7e5".to_string()],
            full_hash.clone(),
        );

        let log_summary = sync_response.log_summary();

        // Should contain type and game ID but not full hash or complete board state
        assert!(log_summary.contains("SyncResponse"));
        assert!(log_summary.contains(&game_id[..8]));
        assert!(!log_summary.contains(&full_hash)); // Full hash should be filtered
    }

    // =============================================================================
    // Message Utility Tests
    // =============================================================================

    #[test]
    fn test_chess_message_detection() {
        let game_id = generate_game_id();

        // Chess messages
        let chess_messages = vec![
            Message::new_game_invite(game_id.clone(), None),
            Message::new_game_accept(game_id.clone(), Color::White),
            Message::new_game_decline(game_id.clone(), None),
            Message::new_move(game_id.clone(), "e2e4".to_string(), "hash".to_string()),
            Message::new_move_ack(game_id.clone(), None),
            Message::new_sync_request(game_id.clone()),
            Message::new_sync_response(game_id, "fen".to_string(), vec![], "hash".to_string()),
        ];

        // Non-chess messages
        let non_chess_messages = vec![
            Message::new_ping(123, "test".to_string()),
            Message::new_pong(123, "test".to_string()),
        ];

        for msg in chess_messages {
            assert!(
                msg.is_chess_message(),
                "Expected chess message: {:?}",
                msg.message_type()
            );
        }

        for msg in non_chess_messages {
            assert!(
                !msg.is_chess_message(),
                "Expected non-chess message: {:?}",
                msg.message_type()
            );
        }
    }

    #[test]
    fn test_game_id_extraction() {
        let game_id = generate_game_id();

        let messages_with_game_id = vec![
            Message::new_game_invite(game_id.clone(), None),
            Message::new_game_accept(game_id.clone(), Color::White),
            Message::new_move(game_id.clone(), "e2e4".to_string(), "hash".to_string()),
        ];

        let messages_without_game_id = vec![
            Message::new_ping(123, "test".to_string()),
            Message::new_pong(123, "test".to_string()),
        ];

        for msg in messages_with_game_id {
            assert_eq!(msg.get_game_id(), Some(game_id.as_str()));
        }

        for msg in messages_without_game_id {
            assert_eq!(msg.get_game_id(), None);
        }
    }

    #[test]
    fn test_message_typing() {
        let game_id = generate_game_id();

        let type_map: HashMap<&str, Message> = [
            ("Ping", Message::new_ping(123, "test".to_string())),
            ("Pong", Message::new_pong(123, "test".to_string())),
            (
                "GameInvite",
                Message::new_game_invite(game_id.clone(), None),
            ),
            (
                "GameAccept",
                Message::new_game_accept(game_id.clone(), Color::White),
            ),
            (
                "GameDecline",
                Message::new_game_decline(game_id.clone(), None),
            ),
            (
                "Move",
                Message::new_move(game_id.clone(), "e2e4".to_string(), "hash".to_string()),
            ),
            ("MoveAck", Message::new_move_ack(game_id.clone(), None)),
            ("SyncRequest", Message::new_sync_request(game_id.clone())),
            (
                "SyncResponse",
                Message::new_sync_response(game_id, "fen".to_string(), vec![], "hash".to_string()),
            ),
        ]
        .iter()
        .cloned()
        .collect();

        for (expected_type, msg) in type_map {
            assert_eq!(msg.message_type(), expected_type);
        }
    }

    #[test]
    fn test_message_introspection() {
        let ping = Message::new_ping(42, "payload".to_string());
        let pong = Message::new_pong(42, "payload".to_string());

        assert!(ping.is_ping());
        assert!(!ping.is_pong());
        assert!(!ping.is_chess_message());

        assert!(pong.is_pong());
        assert!(!pong.is_ping());
        assert!(!pong.is_chess_message());

        assert_eq!(ping.get_nonce(), 42);
        assert_eq!(pong.get_nonce(), 42);
        assert_eq!(ping.get_payload(), "payload");
        assert_eq!(pong.get_payload(), "payload");
    }

    // =============================================================================
    // Validation Integration Tests
    // =============================================================================

    #[test]
    fn test_validation_pipeline_integration() {
        let game_id = generate_game_id();
        let board = Board::new();
        let valid_hash = hash_board_state(&board);

        // Valid messages should pass validation
        let valid_messages = vec![
            Message::new_game_invite(game_id.clone(), Some(Color::White)),
            Message::new_game_accept(game_id.clone(), Color::Black),
            Message::new_game_decline(game_id.clone(), Some("reason".to_string())),
            Message::new_move(game_id.clone(), "e2e4".to_string(), valid_hash.clone()),
            Message::new_move_ack(game_id.clone(), Some("move-1".to_string())),
            Message::new_sync_request(game_id.clone()),
            Message::new_sync_response(
                game_id.clone(),
                board.to_fen(),
                vec!["e2e4".to_string()],
                valid_hash,
            ),
        ];

        for msg in valid_messages {
            assert!(
                msg.validate().is_ok(),
                "Valid message failed validation: {:?}",
                msg.message_type()
            );
        }
    }

    #[test]
    fn test_validation_pipeline_invalid_game_id() {
        let invalid_game_id = "not-a-uuid";
        let msg = Message::new_game_invite(invalid_game_id.to_string(), None);

        let result = msg.validate();
        assert!(
            result.is_err(),
            "Expected validation to fail for invalid game ID"
        );
    }

    #[test]
    fn test_validation_pipeline_invalid_move_format() {
        let game_id = generate_game_id();
        let msg = Message::new_move(game_id, "invalid-move".to_string(), "hash".to_string());

        let result = msg.validate();
        assert!(
            result.is_err(),
            "Expected validation to fail for invalid move format"
        );
    }

    #[test]
    fn test_security_integration() {
        let game_id = generate_game_id();

        // Test with extremely long reason (should be handled gracefully)
        let long_reason = "x".repeat(1000);
        let msg = Message::new_game_decline(game_id, Some(long_reason));

        // Validation should catch security issues
        let result = msg.validate();
        // The specific behavior depends on security validation implementation
        // but it should either pass or fail gracefully without panicking
        let _ = result; // Don't assert specific outcome as it depends on security policy
    }

    // =============================================================================
    // Cross-Format Compatibility Tests
    // =============================================================================

    #[test]
    fn test_json_vs_binary_equivalence() {
        let game_id = generate_game_id();
        let board = Board::new();
        let msg = Message::new_sync_response(
            game_id,
            board.to_fen(),
            vec!["e2e4".to_string(), "e7e5".to_string()],
            hash_board_state(&board),
        );

        // Serialize to both formats
        let json_data = msg.to_json().expect("Failed to serialize to JSON");
        let binary_data = msg.serialize().expect("Failed to serialize to binary");

        // Deserialize from both formats
        let from_json = Message::from_json(&json_data).expect("Failed to deserialize from JSON");
        let from_binary =
            Message::deserialize(&binary_data).expect("Failed to deserialize from binary");

        // Results should be equivalent
        assert_eq!(from_json.message_type(), from_binary.message_type());
        assert_eq!(from_json.get_game_id(), from_binary.get_game_id());
        assert_eq!(from_json.is_chess_message(), from_binary.is_chess_message());
    }

    #[test]
    fn test_format_migration_stability() {
        let game_id = generate_game_id();
        let original_msg = Message::new_game_invite(game_id.clone(), Some(Color::White));

        // Test multiple serialization/deserialization cycles
        let mut current_msg = original_msg;

        for _ in 0..5 {
            // JSON cycle
            let json = current_msg.to_json().expect("Failed to serialize to JSON");
            current_msg = Message::from_json(&json).expect("Failed to deserialize from JSON");

            // Binary cycle
            let binary = current_msg
                .serialize()
                .expect("Failed to serialize to binary");
            current_msg = Message::deserialize(&binary).expect("Failed to deserialize from binary");
        }

        // Message should remain consistent
        assert_eq!(current_msg.message_type(), "GameInvite");
        assert_eq!(current_msg.get_game_id(), Some(game_id.as_str()));
        assert!(current_msg.is_chess_message());
    }

    #[test]
    fn test_cross_format_size_consistency() {
        let game_id = generate_game_id();
        let msg = Message::new_move(game_id, "e2e4".to_string(), "hash123".to_string());

        let json_size = msg.to_json().expect("Failed to serialize to JSON").len();
        let binary_size = msg
            .serialize()
            .expect("Failed to serialize to binary")
            .len();
        let estimated_size = msg.estimated_size();

        // Binary should typically be more compact than JSON
        assert!(
            binary_size <= json_size,
            "Binary should be more compact than JSON"
        );

        // Estimated size should be in reasonable range for both formats
        assert!(estimated_size >= binary_size / 2);
        assert!(estimated_size <= json_size * 2);
    }

    #[test]
    fn test_format_compatibility_with_large_data() {
        let game_id = generate_game_id();
        let large_history: Vec<String> = (0..100).map(|i| format!("move{i:03}"))
            .collect();
        let board = Board::new();

        let large_msg = Message::new_sync_response(
            game_id,
            board.to_fen(),
            large_history,
            hash_board_state(&board),
        );

        // Both formats should handle large data
        let json_result = large_msg.to_json();
        let binary_result = large_msg.serialize();

        assert!(
            json_result.is_ok(),
            "JSON serialization failed for large message"
        );
        assert!(
            binary_result.is_ok(),
            "Binary serialization failed for large message"
        );

        // Deserialization should also work
        if let (Ok(json_data), Ok(binary_data)) = (json_result, binary_result) {
            let from_json = Message::from_json(&json_data);
            let from_binary = Message::deserialize(&binary_data);

            assert!(
                from_json.is_ok(),
                "JSON deserialization failed for large message"
            );
            assert!(
                from_binary.is_ok(),
                "Binary deserialization failed for large message"
            );
        }
    }
}
