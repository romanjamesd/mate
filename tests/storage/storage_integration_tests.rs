use mate::storage::{Database, GameStatus, PlayerColor};
use tempfile::TempDir;

/// Test helper that ensures proper environment cleanup
struct TestEnvironment {
    _temp_dir: TempDir,
    original_data_dir: Option<String>,
}

impl TestEnvironment {
    fn new() -> (Database, Self) {
        // Save original environment variable
        let original_data_dir = std::env::var("MATE_DATA_DIR").ok();

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Use a unique directory for each test to avoid pollution
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos();
        let unique_temp_dir = temp_dir
            .path()
            .join(format!("test_integration_{}", timestamp));
        std::fs::create_dir_all(&unique_temp_dir).expect("Failed to create unique test dir");

        // Override the database path for testing
        std::env::set_var("MATE_DATA_DIR", &unique_temp_dir);

        let db = Database::new("test_peer_integration").expect("Failed to create test database");

        let env = TestEnvironment {
            _temp_dir: temp_dir,
            original_data_dir,
        };

        (db, env)
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // Restore original environment variable
        match &self.original_data_dir {
            Some(original) => std::env::set_var("MATE_DATA_DIR", original),
            None => std::env::remove_var("MATE_DATA_DIR"),
        }
    }
}

/// Test helper to create a temporary database
fn create_test_database() -> (Database, TestEnvironment) {
    TestEnvironment::new()
}

/// Priority 1: Complete Game Lifecycle Tests (3 tests)
/// Test full game workflows from creation to completion

#[test]
fn test_complete_game_lifecycle_with_messages() {
    let (db, _env) = create_test_database();

    // Phase 1: Game Creation
    let game = db
        .create_game(
            "opponent_lifecycle".to_string(),
            PlayerColor::White,
            Some(serde_json::json!({
                "initial_fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
                "time_control": {"initial_time_ms": 600000, "increment_ms": 10000}
            })),
        )
        .expect("Failed to create game");

    assert_eq!(game.status, GameStatus::Pending);

    // Phase 2: Game Initialization Messages
    let _game_request_msg = db.store_message(
        game.id.clone(),
        "game_request".to_string(),
        serde_json::json!({"color": "white", "time_control": {"initial": 600, "increment": 10}}).to_string(),
        "request_signature".to_string(),
        "test_peer_integration".to_string(),
    ).expect("Failed to store game request message");

    let _game_accept_msg = db
        .store_message(
            game.id.clone(),
            "game_accept".to_string(),
            serde_json::json!({"accepted": true}).to_string(),
            "accept_signature".to_string(),
            "opponent_lifecycle".to_string(),
        )
        .expect("Failed to store game accept message");

    // Phase 3: Activate Game
    db.update_game_status(&game.id, GameStatus::Active)
        .expect("Failed to activate game");

    let active_game = db.get_game(&game.id).expect("Failed to get active game");
    assert_eq!(active_game.status, GameStatus::Active);

    // Phase 4: Game Moves
    let moves = vec![
        ("e2e4", "test_peer_integration"),
        ("e7e5", "opponent_lifecycle"),
        ("Nf3", "test_peer_integration"),
        ("Nc6", "opponent_lifecycle"),
    ];

    for (move_notation, sender) in moves {
        db.store_message(
            game.id.clone(),
            "move".to_string(),
            serde_json::json!({"move": move_notation, "fen": "updated_position"}).to_string(),
            format!("move_sig_{}", move_notation),
            sender.to_string(),
        )
        .expect(&format!("Failed to store move: {}", move_notation));
    }

    // Phase 5: Chat Messages
    db.store_message(
        game.id.clone(),
        "chat".to_string(),
        serde_json::json!({"message": "Great opening!"}).to_string(),
        "chat_sig_1".to_string(),
        "opponent_lifecycle".to_string(),
    )
    .expect("Failed to store chat message");

    // Phase 6: Game Completion
    db.update_game_result(&game.id, mate::storage::models::GameResult::Win)
        .expect("Failed to update game result");

    let completed_game = db.get_game(&game.id).expect("Failed to get completed game");
    assert_eq!(completed_game.status, GameStatus::Completed);
    assert!(completed_game.result.is_some());
    assert!(completed_game.completed_at.is_some());

    // Phase 7: Verification of Complete History
    let all_messages = db
        .get_messages_for_game(&game.id)
        .expect("Failed to get all game messages");

    // Should have: 1 request + 1 accept + 4 moves + 1 chat = 7 messages
    assert_eq!(all_messages.len(), 7, "Should have all game messages");

    // Verify message types
    let move_messages = db
        .get_messages_by_type(&game.id, "move")
        .expect("Failed to get move messages");
    assert_eq!(move_messages.len(), 4, "Should have 4 move messages");

    let chat_messages = db
        .get_messages_by_type(&game.id, "chat")
        .expect("Failed to get chat messages");
    assert_eq!(chat_messages.len(), 1, "Should have 1 chat message");

    // Verify chronological order
    for i in 1..all_messages.len() {
        assert!(
            all_messages[i - 1].created_at <= all_messages[i].created_at,
            "Messages should be in chronological order"
        );
    }
}

#[test]
fn test_concurrent_games_workflow() {
    let (db, _env) = create_test_database();

    // Create multiple concurrent games
    let game1 = db
        .create_game(
            "opponent_concurrent_1".to_string(),
            PlayerColor::White,
            None,
        )
        .expect("Failed to create game 1");

    let game2 = db
        .create_game(
            "opponent_concurrent_2".to_string(),
            PlayerColor::Black,
            None,
        )
        .expect("Failed to create game 2");

    let game3 = db
        .create_game(
            "opponent_concurrent_1".to_string(), // Same opponent as game1
            PlayerColor::White,
            None,
        )
        .expect("Failed to create game 3");

    // Activate games with different timings
    db.update_game_status(&game1.id, GameStatus::Active)
        .expect("Failed to activate game 1");
    db.update_game_status(&game2.id, GameStatus::Active)
        .expect("Failed to activate game 2");
    // game3 remains pending

    // Add moves to active games
    db.store_message(
        game1.id.clone(),
        "move".to_string(),
        r#"{"move": "e4"}"#.to_string(),
        "sig_g1_m1".to_string(),
        "test_peer_integration".to_string(),
    )
    .expect("Failed to store move in game 1");

    db.store_message(
        game2.id.clone(),
        "move".to_string(),
        r#"{"move": "d4"}"#.to_string(),
        "sig_g2_m1".to_string(),
        "test_peer_integration".to_string(),
    )
    .expect("Failed to store move in game 2");

    // Complete one game
    db.update_game_result(&game1.id, mate::storage::models::GameResult::Win)
        .expect("Failed to complete game 1");

    // Abandon another game
    db.update_game_status(&game3.id, GameStatus::Abandoned)
        .expect("Failed to abandon game 3");

    // Verify game states
    let final_game1 = db.get_game(&game1.id).expect("Failed to get game 1");
    let final_game2 = db.get_game(&game2.id).expect("Failed to get game 2");
    let final_game3 = db.get_game(&game3.id).expect("Failed to get game 3");

    assert_eq!(final_game1.status, GameStatus::Completed);
    assert_eq!(final_game2.status, GameStatus::Active);
    assert_eq!(final_game3.status, GameStatus::Abandoned);

    // Verify queries work correctly with mixed states
    let active_games = db
        .get_games_by_status(GameStatus::Active)
        .expect("Failed to get active games");
    assert_eq!(active_games.len(), 1, "Should have 1 active game");
    assert_eq!(active_games[0].id, game2.id);

    let opponent1_games = db
        .get_games_with_opponent("opponent_concurrent_1")
        .expect("Failed to get games with opponent 1");
    assert_eq!(
        opponent1_games.len(),
        2,
        "Should have 2 games with opponent 1"
    );

    let completed_games = db
        .get_games_by_status(GameStatus::Completed)
        .expect("Failed to get completed games");
    assert_eq!(completed_games.len(), 1, "Should have 1 completed game");

    let abandoned_games = db
        .get_games_by_status(GameStatus::Abandoned)
        .expect("Failed to get abandoned games");
    assert_eq!(abandoned_games.len(), 1, "Should have 1 abandoned game");
}

#[test]
fn test_game_with_extensive_message_history() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("opponent_extensive".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    db.update_game_status(&game.id, GameStatus::Active)
        .expect("Failed to activate game");

    // Simulate a long game with many moves and chat
    let mut move_count = 0;
    let mut chat_count = 0;

    // Add 50 moves (25 per player)
    for i in 0..50 {
        let sender = if i % 2 == 0 {
            "test_peer_integration"
        } else {
            "opponent_extensive"
        };
        let move_notation = format!("move_{}", i + 1);

        db.store_message(
            game.id.clone(),
            "move".to_string(),
            serde_json::json!({"move": move_notation, "move_number": i + 1}).to_string(),
            format!("move_sig_{}", i),
            sender.to_string(),
        )
        .expect(&format!("Failed to store move {}", i));

        move_count += 1;

        // Add occasional chat messages
        if i % 10 == 9 {
            db.store_message(
                game.id.clone(),
                "chat".to_string(),
                serde_json::json!({"message": format!("Good move #{}", i + 1)}).to_string(),
                format!("chat_sig_{}", chat_count),
                sender.to_string(),
            )
            .expect(&format!("Failed to store chat {}", chat_count));

            chat_count += 1;
        }
    }

    // Add some game state messages
    db.store_message(
        game.id.clone(),
        "draw_offer".to_string(),
        serde_json::json!({"offered": true}).to_string(),
        "draw_offer_sig".to_string(),
        "test_peer_integration".to_string(),
    )
    .expect("Failed to store draw offer");

    db.store_message(
        game.id.clone(),
        "draw_decline".to_string(),
        serde_json::json!({"declined": true}).to_string(),
        "draw_decline_sig".to_string(),
        "opponent_extensive".to_string(),
    )
    .expect("Failed to store draw decline");

    // Complete the game
    db.update_game_result(&game.id, mate::storage::models::GameResult::Draw)
        .expect("Failed to set draw result");

    // Verify complete message history
    let all_messages = db
        .get_messages_for_game(&game.id)
        .expect("Failed to get all messages");

    let expected_total = move_count + chat_count + 2; // +2 for draw offer/decline
    assert_eq!(
        all_messages.len(),
        expected_total,
        "Should have all messages stored"
    );

    // Test pagination on large message set
    let page1 = db
        .get_messages_for_game_paginated(&game.id, 10, 0)
        .expect("Failed to get first page");
    assert_eq!(page1.len(), 10, "First page should have 10 messages");

    let last_page_offset = (expected_total / 10) * 10;
    let last_page = db
        .get_messages_for_game_paginated(&game.id, 10, last_page_offset as u32)
        .expect("Failed to get last page");
    assert!(
        last_page.len() <= 10,
        "Last page should have <= 10 messages"
    );

    // Verify message type distribution
    let move_messages = db
        .get_messages_by_type(&game.id, "move")
        .expect("Failed to get move messages");
    assert_eq!(
        move_messages.len(),
        move_count,
        "Should have correct number of move messages"
    );

    let chat_messages = db
        .get_messages_by_type(&game.id, "chat")
        .expect("Failed to get chat messages");
    assert_eq!(
        chat_messages.len(),
        chat_count,
        "Should have correct number of chat messages"
    );

    // Test message count function
    let total_count = db
        .count_messages_for_game(&game.id)
        .expect("Failed to count messages");
    assert_eq!(
        total_count as usize, expected_total,
        "Message count should match actual count"
    );
}

/// Priority 2: Database Performance and Concurrency (2 tests)
/// Test performance characteristics and concurrent access

#[test]
fn test_database_connection_management() {
    let (db, _env) = create_test_database();

    // Test multiple operations to verify connection management
    let initial_stats = db.get_connection_stats();

    // Perform multiple database operations
    for i in 0..20 {
        let game = db
            .create_game(
                format!("opponent_conn_{}", i),
                if i % 2 == 0 {
                    PlayerColor::White
                } else {
                    PlayerColor::Black
                },
                None,
            )
            .expect(&format!("Failed to create game {}", i));

        db.store_message(
            game.id,
            "test".to_string(),
            format!(r#"{{"test": {}}}"#, i),
            format!("sig_{}", i),
            "test_sender".to_string(),
        )
        .expect(&format!("Failed to store message {}", i));
    }

    let final_stats = db.get_connection_stats();

    // Verify operations were tracked
    assert!(
        final_stats.0 > initial_stats.0,
        "Operations count should increase"
    );
    assert_eq!(
        final_stats.2, initial_stats.2,
        "Error count should remain the same"
    );

    // Test connection health
    assert!(
        db.check_connection_health().unwrap(),
        "Connection should remain healthy"
    );

    // Test maintenance operations
    db.perform_maintenance()
        .expect("Maintenance should succeed");

    // Verify database is still functional after maintenance
    let test_game = db
        .create_game(
            "post_maintenance_test".to_string(),
            PlayerColor::White,
            None,
        )
        .expect("Should be able to create game after maintenance");

    assert!(
        db.get_game(&test_game.id).is_ok(),
        "Should be able to retrieve game after maintenance"
    );
}

#[test]
fn test_transaction_consistency() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("transaction_test".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    // Test that game updates are consistent
    let initial_updated_at = game.updated_at;

    // Add a delay to ensure timestamp difference (SQLite uses second precision)
    std::thread::sleep(std::time::Duration::from_millis(1100));

    // Update status and verify timestamp consistency
    db.update_game_status(&game.id, GameStatus::Active)
        .expect("Failed to update status");

    let updated_game = db
        .get_game(&game.id)
        .expect("Failed to retrieve updated game");

    assert!(
        updated_game.updated_at > initial_updated_at,
        "Updated timestamp should change"
    );
    assert_eq!(
        updated_game.status,
        GameStatus::Active,
        "Status should be updated"
    );

    // Test result update transaction consistency
    db.update_game_result(&game.id, mate::storage::models::GameResult::Win)
        .expect("Failed to update result");

    let final_game = db
        .get_game(&game.id)
        .expect("Failed to retrieve final game");

    // All fields should be updated consistently
    assert_eq!(final_game.status, GameStatus::Completed);
    assert!(final_game.result.is_some());
    assert!(final_game.completed_at.is_some());
    assert!(final_game.updated_at >= updated_game.updated_at);

    // Completed timestamp should be consistent with updated timestamp
    let completed_at = final_game.completed_at.unwrap();
    let updated_at = final_game.updated_at;
    assert!(
        completed_at >= updated_at - 1000,
        "Timestamps should be close"
    ); // Allow 1 second tolerance
}

/// Priority 3: Schema and Migration Tests (2 tests)
/// Test database schema creation and evolution

#[test]
fn test_database_schema_creation() {
    let (db, _env) = create_test_database();

    // Verify that we can perform all expected operations
    // This implicitly tests that the schema was created correctly

    // Test games table
    let game = db
        .create_game(
            "schema_test".to_string(),
            PlayerColor::White,
            Some(serde_json::json!({"test": "data"})),
        )
        .expect("Games table should be functional");

    // Test all game operations
    db.update_game_status(&game.id, GameStatus::Active)
        .expect("Status updates should work");
    db.update_game_result(&game.id, mate::storage::models::GameResult::Win)
        .expect("Result updates should work");

    let retrieved_game = db.get_game(&game.id).expect("Game retrieval should work");
    assert!(
        retrieved_game.metadata.is_some(),
        "Metadata should be preserved"
    );

    // Test messages table with foreign key relationship
    let message = db
        .store_message(
            game.id.clone(),
            "test".to_string(),
            "test_content".to_string(),
            "test_signature".to_string(),
            "test_sender".to_string(),
        )
        .expect("Messages table should be functional");

    let retrieved_message = db
        .get_message(message.id.unwrap())
        .expect("Message retrieval should work");
    assert_eq!(
        retrieved_message.game_id, game.id,
        "Foreign key relationship should work"
    );

    // Test cascade delete (delete game should remove messages)
    db.delete_game(&game.id).expect("Game deletion should work");

    // Message should be deleted due to CASCADE
    let message_result = db.get_message(message.id.unwrap());
    assert!(
        message_result.is_err(),
        "Message should be deleted when game is deleted"
    );
}

#[test]
fn test_database_indexes_and_performance() {
    let (db, _env) = create_test_database();

    // Create multiple games and messages to test index effectiveness
    let mut game_ids = Vec::new();

    // Create games with different opponents
    for i in 0..10 {
        let opponent = format!("opponent_{}", i % 3); // 3 different opponents
        let game = db
            .create_game(
                opponent,
                if i % 2 == 0 {
                    PlayerColor::White
                } else {
                    PlayerColor::Black
                },
                None,
            )
            .expect(&format!("Failed to create game {}", i));

        game_ids.push(game.id);

        // Add messages to each game
        for j in 0..5 {
            db.store_message(
                game_ids[i].clone(),
                "move".to_string(),
                format!(r#"{{"move": {}}}"#, j),
                format!("sig_{}_{}", i, j),
                format!("sender_{}", j % 2),
            )
            .expect(&format!("Failed to store message {} for game {}", j, i));
        }
    }

    // Test queries that should benefit from indexes
    // These operations should complete quickly due to proper indexing

    // Query by opponent (should use opponent_peer_id index)
    let opponent_0_games = db
        .get_games_with_opponent("opponent_0")
        .expect("Failed to query games by opponent");
    assert!(
        opponent_0_games.len() > 0,
        "Should find games for opponent_0"
    );

    // Query by status (should use status index)
    let pending_games = db
        .get_games_by_status(GameStatus::Pending)
        .expect("Failed to query games by status");
    assert_eq!(pending_games.len(), 10, "All games should be pending");

    // Query messages by game_id (should use game_id index)
    for game_id in &game_ids {
        let messages = db
            .get_messages_for_game(game_id)
            .expect("Failed to query messages by game_id");
        assert_eq!(messages.len(), 5, "Each game should have 5 messages");
    }

    // Query messages by type (should use message_type index if exists)
    let move_messages = db
        .get_messages_by_type(&game_ids[0], "move")
        .expect("Failed to query messages by type");
    assert_eq!(move_messages.len(), 5, "Should find all move messages");

    // Test recent queries (should use created_at index)
    let recent_games = db.get_recent_games(5).expect("Failed to get recent games");
    assert_eq!(recent_games.len(), 5, "Should get 5 most recent games");

    let recent_messages = db
        .get_recent_messages(10)
        .expect("Failed to get recent messages");
    assert_eq!(
        recent_messages.len(),
        10,
        "Should get 10 most recent messages"
    );
}

/// Priority 4: Data Integrity and Consistency (1 test)
/// Test data integrity constraints and consistency

#[test]
fn test_data_integrity_and_consistency() {
    let (db, _env) = create_test_database();

    // Test game data integrity
    let game = db
        .create_game(
            "integrity_test".to_string(),
            PlayerColor::White,
            Some(serde_json::json!({
                "complex_data": {
                    "nested": "value",
                    "array": [1, 2, 3],
                    "boolean": true
                }
            })),
        )
        .expect("Failed to create game with complex metadata");

    // Verify complex metadata is preserved correctly
    let retrieved_game = db.get_game(&game.id).expect("Failed to retrieve game");

    let metadata = retrieved_game.metadata.as_ref().unwrap();
    assert_eq!(metadata["complex_data"]["nested"], "value");
    assert_eq!(metadata["complex_data"]["array"][1], 2);
    assert_eq!(metadata["complex_data"]["boolean"], true);

    // Test message data integrity
    let complex_content = serde_json::json!({
        "move": {
            "from": "e2",
            "to": "e4",
            "piece": "pawn",
            "promotion": null
        },
        "game_state": {
            "fen": "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
            "turn": "black",
            "castling": "KQkq"
        }
    })
    .to_string();

    let message = db
        .store_message(
            game.id.clone(),
            "complex_move".to_string(),
            complex_content.clone(),
            "complex_signature".to_string(),
            "complex_sender".to_string(),
        )
        .expect("Failed to store complex message");

    let retrieved_message = db
        .get_message(message.id.unwrap())
        .expect("Failed to retrieve complex message");

    // Verify complex JSON content is preserved
    let parsed_content: serde_json::Value = serde_json::from_str(&retrieved_message.content)
        .expect("Failed to parse retrieved message content");

    assert_eq!(parsed_content["move"]["from"], "e2");
    assert_eq!(parsed_content["game_state"]["turn"], "black");

    // Test timestamp consistency and ordering
    let initial_time = game.created_at;

    // Add multiple messages with small delays to test ordering
    let mut message_times = Vec::new();
    for i in 0..3 {
        std::thread::sleep(std::time::Duration::from_millis(10)); // Small delay
        let msg = db
            .store_message(
                game.id.clone(),
                "timing_test".to_string(),
                format!(r#"{{"order": {}}}"#, i),
                format!("timing_sig_{}", i),
                "timing_sender".to_string(),
            )
            .expect(&format!("Failed to store timing message {}", i));

        message_times.push(msg.created_at);
    }

    // Verify timestamps are ordered
    for i in 1..message_times.len() {
        assert!(
            message_times[i] >= message_times[i - 1],
            "Message timestamps should be non-decreasing"
        );
    }

    // Verify all message timestamps are after game creation
    for &msg_time in &message_times {
        assert!(
            msg_time >= initial_time,
            "Message timestamps should be after game creation"
        );
    }

    // Test game update timestamp consistency
    let pre_update_time = retrieved_game.updated_at;

    // Add a delay to ensure timestamp difference (SQLite uses second precision)
    std::thread::sleep(std::time::Duration::from_millis(1100));

    db.update_game_status(&game.id, GameStatus::Active)
        .expect("Failed to update game status");

    let updated_game = db.get_game(&game.id).expect("Failed to get updated game");

    assert!(
        updated_game.updated_at > pre_update_time,
        "Updated timestamp should increase after modification"
    );
    assert_eq!(
        updated_game.created_at, initial_time,
        "Created timestamp should never change"
    );
}
