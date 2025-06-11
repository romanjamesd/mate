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
        let unique_temp_dir = temp_dir.path().join(format!("test_{}", timestamp));
        std::fs::create_dir_all(&unique_temp_dir).expect("Failed to create unique test dir");
        
        // Override the database path for testing
        std::env::set_var("MATE_DATA_DIR", &unique_temp_dir);
        
        let db = Database::new("test_peer_12345678").expect("Failed to create test database");
        
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

/// Priority 1: Core Database Tests (4 tests)
/// Test fundamental database functionality

#[test]
fn test_database_initialization() {
    let (db, _env) = create_test_database();
    
    // Verify database can be created and initialized
    assert!(db.check_connection_health().is_ok(), "Database should be healthy after initialization");
    
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
    assert!(id1.contains("test_pee"), "Game ID should contain peer ID prefix");
    
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
    let _game = db.create_game(
        "opponent_peer_id".to_string(),
        PlayerColor::White,
        None,
    ).expect("Failed to create game");
    
    let (ops, _txn, err, _time) = db.get_connection_stats();
    assert!(ops > 0, "Operations count should increase after database operation");
    assert_eq!(err, 0, "Error count should remain 0 for successful operations");
}

#[test]
fn test_database_maintenance() {
    let (db, _env) = create_test_database();
    
    // Test maintenance operations
    let maintenance_result = db.perform_maintenance();
    assert!(maintenance_result.is_ok(), "Database maintenance should succeed");
    
    // Verify database is still healthy after maintenance
    assert!(db.check_connection_health().unwrap(), "Database should remain healthy after maintenance");
}

/// Priority 2: Game CRUD Operations (6 tests)
/// Test complete game lifecycle operations

#[test]
fn test_game_creation_and_retrieval() {
    let (db, _env) = create_test_database();
    
    // Test game creation
    let game = db.create_game(
        "opponent_peer_123".to_string(),
        PlayerColor::White,
        None,
    ).expect("Failed to create game");
    
    assert_eq!(game.opponent_peer_id, "opponent_peer_123");
    assert_eq!(game.my_color, PlayerColor::White);
    assert_eq!(game.status, GameStatus::Pending);
    assert!(game.created_at > 0, "Created timestamp should be set");
    assert_eq!(game.created_at, game.updated_at, "Initial timestamps should match");
    assert!(game.completed_at.is_none(), "Completed timestamp should be None for pending game");
    assert!(game.result.is_none(), "Result should be None for pending game");
    
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
    
    let game = db.create_game(
        "opponent_peer_456".to_string(),
        PlayerColor::Black,
        Some(metadata.clone()),
    ).expect("Failed to create game with metadata");
    
    assert_eq!(game.my_color, PlayerColor::Black);
    assert!(game.metadata.is_some(), "Metadata should be preserved");
    
    let retrieved_metadata = game.metadata.as_ref().unwrap();
    assert_eq!(retrieved_metadata["initial_fen"], metadata["initial_fen"]);
    assert_eq!(retrieved_metadata["rated"], metadata["rated"]);
}

#[test]
fn test_game_status_updates() {
    let (db, _env) = create_test_database();
    
    let game = db.create_game(
        "opponent_peer_789".to_string(),
        PlayerColor::White,
        None,
    ).expect("Failed to create game");
    
    let initial_updated_at = game.updated_at;
    
    // Add a delay to ensure timestamp difference (SQLite uses second precision)
    std::thread::sleep(std::time::Duration::from_millis(1100));
    
    // Test status update to active
    db.update_game_status(&game.id, GameStatus::Active)
        .expect("Failed to update game status");
    
    let updated_game = db.get_game(&game.id).expect("Failed to retrieve updated game");
    assert_eq!(updated_game.status, GameStatus::Active);
    assert!(updated_game.updated_at > initial_updated_at, "Updated timestamp should change");
    assert!(updated_game.completed_at.is_none(), "Completed timestamp should still be None");
    
    // Test status update to completed
    db.update_game_status(&game.id, GameStatus::Completed)
        .expect("Failed to update game status to completed");
    
    let completed_game = db.get_game(&game.id).expect("Failed to retrieve completed game");
    assert_eq!(completed_game.status, GameStatus::Completed);
    assert!(completed_game.completed_at.is_some(), "Completed timestamp should be set");
    assert!(completed_game.completed_at.unwrap() >= completed_game.updated_at, "Completed timestamp should be recent");
}

#[test]
fn test_game_result_updates() {
    let (db, _env) = create_test_database();
    
    let game = db.create_game(
        "opponent_peer_win".to_string(),
        PlayerColor::White,
        None,
    ).expect("Failed to create game");
    
    // Test setting game result
    db.update_game_result(&game.id, mate::storage::models::GameResult::Win)
        .expect("Failed to update game result");
    
    let updated_game = db.get_game(&game.id).expect("Failed to retrieve game with result");
    assert!(updated_game.result.is_some(), "Result should be set");
    assert_eq!(updated_game.result.unwrap(), mate::storage::models::GameResult::Win);
    assert_eq!(updated_game.status, GameStatus::Completed, "Status should be automatically set to completed");
    assert!(updated_game.completed_at.is_some(), "Completed timestamp should be set");
}

#[test]
fn test_games_query_operations() {
    let (db, _env) = create_test_database();
    
    // Create multiple games
    let game1 = db.create_game("opponent_1".to_string(), PlayerColor::White, None)
        .expect("Failed to create game 1");
    let game2 = db.create_game("opponent_2".to_string(), PlayerColor::Black, None)
        .expect("Failed to create game 2");
    let game3 = db.create_game("opponent_1".to_string(), PlayerColor::White, None)
        .expect("Failed to create game 3");
    
    // Update statuses for testing
    db.update_game_status(&game2.id, GameStatus::Active)
        .expect("Failed to update game 2 status");
    
    // Test get games with opponent
    let opponent_1_games = db.get_games_with_opponent("opponent_1")
        .expect("Failed to get games with opponent_1");
    assert_eq!(opponent_1_games.len(), 2, "Should find 2 games with opponent_1");
    assert!(opponent_1_games.iter().any(|g| g.id == game1.id));
    assert!(opponent_1_games.iter().any(|g| g.id == game3.id));
    
    // Test get games by status
    let pending_games = db.get_games_by_status(GameStatus::Pending)
        .expect("Failed to get pending games");
    assert!(pending_games.len() >= 2, "Should find at least 2 pending games");
    
    let active_games = db.get_games_by_status(GameStatus::Active)
        .expect("Failed to get active games");
    assert_eq!(active_games.len(), 1, "Should find 1 active game");
    assert_eq!(active_games[0].id, game2.id);
    
    // Test get recent games
    let recent_games = db.get_recent_games(10)
        .expect("Failed to get recent games");
    assert!(recent_games.len() >= 3, "Should find at least 3 recent games");
}

#[test]
fn test_game_deletion() {
    let (db, _env) = create_test_database();
    
    let game = db.create_game(
        "opponent_delete".to_string(),
        PlayerColor::White,
        None,
    ).expect("Failed to create game");
    
    // Verify game exists
    assert!(db.get_game(&game.id).is_ok(), "Game should exist before deletion");
    
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
    
    let game = db.create_game("opponent_msg".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");
    
    // Test message creation
    let message = db.store_message(
        game.id.clone(),
        "move".to_string(),
        r#"{"from": "e2", "to": "e4"}"#.to_string(),
        "signature_123".to_string(),
        "sender_peer_id".to_string(),
    ).expect("Failed to store message");
    
    assert!(message.id.is_some(), "Message ID should be set");
    assert_eq!(message.game_id, game.id);
    assert_eq!(message.message_type, "move");
    assert_eq!(message.sender_peer_id, "sender_peer_id");
    assert!(message.created_at > 0, "Created timestamp should be set");
    
    // Test message retrieval
    let retrieved_message = db.get_message(message.id.unwrap())
        .expect("Failed to retrieve message");
    assert_eq!(retrieved_message.game_id, message.game_id);
    assert_eq!(retrieved_message.message_type, message.message_type);
    assert_eq!(retrieved_message.content, message.content);
}

#[test]
fn test_game_message_operations() {
    let (db, _env) = create_test_database();
    
    let game = db.create_game("opponent_msgs".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");
    
    // Store multiple messages
    let _message1 = db.store_message(
        game.id.clone(),
        "move".to_string(),
        r#"{"from": "e2", "to": "e4"}"#.to_string(),
        "sig1".to_string(),
        "player1".to_string(),
    ).expect("Failed to store message 1");
    
    let _message2 = db.store_message(
        game.id.clone(),
        "move".to_string(),
        r#"{"from": "e7", "to": "e5"}"#.to_string(),
        "sig2".to_string(),
        "player2".to_string(),
    ).expect("Failed to store message 2");
    
    let _message3 = db.store_message(
        game.id.clone(),
        "chat".to_string(),
        r#"{"text": "Good game!"}"#.to_string(),
        "sig3".to_string(),
        "player1".to_string(),
    ).expect("Failed to store message 3");
    
    // Test get all messages for game
    let all_messages = db.get_messages_for_game(&game.id)
        .expect("Failed to get messages for game");
    assert_eq!(all_messages.len(), 3, "Should find 3 messages for game");
    
    // Verify chronological order
    assert!(all_messages[0].created_at <= all_messages[1].created_at);
    assert!(all_messages[1].created_at <= all_messages[2].created_at);
    
    // Test get messages by type
    let move_messages = db.get_messages_by_type(&game.id, "move")
        .expect("Failed to get move messages");
    assert_eq!(move_messages.len(), 2, "Should find 2 move messages");
    
    let chat_messages = db.get_messages_by_type(&game.id, "chat")
        .expect("Failed to get chat messages");
    assert_eq!(chat_messages.len(), 1, "Should find 1 chat message");
    
    // Test get messages from sender
    let player1_messages = db.get_messages_from_sender(&game.id, "player1")
        .expect("Failed to get player1 messages");
    assert_eq!(player1_messages.len(), 2, "Should find 2 messages from player1");
    
    // Test message count
    let message_count = db.count_messages_for_game(&game.id)
        .expect("Failed to count messages");
    assert_eq!(message_count, 3, "Should count 3 messages for game");
}

#[test]
fn test_message_pagination() {
    let (db, _env) = create_test_database();
    
    let game = db.create_game("opponent_page".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");
    
    // Store 5 messages
    for i in 0..5 {
        db.store_message(
            game.id.clone(),
            "move".to_string(),
            format!(r#"{{"move": {}}}"#, i),
            format!("sig_{}", i),
            "sender".to_string(),
        ).expect(&format!("Failed to store message {}", i));
    }
    
    // Test pagination
    let page1 = db.get_messages_for_game_paginated(&game.id, 2, 0)
        .expect("Failed to get first page");
    assert_eq!(page1.len(), 2, "First page should have 2 messages");
    
    let page2 = db.get_messages_for_game_paginated(&game.id, 2, 2)
        .expect("Failed to get second page");
    assert_eq!(page2.len(), 2, "Second page should have 2 messages");
    
    let page3 = db.get_messages_for_game_paginated(&game.id, 2, 4)
        .expect("Failed to get third page");
    assert_eq!(page3.len(), 1, "Third page should have 1 message");
    
    // Verify no overlap
    assert_ne!(page1[0].id, page2[0].id, "Pages should not overlap");
    assert_ne!(page2[0].id, page3[0].id, "Pages should not overlap");
}

#[test]
fn test_recent_messages_query() {
    let (db, _env) = create_test_database();
    
    let game1 = db.create_game("opponent_recent1".to_string(), PlayerColor::White, None)
        .expect("Failed to create game 1");
    let game2 = db.create_game("opponent_recent2".to_string(), PlayerColor::Black, None)
        .expect("Failed to create game 2");
    
    // Store messages in both games
    db.store_message(game1.id, "move".to_string(), "content1".to_string(), "sig1".to_string(), "sender1".to_string())
        .expect("Failed to store message in game 1");
    db.store_message(game2.id, "move".to_string(), "content2".to_string(), "sig2".to_string(), "sender2".to_string())
        .expect("Failed to store message in game 2");
    
    // Test recent messages query
    let recent_messages = db.get_recent_messages(10)
        .expect("Failed to get recent messages");
    assert!(recent_messages.len() >= 2, "Should find at least 2 recent messages");
    
    // Verify reverse chronological order (most recent first)
    if recent_messages.len() > 1 {
        assert!(recent_messages[0].created_at >= recent_messages[1].created_at, 
                "Recent messages should be in reverse chronological order");
    }
}

#[test]
fn test_message_deletion() {
    let (db, _env) = create_test_database();
    
    let game = db.create_game("opponent_del_msg".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");
    
    let message = db.store_message(
        game.id.clone(),
        "move".to_string(),
        "content".to_string(),
        "sig".to_string(),
        "sender".to_string(),
    ).expect("Failed to store message");
    
    let message_id = message.id.unwrap();
    
    // Verify message exists
    assert!(db.get_message(message_id).is_ok(), "Message should exist before deletion");
    
    // Delete message
    db.delete_message(message_id).expect("Failed to delete message");
    
    // Verify message is deleted
    assert!(db.get_message(message_id).is_err(), "Message should not exist after deletion");
    
    // Test delete all messages for game
    db.store_message(game.id.clone(), "move".to_string(), "content2".to_string(), "sig2".to_string(), "sender2".to_string())
        .expect("Failed to store second message");
    
    let deleted_count = db.delete_messages_for_game(&game.id)
        .expect("Failed to delete messages for game");
    assert_eq!(deleted_count, 1, "Should delete 1 message");
    
    let remaining_messages = db.get_messages_for_game(&game.id)
        .expect("Failed to get remaining messages");
    assert_eq!(remaining_messages.len(), 0, "No messages should remain after deletion");
}

/// Priority 4: Model Functionality Tests (3 tests)
/// Test enum and model functionality

#[test]
fn test_player_color_functionality() {
    // Test string conversion
    assert_eq!(PlayerColor::White.as_str(), "white");
    assert_eq!(PlayerColor::Black.as_str(), "black");
    
    // Test from string conversion
    assert_eq!(PlayerColor::from_str("white"), Some(PlayerColor::White));
    assert_eq!(PlayerColor::from_str("black"), Some(PlayerColor::Black));
    assert_eq!(PlayerColor::from_str("WHITE"), Some(PlayerColor::White));
    assert_eq!(PlayerColor::from_str("invalid"), None);
    
    // Test serialization round-trip
    let color = PlayerColor::White;
    let json = serde_json::to_string(&color).expect("Failed to serialize PlayerColor");
    let deserialized: PlayerColor = serde_json::from_str(&json).expect("Failed to deserialize PlayerColor");
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
    assert_eq!(GameStatus::from_str("pending"), Some(GameStatus::Pending));
    assert_eq!(GameStatus::from_str("ACTIVE"), Some(GameStatus::Active));
    assert_eq!(GameStatus::from_str("invalid"), None);
    
    // Test serialization round-trip
    let status = GameStatus::Active;
    let json = serde_json::to_string(&status).expect("Failed to serialize GameStatus");
    let deserialized: GameStatus = serde_json::from_str(&json).expect("Failed to deserialize GameStatus");
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
    assert_eq!(GameResult::from_str("win"), Some(GameResult::Win));
    assert_eq!(GameResult::from_str("DRAW"), Some(GameResult::Draw));
    assert_eq!(GameResult::from_str("invalid"), None);
    
    // Test serialization round-trip
    let result = GameResult::Win;
    let json = serde_json::to_string(&result).expect("Failed to serialize GameResult");
    let deserialized: GameResult = serde_json::from_str(&json).expect("Failed to deserialize GameResult");
    assert_eq!(result, deserialized);
} 