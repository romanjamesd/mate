#[cfg(test)]
mod tests {
    use mate::chess::{Board, Color};
    use mate::messages::chess::{
        generate_game_id, hash_board_state, GameAccept, GameDecline, GameInvite, Move, MoveAck,
        SyncRequest, SyncResponse,
    };
    use serde_json;

    // =============================================================================
    // GameInvite Tests
    // =============================================================================

    #[test]
    fn test_game_invite_new_basic() {
        let game_id = generate_game_id();
        let invite = GameInvite::new(game_id.clone(), None);

        assert_eq!(invite.game_id, game_id);
        assert_eq!(invite.suggested_color, None);
    }

    #[test]
    fn test_game_invite_new_with_white_color() {
        let game_id = generate_game_id();
        let invite = GameInvite::new(game_id.clone(), Some(Color::White));

        assert_eq!(invite.game_id, game_id);
        assert_eq!(invite.suggested_color, Some(Color::White));
    }

    #[test]
    fn test_game_invite_new_with_black_color() {
        let game_id = generate_game_id();
        let invite = GameInvite::new(game_id.clone(), Some(Color::Black));

        assert_eq!(invite.game_id, game_id);
        assert_eq!(invite.suggested_color, Some(Color::Black));
    }

    #[test]
    fn test_game_invite_new_no_color_preference() {
        let game_id = generate_game_id();
        let invite = GameInvite::new_no_color_preference(game_id.clone());

        assert_eq!(invite.game_id, game_id);
        assert_eq!(invite.suggested_color, None);
    }

    #[test]
    fn test_game_invite_new_with_color() {
        let game_id = generate_game_id();
        let invite = GameInvite::new_with_color(game_id.clone(), Color::White);

        assert_eq!(invite.game_id, game_id);
        assert_eq!(invite.suggested_color, Some(Color::White));
    }

    #[test]
    fn test_game_invite_equality() {
        let game_id = generate_game_id();
        let invite1 = GameInvite::new(game_id.clone(), Some(Color::White));
        let invite2 = GameInvite::new(game_id.clone(), Some(Color::White));
        let invite3 = GameInvite::new(game_id.clone(), Some(Color::Black));

        assert_eq!(invite1, invite2);
        assert_ne!(invite1, invite3);
    }

    #[test]
    fn test_game_invite_json_serialization() {
        let game_id = generate_game_id();
        let invite = GameInvite::new(game_id.clone(), Some(Color::White));

        let json = serde_json::to_string(&invite).expect("Failed to serialize");
        let deserialized: GameInvite = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(invite, deserialized);
    }

    #[test]
    fn test_game_invite_json_roundtrip_no_color() {
        let game_id = generate_game_id();
        let invite = GameInvite::new_no_color_preference(game_id);

        let json = serde_json::to_string(&invite).expect("Failed to serialize");
        let deserialized: GameInvite = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(invite, deserialized);
    }

    #[test]
    fn test_game_invite_binary_serialization() {
        let game_id = generate_game_id();
        let invite = GameInvite::new(game_id, Some(Color::Black));

        let bytes = bincode::serialize(&invite).expect("Failed to serialize");
        let deserialized: GameInvite = bincode::deserialize(&bytes).expect("Failed to deserialize");

        assert_eq!(invite, deserialized);
    }

    // =============================================================================
    // GameAccept Tests
    // =============================================================================

    #[test]
    fn test_game_accept_new_white() {
        let game_id = generate_game_id();
        let accept = GameAccept::new(game_id.clone(), Color::White);

        assert_eq!(accept.game_id, game_id);
        assert_eq!(accept.accepted_color, Color::White);
    }

    #[test]
    fn test_game_accept_new_black() {
        let game_id = generate_game_id();
        let accept = GameAccept::new(game_id.clone(), Color::Black);

        assert_eq!(accept.game_id, game_id);
        assert_eq!(accept.accepted_color, Color::Black);
    }

    #[test]
    fn test_game_accept_equality() {
        let game_id = generate_game_id();
        let accept1 = GameAccept::new(game_id.clone(), Color::White);
        let accept2 = GameAccept::new(game_id.clone(), Color::White);
        let accept3 = GameAccept::new(game_id.clone(), Color::Black);

        assert_eq!(accept1, accept2);
        assert_ne!(accept1, accept3);
    }

    #[test]
    fn test_game_accept_json_serialization() {
        let game_id = generate_game_id();
        let accept = GameAccept::new(game_id, Color::White);

        let json = serde_json::to_string(&accept).expect("Failed to serialize");
        let deserialized: GameAccept = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(accept, deserialized);
    }

    #[test]
    fn test_game_accept_binary_serialization() {
        let game_id = generate_game_id();
        let accept = GameAccept::new(game_id, Color::Black);

        let bytes = bincode::serialize(&accept).expect("Failed to serialize");
        let deserialized: GameAccept = bincode::deserialize(&bytes).expect("Failed to deserialize");

        assert_eq!(accept, deserialized);
    }

    // =============================================================================
    // GameDecline Tests
    // =============================================================================

    #[test]
    fn test_game_decline_new_no_reason() {
        let game_id = generate_game_id();
        let decline = GameDecline::new(game_id.clone(), None);

        assert_eq!(decline.game_id, game_id);
        assert_eq!(decline.reason, None);
    }

    #[test]
    fn test_game_decline_new_with_reason() {
        let game_id = generate_game_id();
        let reason = "Already in a game".to_string();
        let decline = GameDecline::new(game_id.clone(), Some(reason.clone()));

        assert_eq!(decline.game_id, game_id);
        assert_eq!(decline.reason, Some(reason));
    }

    #[test]
    fn test_game_decline_new_no_reason_convenience() {
        let game_id = generate_game_id();
        let decline = GameDecline::new_no_reason(game_id.clone());

        assert_eq!(decline.game_id, game_id);
        assert_eq!(decline.reason, None);
    }

    #[test]
    fn test_game_decline_new_with_reason_convenience() {
        let game_id = generate_game_id();
        let reason = "Not interested".to_string();
        let decline = GameDecline::new_with_reason(game_id.clone(), reason.clone());

        assert_eq!(decline.game_id, game_id);
        assert_eq!(decline.reason, Some(reason));
    }

    #[test]
    fn test_game_decline_equality() {
        let game_id = generate_game_id();
        let decline1 = GameDecline::new(game_id.clone(), Some("reason".to_string()));
        let decline2 = GameDecline::new(game_id.clone(), Some("reason".to_string()));
        let decline3 = GameDecline::new(game_id.clone(), None);

        assert_eq!(decline1, decline2);
        assert_ne!(decline1, decline3);
    }

    #[test]
    fn test_game_decline_json_serialization_with_reason() {
        let game_id = generate_game_id();
        let decline = GameDecline::new_with_reason(game_id, "Busy".to_string());

        let json = serde_json::to_string(&decline).expect("Failed to serialize");
        let deserialized: GameDecline = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(decline, deserialized);
    }

    #[test]
    fn test_game_decline_json_serialization_no_reason() {
        let game_id = generate_game_id();
        let decline = GameDecline::new_no_reason(game_id);

        let json = serde_json::to_string(&decline).expect("Failed to serialize");
        let deserialized: GameDecline = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(decline, deserialized);
    }

    #[test]
    fn test_game_decline_binary_serialization() {
        let game_id = generate_game_id();
        let decline = GameDecline::new_with_reason(game_id, "Long reason here".to_string());

        let bytes = bincode::serialize(&decline).expect("Failed to serialize");
        let deserialized: GameDecline =
            bincode::deserialize(&bytes).expect("Failed to deserialize");

        assert_eq!(decline, deserialized);
    }

    // =============================================================================
    // Move Tests
    // =============================================================================

    #[test]
    fn test_move_new_basic() {
        let game_id = generate_game_id();
        let chess_move = "e2e4".to_string();
        let board = Board::new();
        let board_hash = hash_board_state(&board);

        let move_msg = Move::new(game_id.clone(), chess_move.clone(), board_hash.clone());

        assert_eq!(move_msg.game_id, game_id);
        assert_eq!(move_msg.chess_move, chess_move);
        assert_eq!(move_msg.board_state_hash, board_hash);
    }

    #[test]
    fn test_move_new_castling() {
        let game_id = generate_game_id();
        let chess_move = "O-O".to_string();
        let board = Board::new();
        let board_hash = hash_board_state(&board);

        let move_msg = Move::new(game_id.clone(), chess_move.clone(), board_hash.clone());

        assert_eq!(move_msg.game_id, game_id);
        assert_eq!(move_msg.chess_move, chess_move);
        assert_eq!(move_msg.board_state_hash, board_hash);
    }

    #[test]
    fn test_move_new_promotion() {
        let game_id = generate_game_id();
        let chess_move = "e7e8=Q".to_string();
        let board = Board::new();
        let board_hash = hash_board_state(&board);

        let move_msg = Move::new(game_id.clone(), chess_move.clone(), board_hash.clone());

        assert_eq!(move_msg.game_id, game_id);
        assert_eq!(move_msg.chess_move, chess_move);
        assert_eq!(move_msg.board_state_hash, board_hash);
    }

    #[test]
    fn test_move_equality() {
        let game_id = generate_game_id();
        let chess_move = "Nf3".to_string();
        let board_hash = hash_board_state(&Board::new());

        let move1 = Move::new(game_id.clone(), chess_move.clone(), board_hash.clone());
        let move2 = Move::new(game_id.clone(), chess_move.clone(), board_hash.clone());
        let move3 = Move::new(game_id.clone(), "d2d4".to_string(), board_hash.clone());

        assert_eq!(move1, move2);
        assert_ne!(move1, move3);
    }

    #[test]
    fn test_move_board_hash_integration() {
        let game_id = generate_game_id();
        let board1 = Board::new();
        let board2 = Board::new();

        let hash1 = hash_board_state(&board1);
        let hash2 = hash_board_state(&board2);

        // Same board state should produce same hash
        assert_eq!(hash1, hash2);

        let move_msg = Move::new(game_id, "e2e4".to_string(), hash1);
        assert_eq!(move_msg.board_state_hash.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn test_move_json_serialization() {
        let game_id = generate_game_id();
        let move_msg = Move::new(game_id, "Qh5".to_string(), hash_board_state(&Board::new()));

        let json = serde_json::to_string(&move_msg).expect("Failed to serialize");
        let deserialized: Move = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(move_msg, deserialized);
    }

    #[test]
    fn test_move_binary_serialization() {
        let game_id = generate_game_id();
        let move_msg = Move::new(
            game_id,
            "O-O-O".to_string(),
            hash_board_state(&Board::new()),
        );

        let bytes = bincode::serialize(&move_msg).expect("Failed to serialize");
        let deserialized: Move = bincode::deserialize(&bytes).expect("Failed to deserialize");

        assert_eq!(move_msg, deserialized);
    }

    // =============================================================================
    // MoveAck Tests
    // =============================================================================

    #[test]
    fn test_move_ack_new_no_move_id() {
        let game_id = generate_game_id();
        let ack = MoveAck::new(game_id.clone(), None);

        assert_eq!(ack.game_id, game_id);
        assert_eq!(ack.move_id, None);
    }

    #[test]
    fn test_move_ack_new_with_move_id() {
        let game_id = generate_game_id();
        let move_id = "move-123".to_string();
        let ack = MoveAck::new(game_id.clone(), Some(move_id.clone()));

        assert_eq!(ack.game_id, game_id);
        assert_eq!(ack.move_id, Some(move_id));
    }

    #[test]
    fn test_move_ack_new_no_move_id_convenience() {
        let game_id = generate_game_id();
        let ack = MoveAck::new_no_move_id(game_id.clone());

        assert_eq!(ack.game_id, game_id);
        assert_eq!(ack.move_id, None);
    }

    #[test]
    fn test_move_ack_new_with_move_id_convenience() {
        let game_id = generate_game_id();
        let move_id = "move-456".to_string();
        let ack = MoveAck::new_with_move_id(game_id.clone(), move_id.clone());

        assert_eq!(ack.game_id, game_id);
        assert_eq!(ack.move_id, Some(move_id));
    }

    #[test]
    fn test_move_ack_equality() {
        let game_id = generate_game_id();
        let ack1 = MoveAck::new(game_id.clone(), Some("id".to_string()));
        let ack2 = MoveAck::new(game_id.clone(), Some("id".to_string()));
        let ack3 = MoveAck::new(game_id.clone(), None);

        assert_eq!(ack1, ack2);
        assert_ne!(ack1, ack3);
    }

    #[test]
    fn test_move_ack_json_serialization_with_move_id() {
        let game_id = generate_game_id();
        let ack = MoveAck::new_with_move_id(game_id, "move-789".to_string());

        let json = serde_json::to_string(&ack).expect("Failed to serialize");
        let deserialized: MoveAck = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(ack, deserialized);
    }

    #[test]
    fn test_move_ack_json_serialization_no_move_id() {
        let game_id = generate_game_id();
        let ack = MoveAck::new_no_move_id(game_id);

        let json = serde_json::to_string(&ack).expect("Failed to serialize");
        let deserialized: MoveAck = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(ack, deserialized);
    }

    #[test]
    fn test_move_ack_binary_serialization() {
        let game_id = generate_game_id();
        let ack = MoveAck::new_with_move_id(game_id, "complex-move-id-123".to_string());

        let bytes = bincode::serialize(&ack).expect("Failed to serialize");
        let deserialized: MoveAck = bincode::deserialize(&bytes).expect("Failed to deserialize");

        assert_eq!(ack, deserialized);
    }

    // =============================================================================
    // SyncRequest Tests
    // =============================================================================

    #[test]
    fn test_sync_request_new() {
        let game_id = generate_game_id();
        let request = SyncRequest::new(game_id.clone());

        assert_eq!(request.game_id, game_id);
    }

    #[test]
    fn test_sync_request_equality() {
        let game_id = generate_game_id();
        let request1 = SyncRequest::new(game_id.clone());
        let request2 = SyncRequest::new(game_id.clone());
        let request3 = SyncRequest::new(generate_game_id());

        assert_eq!(request1, request2);
        assert_ne!(request1, request3);
    }

    #[test]
    fn test_sync_request_json_serialization() {
        let game_id = generate_game_id();
        let request = SyncRequest::new(game_id);

        let json = serde_json::to_string(&request).expect("Failed to serialize");
        let deserialized: SyncRequest = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(request, deserialized);
    }

    #[test]
    fn test_sync_request_binary_serialization() {
        let game_id = generate_game_id();
        let request = SyncRequest::new(game_id);

        let bytes = bincode::serialize(&request).expect("Failed to serialize");
        let deserialized: SyncRequest =
            bincode::deserialize(&bytes).expect("Failed to deserialize");

        assert_eq!(request, deserialized);
    }

    // =============================================================================
    // SyncResponse Tests
    // =============================================================================

    #[test]
    fn test_sync_response_new_empty_history() {
        let game_id = generate_game_id();
        let board = Board::new();
        let board_state = board.to_fen();
        let move_history = vec![];
        let board_hash = hash_board_state(&board);

        let response = SyncResponse::new(
            game_id.clone(),
            board_state.clone(),
            move_history.clone(),
            board_hash.clone(),
        );

        assert_eq!(response.game_id, game_id);
        assert_eq!(response.board_state, board_state);
        assert_eq!(response.move_history, move_history);
        assert_eq!(response.board_state_hash, board_hash);
    }

    #[test]
    fn test_sync_response_new_with_move_history() {
        let game_id = generate_game_id();
        let board = Board::new();
        let board_state = board.to_fen();
        let move_history = vec!["e2e4".to_string(), "e7e5".to_string()];
        let board_hash = hash_board_state(&board);

        let response = SyncResponse::new(
            game_id.clone(),
            board_state.clone(),
            move_history.clone(),
            board_hash.clone(),
        );

        assert_eq!(response.game_id, game_id);
        assert_eq!(response.board_state, board_state);
        assert_eq!(response.move_history, move_history);
        assert_eq!(response.board_state_hash, board_hash);
    }

    #[test]
    fn test_sync_response_new_large_move_history() {
        let game_id = generate_game_id();
        let board = Board::new();
        let board_state = board.to_fen();
        let board_hash = hash_board_state(&board);

        // Create a larger move history to test message handling
        let move_history: Vec<String> = (1..=50)
            .map(|i| {
                if i % 2 == 1 {
                    format!("e{}e{}", i % 7 + 2, i % 7 + 3)
                } else {
                    format!("d{}d{}", i % 7 + 2, i % 7 + 3)
                }
            })
            .collect();

        let response = SyncResponse::new(
            game_id.clone(),
            board_state.clone(),
            move_history.clone(),
            board_hash.clone(),
        );

        assert_eq!(response.game_id, game_id);
        assert_eq!(response.board_state, board_state);
        assert_eq!(response.move_history.len(), 50);
        assert_eq!(response.board_state_hash, board_hash);
    }

    #[test]
    fn test_sync_response_equality() {
        let game_id = generate_game_id();
        let board = Board::new();
        let board_state = board.to_fen();
        let move_history = vec!["Nf3".to_string()];
        let board_hash = hash_board_state(&board);

        let response1 = SyncResponse::new(
            game_id.clone(),
            board_state.clone(),
            move_history.clone(),
            board_hash.clone(),
        );
        let response2 = SyncResponse::new(
            game_id.clone(),
            board_state.clone(),
            move_history.clone(),
            board_hash.clone(),
        );
        let response3 = SyncResponse::new(
            game_id.clone(),
            board_state.clone(),
            vec!["d2d4".to_string()],
            board_hash.clone(),
        );

        assert_eq!(response1, response2);
        assert_ne!(response1, response3);
    }

    #[test]
    fn test_sync_response_json_serialization() {
        let game_id = generate_game_id();
        let board = Board::new();
        let response = SyncResponse::new(
            game_id,
            board.to_fen(),
            vec!["e2e4".to_string(), "e7e5".to_string()],
            hash_board_state(&board),
        );

        let json = serde_json::to_string(&response).expect("Failed to serialize");
        let deserialized: SyncResponse =
            serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_sync_response_binary_serialization() {
        let game_id = generate_game_id();
        let board = Board::new();
        let response = SyncResponse::new(
            game_id,
            board.to_fen(),
            vec!["Nf3".to_string(), "Nc6".to_string(), "Bb5".to_string()],
            hash_board_state(&board),
        );

        let bytes = bincode::serialize(&response).expect("Failed to serialize");
        let deserialized: SyncResponse =
            bincode::deserialize(&bytes).expect("Failed to deserialize");

        assert_eq!(response, deserialized);
    }

    #[test]
    fn test_sync_response_large_message_handling() {
        let game_id = generate_game_id();
        let board = Board::new();
        let board_state = board.to_fen();
        let board_hash = hash_board_state(&board);

        // Create an extensive move history for large message testing
        let move_history: Vec<String> = (1..=100).map(|i| format!("move_{}", i)).collect();

        let response = SyncResponse::new(game_id, board_state, move_history, board_hash);

        // Test that large messages can be serialized and deserialized
        let json = serde_json::to_string(&response).expect("Failed to serialize large message");
        let _deserialized: SyncResponse =
            serde_json::from_str(&json).expect("Failed to deserialize large message");

        let bytes =
            bincode::serialize(&response).expect("Failed to serialize large message to binary");
        let _deserialized: SyncResponse =
            bincode::deserialize(&bytes).expect("Failed to deserialize large binary message");

        assert_eq!(response.move_history.len(), 100);
    }
}
