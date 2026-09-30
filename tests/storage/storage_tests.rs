use mate::storage::{Database, GameStatus, Message, PlayerColor, StorageError};
use tempfile::TempDir;

/// Per-test database isolation via an explicit path (not process-global env).
struct TestEnvironment {
    _temp_dir: TempDir,
    test_data_dir: std::path::PathBuf,
}

impl TestEnvironment {
    fn new() -> (Database, Self) {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let test_data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&test_data_dir).expect("Failed to create test data dir");

        let db_path = test_data_dir.join("database.sqlite");
        let db = Database::new_with_path("test_peer_12345678", &db_path)
            .expect("Failed to create test database");

        let env = TestEnvironment {
            _temp_dir: temp_dir,
            test_data_dir,
        };

        (db, env)
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        let db_path = self.test_data_dir.join("database.sqlite");
        let wal_path = db_path.with_extension("sqlite-wal");
        let shm_path = db_path.with_extension("sqlite-shm");

        let _ = std::fs::remove_file(&wal_path);
        let _ = std::fs::remove_file(&shm_path);
        let _ = std::fs::remove_file(&db_path);
    }
}

/// Test helper to create a temporary database
fn create_test_database() -> (Database, TestEnvironment) {
    TestEnvironment::new()
}

fn set_created_at(db: &Database, id: i64, ts: i64) {
    db.with_connection(|conn| {
        let updated = conn.execute(
            "UPDATE messages SET created_at = ?1 WHERE id = ?2",
            [ts, id],
        )?;
        assert_eq!(
            updated, 1,
            "Timestamp update must affect one stored message"
        );
        Ok(())
    })
    .expect("Failed to set created_at");
}

fn ids(messages: &[Message]) -> Vec<i64> {
    messages.iter().map(|m| m.id.expect("stored id")).collect()
}

fn assert_messages(actual: &[Message], expected: &[Message]) {
    assert_eq!(ids(actual), ids(expected));
    // Compare whole rows to check payloads and preserved timestamp metadata too.
    assert_eq!(
        serde_json::to_value(actual).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
}

fn store_order_messages(db: &Database, game_id: &str) -> Vec<Message> {
    [
        ("move", "player1"),
        ("chat", "player2"),
        ("move", "player2"),
        ("chat", "player1"),
        ("move", "player1"),
    ]
    .into_iter()
    .enumerate()
    .map(|(order, (message_type, sender))| {
        db.store_message(
            game_id.to_string(),
            message_type.to_string(),
            format!(r#"{{"order": {order}}}"#),
            "signature".to_string(),
            sender.to_string(),
        )
        .expect("Failed to store ordered message")
    })
    .collect()
}

/// Priority 1: Core Database Tests (4 tests)
/// Test fundamental database functionality

#[test]
fn test_database_initialization() {
    let (db, _env) = create_test_database();

    // Verify database can be created and initialized
    assert!(
        db.check_connection_health().is_ok(),
        "Database should be healthy after initialization"
    );

    // Verify connection stats are initialized
    let (ops, txn, err, _time) = db.get_connection_stats();
    assert_eq!(ops, 0, "Operations count should start at 0");
    assert_eq!(txn, 0, "Transaction count should start at 0");
    assert_eq!(err, 0, "Error count should start at 0");
}

#[test]
fn test_game_id_generation() {
    let (db, _env) = create_test_database();

    // Test basic game ID generation
    let id1 = db.generate_game_id();
    let id2 = db.generate_game_id();

    assert_ne!(id1, id2, "Generated game IDs should be unique");
    assert!(id1.len() > 10, "Game ID should be reasonably long");
    // Game IDs should contain the peer ID prefix
    assert!(
        id1.contains("test_peer"),
        "Game ID should contain peer ID prefix, got: {}",
        id1
    );

    // Test multiple generations for uniqueness
    let mut ids = std::collections::HashSet::new();
    for i in 0..10 {
        let id = db.generate_game_id();
        assert!(ids.insert(id), "Game ID {} should be unique", i);
    }
}

#[test]
fn test_connection_health_monitoring() {
    let (db, _env) = create_test_database();

    // Test connection health check
    let health_result = db.check_connection_health();
    assert!(health_result.is_ok(), "Health check should succeed");
    assert!(health_result.unwrap(), "Connection should be healthy");

    // Test connection stats tracking
    let _game = db
        .create_game("opponent_peer_id".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    let (ops, _txn, err, _time) = db.get_connection_stats();
    assert!(
        ops > 0,
        "Operations count should increase after database operation"
    );
    assert_eq!(
        err, 0,
        "Error count should remain 0 for successful operations"
    );
}

#[test]
fn test_database_maintenance() {
    let (db, _env) = create_test_database();

    // Test maintenance operations
    let maintenance_result = db.perform_maintenance();
    assert!(
        maintenance_result.is_ok(),
        "Database maintenance should succeed"
    );

    // Verify database is still healthy after maintenance
    assert!(
        db.check_connection_health().unwrap(),
        "Database should remain healthy after maintenance"
    );
}

/// Priority 2: Game CRUD Operations (6 tests)
/// Test complete game lifecycle operations

#[test]
fn test_game_creation_and_retrieval() {
    let (db, _env) = create_test_database();

    // Test game creation
    let game = db
        .create_game("opponent_peer_123".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    assert_eq!(game.opponent_peer_id, "opponent_peer_123");
    assert_eq!(game.my_color, PlayerColor::White);
    assert_eq!(game.status, GameStatus::Pending);
    assert!(game.created_at > 0, "Created timestamp should be set");
    assert_eq!(
        game.created_at, game.updated_at,
        "Initial timestamps should match"
    );
    assert!(
        game.completed_at.is_none(),
        "Completed timestamp should be None for pending game"
    );
    assert!(
        game.result.is_none(),
        "Result should be None for pending game"
    );

    // Test game retrieval
    let retrieved_game = db.get_game(&game.id).expect("Failed to retrieve game");
    assert_eq!(retrieved_game.id, game.id);
    assert_eq!(retrieved_game.opponent_peer_id, game.opponent_peer_id);
    assert_eq!(retrieved_game.my_color, game.my_color);
    assert_eq!(retrieved_game.status, game.status);
}

#[test]
fn test_game_creation_with_metadata() {
    let (db, _env) = create_test_database();

    let metadata = serde_json::json!({
        "initial_fen": "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "time_control": {
            "initial_time_ms": 300000,
            "increment_ms": 5000
        },
        "rated": true
    });

    let game = db
        .create_game(
            "opponent_peer_456".to_string(),
            PlayerColor::Black,
            Some(metadata.clone()),
        )
        .expect("Failed to create game with metadata");

    assert_eq!(game.my_color, PlayerColor::Black);
    assert!(game.metadata.is_some(), "Metadata should be preserved");

    let retrieved_metadata = game.metadata.as_ref().unwrap();
    assert_eq!(retrieved_metadata["initial_fen"], metadata["initial_fen"]);
    assert_eq!(retrieved_metadata["rated"], metadata["rated"]);
}

#[test]
fn test_create_game_with_caller_supplied_id() {
    let (db, _env) = create_test_database();

    let fixed_id = "inviter-supplied-game-id-001".to_string();
    let game = db
        .create_game_with_id(
            fixed_id.clone(),
            "opponent_peer_invitee".to_string(),
            PlayerColor::Black,
            None,
        )
        .expect("Failed to create game with caller-supplied ID");

    assert_eq!(game.id, fixed_id);
    assert_eq!(game.opponent_peer_id, "opponent_peer_invitee");
    assert_eq!(game.my_color, PlayerColor::Black);
    assert_eq!(game.status, GameStatus::Pending);

    let retrieved = db
        .get_game(&fixed_id)
        .expect("Failed to retrieve game by supplied ID");
    assert_eq!(retrieved.id, fixed_id);
    assert_eq!(retrieved.opponent_peer_id, game.opponent_peer_id);
    assert_eq!(retrieved.my_color, game.my_color);
}

#[test]
fn test_create_game_with_id_rejects_duplicate() {
    let (db, _env) = create_test_database();

    let fixed_id = "duplicate-game-id-001".to_string();
    db.create_game_with_id(
        fixed_id.clone(),
        "opponent_a".to_string(),
        PlayerColor::White,
        None,
    )
    .expect("First create with ID should succeed");

    let duplicate = db.create_game_with_id(
        fixed_id.clone(),
        "opponent_b".to_string(),
        PlayerColor::Black,
        None,
    );

    assert!(duplicate.is_err(), "Duplicate game ID should fail");
    match duplicate.unwrap_err() {
        StorageError::ConstraintViolation {
            table,
            column,
            constraint,
        } => {
            assert_eq!(table, "games");
            assert_eq!(column, "id");
            assert!(
                constraint.contains("UNIQUE")
                    || constraint.contains("duplicate")
                    || constraint.contains(&fixed_id),
                "Constraint message should mention duplicate/UNIQUE, got: {constraint}"
            );
        }
        other => panic!("Expected ConstraintViolation, got: {other:?}"),
    }

    // Original row unchanged
    let existing = db.get_game(&fixed_id).expect("Original game should remain");
    assert_eq!(existing.opponent_peer_id, "opponent_a");
    assert_eq!(existing.my_color, PlayerColor::White);
}

#[test]
fn test_create_game_with_id_rejects_empty_id() {
    let (db, _env) = create_test_database();

    let result = db.create_game_with_id(
        String::new(),
        "opponent".to_string(),
        PlayerColor::White,
        None,
    );

    assert!(result.is_err(), "Empty game ID should fail");
    match result.unwrap_err() {
        StorageError::InvalidData { field, .. } => {
            assert_eq!(field, "game.id");
        }
        other => panic!("Expected InvalidData, got: {other:?}"),
    }
}

#[test]
fn test_update_game_color() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game_with_id(
            "color-update-game".to_string(),
            "opponent_color".to_string(),
            PlayerColor::White,
            None,
        )
        .expect("Failed to create game");

    let initial_updated_at = game.updated_at;
    std::thread::sleep(std::time::Duration::from_millis(1100));

    db.update_game_color(&game.id, PlayerColor::Black)
        .expect("Failed to update game color");

    let updated = db
        .get_game(&game.id)
        .expect("Failed to retrieve game after color update");
    assert_eq!(updated.my_color, PlayerColor::Black);
    assert!(
        updated.updated_at > initial_updated_at,
        "Updated timestamp should change after color update"
    );
}

#[test]
fn test_update_opponent_peer_id() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game_with_id(
            "peer-id-update-game".to_string(),
            "original-peer".to_string(),
            PlayerColor::White,
            Some(serde_json::json!({ "dial_address": "127.0.0.1:8080" })),
        )
        .expect("Failed to create game");

    assert_eq!(game.opponent_peer_id, "original-peer");
    assert!(db.update_opponent_peer_id(&game.id, "").is_err());
    assert_eq!(
        db.get_game(&game.id).unwrap().opponent_peer_id,
        "original-peer"
    );
    let initial_updated_at = game.updated_at;
    std::thread::sleep(std::time::Duration::from_millis(1100));

    db.update_opponent_peer_id(&game.id, "remote-peer-abc")
        .expect("Failed to update opponent peer id");

    let updated = db
        .get_game(&game.id)
        .expect("Failed to retrieve game after peer id update");
    assert_eq!(updated.opponent_peer_id, "remote-peer-abc");
    assert_eq!(
        updated.metadata,
        Some(serde_json::json!({ "dial_address": "127.0.0.1:8080" }))
    );
    assert!(
        updated.updated_at > initial_updated_at,
        "Updated timestamp should change after peer id update"
    );

    let missing = db.update_opponent_peer_id("nonexistent", "peer");
    assert!(missing.is_err(), "Missing game should fail peer id update");
}

#[test]
fn test_game_status_updates() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("opponent_peer_789".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    let initial_updated_at = game.updated_at;

    // Add a delay to ensure timestamp difference (SQLite uses second precision)
    std::thread::sleep(std::time::Duration::from_millis(1100));

    // Test status update to active
    db.update_game_status(&game.id, GameStatus::Active)
        .expect("Failed to update game status");

    let updated_game = db
        .get_game(&game.id)
        .expect("Failed to retrieve updated game");
    assert_eq!(updated_game.status, GameStatus::Active);
    assert!(
        updated_game.updated_at > initial_updated_at,
        "Updated timestamp should change"
    );
    assert!(
        updated_game.completed_at.is_none(),
        "Completed timestamp should still be None"
    );

    // Test status update to completed
    db.update_game_status(&game.id, GameStatus::Completed)
        .expect("Failed to update game status to completed");

    let completed_game = db
        .get_game(&game.id)
        .expect("Failed to retrieve completed game");
    assert_eq!(completed_game.status, GameStatus::Completed);
    assert!(
        completed_game.completed_at.is_some(),
        "Completed timestamp should be set"
    );
    assert!(
        completed_game.completed_at.unwrap() >= completed_game.updated_at,
        "Completed timestamp should be recent"
    );
}

#[test]
fn test_game_result_updates() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("opponent_peer_win".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    // Test setting game result
    db.update_game_result(&game.id, mate::storage::models::GameResult::Win)
        .expect("Failed to update game result");

    let updated_game = db
        .get_game(&game.id)
        .expect("Failed to retrieve game with result");
    assert!(updated_game.result.is_some(), "Result should be set");
    assert_eq!(
        updated_game.result.unwrap(),
        mate::storage::models::GameResult::Win
    );
    assert_eq!(
        updated_game.status,
        GameStatus::Completed,
        "Status should be automatically set to completed"
    );
    assert!(
        updated_game.completed_at.is_some(),
        "Completed timestamp should be set"
    );
}

#[test]
fn test_games_query_operations() {
    let (db, _env) = create_test_database();

    // Create multiple games
    let game1 = db
        .create_game("opponent_1".to_string(), PlayerColor::White, None)
        .expect("Failed to create game 1");
    let game2 = db
        .create_game("opponent_2".to_string(), PlayerColor::Black, None)
        .expect("Failed to create game 2");
    let game3 = db
        .create_game("opponent_1".to_string(), PlayerColor::White, None)
        .expect("Failed to create game 3");

    // Update statuses for testing
    db.update_game_status(&game2.id, GameStatus::Active)
        .expect("Failed to update game 2 status");

    // Test get games with opponent
    let opponent_1_games = db
        .get_games_with_opponent("opponent_1")
        .expect("Failed to get games with opponent_1");
    assert_eq!(
        opponent_1_games.len(),
        2,
        "Should find 2 games with opponent_1"
    );
    assert!(opponent_1_games.iter().any(|g| g.id == game1.id));
    assert!(opponent_1_games.iter().any(|g| g.id == game3.id));

    // Test get games by status
    let pending_games = db
        .get_games_by_status(GameStatus::Pending)
        .expect("Failed to get pending games");
    assert!(
        pending_games.len() >= 2,
        "Should find at least 2 pending games"
    );

    let active_games = db
        .get_games_by_status(GameStatus::Active)
        .expect("Failed to get active games");
    assert_eq!(active_games.len(), 1, "Should find 1 active game");
    assert_eq!(active_games[0].id, game2.id);

    // Test get recent games
    let recent_games = db.get_recent_games(10).expect("Failed to get recent games");
    assert!(
        recent_games.len() >= 3,
        "Should find at least 3 recent games"
    );
}

#[test]
fn test_game_deletion() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("opponent_delete".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    // Verify game exists
    assert!(
        db.get_game(&game.id).is_ok(),
        "Game should exist before deletion"
    );

    // Delete game
    db.delete_game(&game.id).expect("Failed to delete game");

    // Verify game is deleted
    let result = db.get_game(&game.id);
    assert!(result.is_err(), "Game should not exist after deletion");
}

/// Priority 3: Message Operations (5 tests)
/// Test message storage and retrieval functionality

#[test]
fn test_message_creation_and_retrieval() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("opponent_msg".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    // Test message creation
    let message = db
        .store_message(
            game.id.clone(),
            "move".to_string(),
            r#"{"from": "e2", "to": "e4"}"#.to_string(),
            "signature_123".to_string(),
            "sender_peer_id".to_string(),
        )
        .expect("Failed to store message");

    assert!(message.id.is_some(), "Message ID should be set");
    assert_eq!(message.game_id, game.id);
    assert_eq!(message.message_type, "move");
    assert_eq!(message.sender_peer_id, "sender_peer_id");
    assert!(message.created_at > 0, "Created timestamp should be set");

    // Test message retrieval
    let retrieved_message = db
        .get_message(message.id.unwrap())
        .expect("Failed to retrieve message");
    assert_eq!(retrieved_message.game_id, message.game_id);
    assert_eq!(retrieved_message.message_type, message.message_type);
    assert_eq!(retrieved_message.content, message.content);
}

#[test]
fn test_game_message_operations() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("opponent_msgs".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    // Store multiple messages
    let message1 = db
        .store_message(
            game.id.clone(),
            "move".to_string(),
            r#"{"from": "e2", "to": "e4"}"#.to_string(),
            "sig1".to_string(),
            "player1".to_string(),
        )
        .expect("Failed to store message 1");

    let message2 = db
        .store_message(
            game.id.clone(),
            "move".to_string(),
            r#"{"from": "e7", "to": "e5"}"#.to_string(),
            "sig2".to_string(),
            "player2".to_string(),
        )
        .expect("Failed to store message 2");

    let message3 = db
        .store_message(
            game.id.clone(),
            "chat".to_string(),
            r#"{"text": "Good game!"}"#.to_string(),
            "sig3".to_string(),
            "player1".to_string(),
        )
        .expect("Failed to store message 3");

    // Test get all messages for game
    let all_messages = db
        .get_messages_for_game(&game.id)
        .expect("Failed to get messages for game");
    assert_eq!(all_messages.len(), 3, "Should find 3 messages for game");

    // Verify local insertion order
    assert_eq!(ids(&all_messages), ids(&[message1, message2, message3]));

    // Test get messages by type
    let move_messages = db
        .get_messages_by_type(&game.id, "move")
        .expect("Failed to get move messages");
    assert_eq!(move_messages.len(), 2, "Should find 2 move messages");

    let chat_messages = db
        .get_messages_by_type(&game.id, "chat")
        .expect("Failed to get chat messages");
    assert_eq!(chat_messages.len(), 1, "Should find 1 chat message");

    // Test get messages from sender
    let player1_messages = db
        .get_messages_from_sender(&game.id, "player1")
        .expect("Failed to get player1 messages");
    assert_eq!(
        player1_messages.len(),
        2,
        "Should find 2 messages from player1"
    );

    // Test message count
    let message_count = db
        .count_messages_for_game(&game.id)
        .expect("Failed to count messages");
    assert_eq!(message_count, 3, "Should count 3 messages for game");
}

#[test]
fn messages_with_equal_timestamps_return_in_insertion_order() {
    let (db, _env) = create_test_database();
    let game = db
        .create_game("opponent_equal".to_string(), PlayerColor::White, None)
        .unwrap();
    let mut stored = store_order_messages(&db, &game.id);
    for message in &mut stored {
        set_created_at(&db, message.id.unwrap(), 100);
        message.created_at = 100;
    }

    assert!(ids(&stored).windows(2).all(|pair| pair[0] < pair[1]));
    assert_messages(&db.get_messages_for_game(&game.id).unwrap(), &stored);
    for message_type in ["move", "chat"] {
        let expected: Vec<_> = stored
            .iter()
            .filter(|m| m.message_type == message_type)
            .cloned()
            .collect();
        assert_messages(
            &db.get_messages_by_type(&game.id, message_type).unwrap(),
            &expected,
        );
    }
    for sender in ["player1", "player2"] {
        let expected: Vec<_> = stored
            .iter()
            .filter(|m| m.sender_peer_id == sender)
            .cloned()
            .collect();
        assert_messages(
            &db.get_messages_from_sender(&game.id, sender).unwrap(),
            &expected,
        );
    }
}

#[test]
fn decreasing_timestamps_do_not_change_order() {
    let (db, _env) = create_test_database();
    let game = db
        .create_game("opponent_rollback".to_string(), PlayerColor::White, None)
        .unwrap();
    let mut stored = store_order_messages(&db, &game.id);
    let total = stored.len();
    for (order, message) in stored.iter_mut().enumerate() {
        let timestamp = (total - order) as i64;
        set_created_at(&db, message.id.unwrap(), timestamp);
        message.created_at = timestamp;
    }

    assert_messages(&db.get_messages_for_game(&game.id).unwrap(), &stored);
    for offset in (0..total as u32).step_by(2) {
        let end = (offset as usize + 2).min(total);
        assert_messages(
            &db.get_messages_for_game_paginated(&game.id, 2, offset)
                .unwrap(),
            &stored[offset as usize..end],
        );
    }
    for message_type in ["move", "chat"] {
        let expected: Vec<_> = stored
            .iter()
            .filter(|m| m.message_type == message_type)
            .cloned()
            .collect();
        assert_messages(
            &db.get_messages_by_type(&game.id, message_type).unwrap(),
            &expected,
        );
    }
    for sender in ["player1", "player2"] {
        let expected: Vec<_> = stored
            .iter()
            .filter(|m| m.sender_peer_id == sender)
            .cloned()
            .collect();
        assert_messages(
            &db.get_messages_from_sender(&game.id, sender).unwrap(),
            &expected,
        );
    }
    let expected: Vec<_> = stored.into_iter().rev().collect();
    assert_messages(&db.get_recent_messages(10).unwrap(), &expected);
}

#[test]
fn recent_messages_newest_first_by_id() {
    let (db, _env) = create_test_database();
    let game1 = db
        .create_game("opponent_recent1".to_string(), PlayerColor::White, None)
        .unwrap();
    let game2 = db
        .create_game("opponent_recent2".to_string(), PlayerColor::Black, None)
        .unwrap();
    let mut stored = Vec::new();
    for order in 0..7 {
        let game_id = if order % 2 == 0 { &game1.id } else { &game2.id };
        let mut message = db
            .store_message(
                game_id.clone(),
                "move".to_string(),
                format!(r#"{{"order": {order}}}"#),
                "signature".to_string(),
                "sender".to_string(),
            )
            .unwrap();
        let timestamp = 7 - order;
        set_created_at(&db, message.id.unwrap(), timestamp);
        message.created_at = timestamp;
        stored.push(message);
    }
    for limit in [0, 3, 10] {
        let expected: Vec<_> = stored.iter().rev().take(limit as usize).cloned().collect();
        assert_messages(&db.get_recent_messages(limit).unwrap(), &expected);
    }
}

#[test]
fn recent_messages_order_survives_delete_of_newest() {
    let (db, _env) = create_test_database();
    let game = db
        .create_game(
            "opponent_delete_newest".to_string(),
            PlayerColor::White,
            None,
        )
        .unwrap();
    let store = |content: &str| {
        db.store_message(
            game.id.clone(),
            "chat".to_string(),
            content.to_string(),
            "signature".to_string(),
            "sender".to_string(),
        )
        .unwrap()
    };
    let a = store("A");
    let b = store("B");
    let c = store("C");
    let deleted_id = c.id.unwrap();
    db.delete_message(deleted_id).unwrap();
    let d = store("D");

    assert!(
        d.id.unwrap() > deleted_id,
        "Deleted maximum ID must not be reused"
    );
    assert_messages(&db.get_recent_messages(10).unwrap(), &[d, b, a]);
}

#[test]
fn test_message_pagination() {
    let (db, _env) = create_test_database();
    let game = db
        .create_game("opponent_page".to_string(), PlayerColor::White, None)
        .unwrap();
    let mut stored: Vec<_> = (0..7)
        .map(|order| {
            db.store_message(
                game.id.clone(),
                "move".to_string(),
                format!(r#"{{"order": {order}}}"#),
                format!("sig_{order}"),
                "sender".to_string(),
            )
            .unwrap()
        })
        .collect();

    for equal_timestamps in [false, true] {
        if equal_timestamps {
            for message in &mut stored {
                set_created_at(&db, message.id.unwrap(), 100);
                message.created_at = 100;
            }
        }
        let all = db.get_messages_for_game(&game.id).unwrap();
        assert_messages(&all, &stored);
        let total = stored.len() as u32;
        for page_size in [1, 2, 3, 7, 10] {
            let mut paged = Vec::new();
            let mut reached_end = false;
            // Include the first offset beyond the data, with a finite bound.
            for page_index in 0..=total.div_ceil(page_size) {
                let offset = page_index * page_size;
                let page = db
                    .get_messages_for_game_paginated(&game.id, page_size, offset)
                    .unwrap();
                let start = (offset as usize).min(stored.len());
                let end = (start + page_size as usize).min(stored.len());
                assert_messages(&page, &stored[start..end]);
                if page.is_empty() {
                    reached_end = true;
                    break;
                }
                paged.extend(page);
            }
            assert!(reached_end, "Pagination must finish with an empty page");
            assert_eq!(ids(&paged), ids(&all));
            assert_messages(&paged, &stored);
            for offset in [total, total + 1] {
                assert!(db
                    .get_messages_for_game_paginated(&game.id, page_size, offset)
                    .unwrap()
                    .is_empty());
            }
        }
        for offset in [0, total, total + 1] {
            assert!(db
                .get_messages_for_game_paginated(&game.id, 0, offset)
                .unwrap()
                .is_empty());
        }
    }
}

#[test]
fn test_recent_messages_query() {
    let (db, _env) = create_test_database();
    let game1 = db
        .create_game("opponent_recent1".to_string(), PlayerColor::White, None)
        .unwrap();
    let game2 = db
        .create_game("opponent_recent2".to_string(), PlayerColor::Black, None)
        .unwrap();
    let message1 = db
        .store_message(
            game1.id,
            "move".to_string(),
            "content1".to_string(),
            "sig1".to_string(),
            "sender1".to_string(),
        )
        .unwrap();
    let message2 = db
        .store_message(
            game2.id,
            "move".to_string(),
            "content2".to_string(),
            "sig2".to_string(),
            "sender2".to_string(),
        )
        .unwrap();
    let recent = db.get_recent_messages(10).unwrap();
    assert_eq!(recent.len(), 2);
    assert!(recent[0].id.unwrap() > recent[1].id.unwrap());
    assert_messages(&recent, &[message2, message1]);
}

#[test]
fn test_message_deletion() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("opponent_del_msg".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    let message = db
        .store_message(
            game.id.clone(),
            "move".to_string(),
            "content".to_string(),
            "sig".to_string(),
            "sender".to_string(),
        )
        .expect("Failed to store message");

    let message_id = message.id.unwrap();

    // Verify message exists
    assert!(
        db.get_message(message_id).is_ok(),
        "Message should exist before deletion"
    );

    // Delete message
    db.delete_message(message_id)
        .expect("Failed to delete message");

    // Verify message is deleted
    assert!(
        db.get_message(message_id).is_err(),
        "Message should not exist after deletion"
    );

    // Test delete all messages for game
    db.store_message(
        game.id.clone(),
        "move".to_string(),
        "content2".to_string(),
        "sig2".to_string(),
        "sender2".to_string(),
    )
    .expect("Failed to store second message");

    let deleted_count = db
        .delete_messages_for_game(&game.id)
        .expect("Failed to delete messages for game");
    assert_eq!(deleted_count, 1, "Should delete 1 message");

    let remaining_messages = db
        .get_messages_for_game(&game.id)
        .expect("Failed to get remaining messages");
    assert_eq!(
        remaining_messages.len(),
        0,
        "No messages should remain after deletion"
    );
}

/// Priority 4: Model Functionality Tests (3 tests)
/// Test enum and model functionality

#[test]
fn test_player_color_functionality() {
    // Test string conversion
    assert_eq!(PlayerColor::White.as_str(), "white");
    assert_eq!(PlayerColor::Black.as_str(), "black");

    // Test from string conversion
    assert_eq!("white".parse::<PlayerColor>(), Ok(PlayerColor::White));
    assert_eq!("black".parse::<PlayerColor>(), Ok(PlayerColor::Black));
    assert_eq!("WHITE".parse::<PlayerColor>(), Ok(PlayerColor::White));
    assert!("invalid".parse::<PlayerColor>().is_err());

    // Test serialization round-trip
    let color = PlayerColor::White;
    let json = serde_json::to_string(&color).expect("Failed to serialize PlayerColor");
    let deserialized: PlayerColor =
        serde_json::from_str(&json).expect("Failed to deserialize PlayerColor");
    assert_eq!(color, deserialized);
}

#[test]
fn test_game_status_functionality() {
    // Test string conversion
    assert_eq!(GameStatus::Pending.as_str(), "pending");
    assert_eq!(GameStatus::Active.as_str(), "active");
    assert_eq!(GameStatus::Completed.as_str(), "completed");
    assert_eq!(GameStatus::Abandoned.as_str(), "abandoned");

    // Test from string conversion
    assert_eq!("pending".parse::<GameStatus>(), Ok(GameStatus::Pending));
    assert_eq!("ACTIVE".parse::<GameStatus>(), Ok(GameStatus::Active));
    assert!("invalid".parse::<GameStatus>().is_err());

    // Test serialization round-trip
    let status = GameStatus::Active;
    let json = serde_json::to_string(&status).expect("Failed to serialize GameStatus");
    let deserialized: GameStatus =
        serde_json::from_str(&json).expect("Failed to deserialize GameStatus");
    assert_eq!(status, deserialized);
}

#[test]
fn test_game_result_functionality() {
    use mate::storage::models::GameResult;

    // Test string conversion
    assert_eq!(GameResult::Win.as_str(), "win");
    assert_eq!(GameResult::Loss.as_str(), "loss");
    assert_eq!(GameResult::Draw.as_str(), "draw");
    assert_eq!(GameResult::Abandoned.as_str(), "abandoned");

    // Test from string conversion
    assert_eq!("win".parse::<GameResult>(), Ok(GameResult::Win));
    assert_eq!("DRAW".parse::<GameResult>(), Ok(GameResult::Draw));
    assert!("invalid".parse::<GameResult>().is_err());

    // Test serialization round-trip
    let result = GameResult::Win;
    let json = serde_json::to_string(&result).expect("Failed to serialize GameResult");
    let deserialized: GameResult =
        serde_json::from_str(&json).expect("Failed to deserialize GameResult");
    assert_eq!(result, deserialized);
}
