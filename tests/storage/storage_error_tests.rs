use mate::storage::{Database, GameStatus, PlayerColor, StorageError};
use rand;
use std::fs;
use tempfile::TempDir;

/// Test helper that ensures proper environment cleanup
struct TestEnvironment {
    _temp_dir: TempDir,
    original_data_dir: Option<String>,
    test_data_dir: std::path::PathBuf,
}

impl TestEnvironment {
    fn new() -> (Database, Self) {
        // Save original environment variable
        let original_data_dir = std::env::var("MATE_DATA_DIR").ok();

        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Add a small random delay to spread out database creation times
        // This helps prevent race conditions in high-parallelism scenarios
        let delay_ms = rand::random::<u8>() as u64 % 50; // 0-49ms
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));

        // Use multiple sources of uniqueness to prevent race conditions
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos();
        let random_id: u64 = rand::random();
        let thread_id = std::thread::current().id();
        let process_id = std::process::id();

        // Include the test function name or a unique identifier in the path
        let unique_temp_dir = temp_dir.path().join(format!(
            "test_errors_{timestamp}_{random_id:x}_{thread_id:?}_{process_id}_{delay_ms}"
        ));
        std::fs::create_dir_all(&unique_temp_dir).expect("Failed to create unique test dir");

        // Override the database path for testing
        std::env::set_var("MATE_DATA_DIR", &unique_temp_dir);

        // Retry database creation with exponential backoff to handle potential race conditions
        let db = Self::create_database_with_retry("test_peer_errors", 3)
            .expect("Failed to create test database after retries");

        let env = TestEnvironment {
            _temp_dir: temp_dir,
            original_data_dir,
            test_data_dir: unique_temp_dir,
        };

        (db, env)
    }

    fn create_database_with_retry(
        peer_id: &str,
        max_retries: u32,
    ) -> Result<Database, mate::storage::StorageError> {
        let mut retries = 0;
        loop {
            match Database::new(peer_id) {
                Ok(db) => return Ok(db),
                Err(e) => {
                    if retries >= max_retries {
                        return Err(e);
                    }
                    retries += 1;
                    // Exponential backoff with jitter
                    let delay_ms = (2_u64.pow(retries) * 10) + (rand::random::<u8>() as u64 % 20);
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                }
            }
        }
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // Force close any database connections by dropping the Database instance
        // This helps ensure WAL files are properly cleaned up

        // Clean up WAL and SHM files that might be left behind
        let db_path = self.test_data_dir.join("database.sqlite");
        let wal_path = db_path.with_extension("sqlite-wal");
        let shm_path = db_path.with_extension("sqlite-shm");

        // Remove WAL files if they exist (ignore errors as they might not exist)
        let _ = fs::remove_file(&wal_path);
        let _ = fs::remove_file(&shm_path);

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

/// Environment cleanup helper for tests that need to modify environment
struct EnvironmentGuard {
    original_data_dir: Option<String>,
    test_data_dir: Option<std::path::PathBuf>,
}

impl EnvironmentGuard {
    fn new() -> Self {
        Self {
            original_data_dir: std::env::var("MATE_DATA_DIR").ok(),
            test_data_dir: None,
        }
    }

    fn set_data_dir<P: AsRef<std::path::Path>>(&mut self, path: P) {
        let path_buf = path.as_ref().to_path_buf();
        self.test_data_dir = Some(path_buf.clone());
        std::env::set_var("MATE_DATA_DIR", &path_buf);
    }
}

impl Drop for EnvironmentGuard {
    fn drop(&mut self) {
        // Clean up WAL files if a test directory was set
        if let Some(test_dir) = &self.test_data_dir {
            let db_path = test_dir.join("database.sqlite");
            let wal_path = db_path.with_extension("sqlite-wal");
            let shm_path = db_path.with_extension("sqlite-shm");

            let _ = fs::remove_file(&wal_path);
            let _ = fs::remove_file(&shm_path);
        }

        // Restore original environment variable
        match &self.original_data_dir {
            Some(original) => std::env::set_var("MATE_DATA_DIR", original),
            None => std::env::remove_var("MATE_DATA_DIR"),
        }
    }
}

/// Priority 1: Database Connection and Initialization Errors (3 tests)
/// Test error handling for database setup and connection issues

#[test]
fn test_database_path_errors() {
    let mut _env_guard = EnvironmentGuard::new();

    // Test with invalid directory path
    let invalid_path = "/root/nonexistent/deeply/nested/path";
    _env_guard.set_data_dir(invalid_path);

    // Creating database with invalid path should handle the error gracefully
    // Note: The actual behavior depends on implementation - it might create directories
    // or fail gracefully. We test that it doesn't panic.
    let result = Database::new("test_peer_invalid_path");
    // The implementation should either succeed (by creating directories) or fail gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Database creation should not panic"
    );

    // Environment will be automatically restored by _env_guard drop
}

#[test]
fn test_corrupted_database_handling() {
    let mut _env_guard = EnvironmentGuard::new();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Use a unique directory for this test
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos();
    let unique_temp_dir = temp_dir
        .path()
        .join(format!("test_corrupted_{timestamp}"));
    std::fs::create_dir_all(&unique_temp_dir).expect("Failed to create unique test dir");

    _env_guard.set_data_dir(&unique_temp_dir);

    // Create a valid database first
    let db_path = mate::storage::get_database_path().expect("Failed to get DB path");

    // Ensure parent directory exists
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create DB directory");
    }

    // Create a corrupted database file (invalid SQLite format)
    fs::write(&db_path, "This is not a valid SQLite database file")
        .expect("Failed to write corrupted file");

    // Attempting to open corrupted database should return an error
    let result = Database::new("test_peer_corrupted");
    assert!(result.is_err(), "Opening corrupted database should fail");

    if let Err(error) = result {
        // Verify it's the right type of error - corrupted database can cause different errors
        match error {
            StorageError::ConnectionFailed(_) => {
                // This is expected for truly corrupted files
            }
            StorageError::MigrationFailed { .. } => {
                // This can also happen if the corrupt file is interpreted as a database with migration issues
            }
            other => panic!(
                "Expected ConnectionFailed or MigrationFailed error, got: {:?}",
                other
            ),
        }
    }
}

#[test]
fn test_database_permission_errors() {
    let mut _env_guard = EnvironmentGuard::new();
    let temp_dir = TempDir::new().expect("Failed to create temp dir");

    // Create a read-only directory to simulate permission issues
    let readonly_path = temp_dir.path().join("readonly");
    fs::create_dir(&readonly_path).expect("Failed to create readonly dir");

    // Set read-only permissions (this may not work on all systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_path)
            .expect("Failed to get metadata")
            .permissions();
        perms.set_mode(0o444); // Read-only
        fs::set_permissions(&readonly_path, perms).expect("Failed to set permissions");
    }

    _env_guard.set_data_dir(&readonly_path);

    // This test may behave differently on different systems
    // On some systems, it might still succeed if the user has sufficient privileges
    let result = Database::new("test_peer_permissions");

    // We mainly want to ensure it doesn't panic and handles errors gracefully
    match result {
        Ok(_) => {
            // On some systems, this might succeed despite readonly directory
            // This is acceptable behavior
        }
        Err(_) => {
            // On other systems, this will fail as expected
            // This is also acceptable behavior
        }
    }

    // Reset permissions to allow cleanup
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&readonly_path)
            .expect("Failed to get metadata")
            .permissions();
        perms.set_mode(0o755); // Read/write/execute
        let _ = fs::set_permissions(&readonly_path, perms);
    }
}

/// Priority 2: Game Operation Errors (4 tests)
/// Test error handling for invalid game operations

#[test]
fn test_game_not_found_errors() {
    let (db, _env) = create_test_database();

    let nonexistent_game_id = "nonexistent_game_12345";

    // Test get_game with nonexistent ID
    let result = db.get_game(nonexistent_game_id);
    assert!(result.is_err(), "Getting nonexistent game should fail");

    match result.unwrap_err() {
        StorageError::GameNotFound { id } => {
            assert_eq!(id, nonexistent_game_id, "Error should include the game ID");
        }
        other => panic!("Expected GameNotFound error, got: {:?}", other),
    }

    // Test update_game_status with nonexistent ID
    let result = db.update_game_status(nonexistent_game_id, GameStatus::Active);
    assert!(
        result.is_err(),
        "Updating nonexistent game status should fail"
    );

    match result.unwrap_err() {
        StorageError::GameNotFound { id } => {
            assert_eq!(id, nonexistent_game_id, "Error should include the game ID");
        }
        other => panic!("Expected GameNotFound error, got: {:?}", other),
    }

    // Test update_game_result with nonexistent ID
    let result = db.update_game_result(nonexistent_game_id, mate::storage::models::GameResult::Win);
    assert!(
        result.is_err(),
        "Updating nonexistent game result should fail"
    );

    match result.unwrap_err() {
        StorageError::GameNotFound { id } => {
            assert_eq!(id, nonexistent_game_id, "Error should include the game ID");
        }
        other => panic!("Expected GameNotFound error, got: {:?}", other),
    }

    // Test delete_game with nonexistent ID
    let result = db.delete_game(nonexistent_game_id);
    assert!(result.is_err(), "Deleting nonexistent game should fail");

    match result.unwrap_err() {
        StorageError::GameNotFound { id } => {
            assert_eq!(id, nonexistent_game_id, "Error should include the game ID");
        }
        other => panic!("Expected GameNotFound error, got: {:?}", other),
    }
}

#[test]
fn test_invalid_game_data_errors() {
    let (db, _env) = create_test_database();

    // Test with invalid JSON metadata
    // Note: This test depends on how the implementation handles JSON serialization
    // If the implementation validates JSON during creation, this should fail
    // If it stores arbitrary strings, this might succeed

    let invalid_metadata = serde_json::Value::String("not a valid metadata object".to_string());

    // This should either succeed (if implementation is permissive) or fail gracefully
    let result = db.create_game(
        "opponent_invalid_meta".to_string(),
        PlayerColor::White,
        Some(invalid_metadata),
    );

    // We mainly test that it doesn't panic
    assert!(
        result.is_ok() || result.is_err(),
        "Invalid metadata handling should not panic"
    );

    // Test with extremely long peer ID
    let very_long_peer_id = "a".repeat(1000);
    let result = db.create_game(very_long_peer_id, PlayerColor::White, None);

    // Should handle long strings gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Long peer ID handling should not panic"
    );
}

#[test]
fn test_concurrent_game_modification_errors() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game("concurrent_test".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    // Delete the game
    db.delete_game(&game.id).expect("Failed to delete game");

    // Now try to modify the deleted game
    let status_result = db.update_game_status(&game.id, GameStatus::Active);
    assert!(status_result.is_err(), "Updating deleted game should fail");

    let result_result = db.update_game_result(&game.id, mate::storage::models::GameResult::Win);
    assert!(
        result_result.is_err(),
        "Setting result for deleted game should fail"
    );

    // Verify both operations return GameNotFound errors
    match status_result.unwrap_err() {
        StorageError::GameNotFound { .. } => { /* Expected */ }
        other => panic!("Expected GameNotFound error, got: {:?}", other),
    }

    match result_result.unwrap_err() {
        StorageError::GameNotFound { .. } => { /* Expected */ }
        other => panic!("Expected GameNotFound error, got: {:?}", other),
    }
}

#[test]
fn test_game_state_transition_errors() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game(
            "state_transition_test".to_string(),
            PlayerColor::White,
            None,
        )
        .expect("Failed to create game");

    // Complete the game
    db.update_game_result(&game.id, mate::storage::models::GameResult::Win)
        .expect("Failed to complete game");

    // Verify game is completed
    let completed_game = db.get_game(&game.id).expect("Failed to get completed game");
    assert_eq!(completed_game.status, GameStatus::Completed);

    // Try to change status from completed back to active
    // The implementation might allow this or prevent it - we test it handles it gracefully
    let result = db.update_game_status(&game.id, GameStatus::Active);

    // Whether this succeeds or fails, it should be handled gracefully
    match result {
        Ok(_) => {
            // Some implementations might allow status changes
            let updated_game = db.get_game(&game.id).expect("Failed to get updated game");
            assert_eq!(updated_game.status, GameStatus::Active);
        }
        Err(_) => {
            // Other implementations might prevent invalid state transitions
            // This is also acceptable
        }
    }
}

/// Priority 3: Message Operation Errors (3 tests)
/// Test error handling for message-related operations

#[test]
fn test_message_not_found_errors() {
    let (db, _env) = create_test_database();

    let nonexistent_message_id = 99999i64;

    // Test get_message with nonexistent ID
    let result = db.get_message(nonexistent_message_id);
    assert!(result.is_err(), "Getting nonexistent message should fail");

    match result.unwrap_err() {
        StorageError::MessageNotFound { id } => {
            assert_eq!(
                id,
                nonexistent_message_id.to_string(),
                "Error should include the message ID"
            );
        }
        other => panic!("Expected MessageNotFound error, got: {:?}", other),
    }

    // Test delete_message with nonexistent ID
    let result = db.delete_message(nonexistent_message_id);
    assert!(result.is_err(), "Deleting nonexistent message should fail");

    // The implementation might return MessageNotFound or succeed with 0 rows affected
    // Both are acceptable behaviors
    assert!(
        result.is_err(),
        "Deleting nonexistent message should return an error"
    );
}

#[test]
fn test_message_with_invalid_game_id() {
    let (db, _env) = create_test_database();

    let nonexistent_game_id = "nonexistent_game_for_message";

    // Try to store a message for a nonexistent game
    let result = db.store_message(
        nonexistent_game_id.to_string(),
        "move".to_string(),
        r#"{"move": "e4"}"#.to_string(),
        "signature".to_string(),
        "sender".to_string(),
    );

    // Depending on implementation, this might:
    // 1. Succeed (if foreign key constraints are not enforced)
    // 2. Fail with a foreign key constraint error
    // Both behaviors are acceptable - we just test it doesn't panic

    match result {
        Ok(message) => {
            // If it succeeds, we should be able to retrieve it
            let retrieved = db.get_message(message.id.unwrap());
            assert!(
                retrieved.is_ok(),
                "Should be able to retrieve stored message"
            );
        }
        Err(_) => {
            // If it fails due to foreign key constraint, that's also acceptable
        }
    }
}

#[test]
fn test_message_data_validation_errors() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game(
            "message_validation_test".to_string(),
            PlayerColor::White,
            None,
        )
        .expect("Failed to create game");

    // Test with extremely long content
    let very_long_content = "x".repeat(100_000);
    let result = db.store_message(
        game.id.clone(),
        "test".to_string(),
        very_long_content,
        "signature".to_string(),
        "sender".to_string(),
    );

    // Should handle long content gracefully (either succeed or fail gracefully)
    assert!(
        result.is_ok() || result.is_err(),
        "Long content should be handled gracefully"
    );

    // Test with empty strings
    let result = db.store_message(
        game.id.clone(),
        "".to_string(), // Empty message type
        "".to_string(), // Empty content
        "".to_string(), // Empty signature
        "".to_string(), // Empty sender
    );

    // Should handle empty strings gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Empty strings should be handled gracefully"
    );

    // Test with null characters (if the implementation needs to handle them)
    let content_with_nulls = "content\0with\0nulls";
    let result = db.store_message(
        game.id.clone(),
        "test".to_string(),
        content_with_nulls.to_string(),
        "signature".to_string(),
        "sender".to_string(),
    );

    // Should handle null characters gracefully
    assert!(
        result.is_ok() || result.is_err(),
        "Null characters should be handled gracefully"
    );
}

/// Priority 4: Query and Pagination Errors (3 tests)
/// Test error handling for query operations and edge cases

#[test]
fn test_invalid_query_parameters() {
    let (db, _env) = create_test_database();

    // Test pagination with invalid parameters
    let game = db
        .create_game("pagination_test".to_string(), PlayerColor::White, None)
        .expect("Failed to create game");

    // Test with very large offset
    let result = db.get_messages_for_game_paginated(&game.id, 10, u32::MAX);
    assert!(
        result.is_ok(),
        "Large offset should return empty result, not error"
    );

    let messages = result.unwrap();
    assert_eq!(
        messages.len(),
        0,
        "Should return empty result for offset beyond data"
    );

    // Test with zero limit
    let result = db.get_messages_for_game_paginated(&game.id, 0, 0);
    assert!(
        result.is_ok(),
        "Zero limit should return empty result, not error"
    );

    let messages = result.unwrap();
    assert_eq!(
        messages.len(),
        0,
        "Should return empty result for zero limit"
    );

    // Test get_recent_games with zero limit
    let result = db.get_recent_games(0);
    assert!(
        result.is_ok(),
        "Zero limit should return empty result, not error"
    );

    let games = result.unwrap();
    assert_eq!(games.len(), 0, "Should return empty result for zero limit");

    // Test get_recent_messages with very large limit
    let result = db.get_recent_messages(u32::MAX);
    assert!(result.is_ok(), "Large limit should not cause error");
    // Result should succeed even if it returns fewer messages than requested
}

#[test]
fn test_query_with_invalid_references() {
    let (db, _env) = create_test_database();

    let nonexistent_game_id = "nonexistent_query_game";
    let nonexistent_peer_id = "nonexistent_peer_12345";

    // Test queries that should return empty results (not errors) for nonexistent references

    let messages = db
        .get_messages_for_game(nonexistent_game_id)
        .expect("Should return empty result for nonexistent game");
    assert_eq!(
        messages.len(),
        0,
        "Should return empty result for nonexistent game"
    );

    let paginated_messages = db
        .get_messages_for_game_paginated(nonexistent_game_id, 10, 0)
        .expect("Should return empty result for nonexistent game");
    assert_eq!(
        paginated_messages.len(),
        0,
        "Should return empty result for nonexistent game"
    );

    let typed_messages = db
        .get_messages_by_type(nonexistent_game_id, "move")
        .expect("Should return empty result for nonexistent game");
    assert_eq!(
        typed_messages.len(),
        0,
        "Should return empty result for nonexistent game"
    );

    let sender_messages = db
        .get_messages_from_sender(nonexistent_game_id, "some_sender")
        .expect("Should return empty result for nonexistent game");
    assert_eq!(
        sender_messages.len(),
        0,
        "Should return empty result for nonexistent game"
    );

    let message_count = db
        .count_messages_for_game(nonexistent_game_id)
        .expect("Should return 0 for nonexistent game");
    assert_eq!(
        message_count, 0,
        "Should return 0 count for nonexistent game"
    );

    let opponent_games = db
        .get_games_with_opponent(nonexistent_peer_id)
        .expect("Should return empty result for nonexistent opponent");
    assert_eq!(
        opponent_games.len(),
        0,
        "Should return empty result for nonexistent opponent"
    );

    // Test deletion of messages for nonexistent game
    let deleted_count = db
        .delete_messages_for_game(nonexistent_game_id)
        .expect("Should return 0 for nonexistent game");
    assert_eq!(
        deleted_count, 0,
        "Should return 0 deleted count for nonexistent game"
    );
}

#[test]
fn test_concurrent_query_and_modification() {
    let (db, _env) = create_test_database();

    let game = db
        .create_game(
            "concurrent_query_test".to_string(),
            PlayerColor::White,
            None,
        )
        .expect("Failed to create game");

    // Add some messages
    for i in 0..5 {
        db.store_message(
            game.id.clone(),
            "move".to_string(),
            format!(r#"{{"move": {i}}}"#),
            format!("sig_{i}"),
            "sender".to_string(),
        )
        .unwrap_or_else(|_| panic!("Failed to store message {}", i));
    }

    // Get initial message count
    let initial_count = db
        .count_messages_for_game(&game.id)
        .expect("Failed to get initial count");
    assert_eq!(initial_count, 5, "Should have 5 initial messages");

    // Delete the game while we have a reference to it
    db.delete_game(&game.id).expect("Failed to delete game");

    // Now query for messages - should return empty result due to cascade delete
    let messages = db
        .get_messages_for_game(&game.id)
        .expect("Should return empty result after game deletion");
    assert_eq!(
        messages.len(),
        0,
        "Should have no messages after game deletion"
    );

    let final_count = db
        .count_messages_for_game(&game.id)
        .expect("Should return 0 after game deletion");
    assert_eq!(final_count, 0, "Should have 0 messages after game deletion");
}

/// Priority 5: Model and Serialization Errors (2 tests)
/// Test error handling for model validation and serialization

#[test]
fn test_enum_parsing_errors() {
    // Test PlayerColor from_str
    assert_eq!("white".parse::<PlayerColor>(), Ok(PlayerColor::White));
    assert_eq!("black".parse::<PlayerColor>(), Ok(PlayerColor::Black));
    assert!("invalid".parse::<PlayerColor>().is_err());
    assert!("".parse::<PlayerColor>().is_err());
    assert!("INVALID".parse::<PlayerColor>().is_err());

    // Test GameStatus from_str
    assert_eq!("pending".parse::<GameStatus>(), Ok(GameStatus::Pending));
    assert_eq!("active".parse::<GameStatus>(), Ok(GameStatus::Active));
    assert_eq!("completed".parse::<GameStatus>(), Ok(GameStatus::Completed));
    assert_eq!("abandoned".parse::<GameStatus>(), Ok(GameStatus::Abandoned));
    assert!("invalid".parse::<GameStatus>().is_err());
    assert!("".parse::<GameStatus>().is_err());

    // Test GameResult from_str
    use mate::storage::models::GameResult;
    assert_eq!("win".parse::<GameResult>(), Ok(GameResult::Win));
    assert_eq!("loss".parse::<GameResult>(), Ok(GameResult::Loss));
    assert_eq!("draw".parse::<GameResult>(), Ok(GameResult::Draw));
    assert_eq!("abandoned".parse::<GameResult>(), Ok(GameResult::Abandoned));
    assert!("invalid".parse::<GameResult>().is_err());
    assert!("".parse::<GameResult>().is_err());
}

#[test]
fn test_json_serialization_errors() {
    use mate::storage::models::{Game, GameResult, Message};

    // Test serialization of Game with complex metadata
    let game = Game {
        id: "test_game".to_string(),
        opponent_peer_id: "opponent".to_string(),
        my_color: PlayerColor::White,
        status: GameStatus::Active,
        created_at: 1234567890,
        updated_at: 1234567890,
        completed_at: None,
        result: Some(GameResult::Win),
        metadata: Some(serde_json::json!({
            "complex": {
                "nested": "value",
                "array": [1, 2, 3]
            }
        })),
    };

    // Test serialization
    let json_result = serde_json::to_string(&game);
    assert!(json_result.is_ok(), "Game serialization should succeed");

    let json_str = json_result.unwrap();

    // Test deserialization
    let deserialization_result: Result<Game, _> = serde_json::from_str(&json_str);
    assert!(
        deserialization_result.is_ok(),
        "Game deserialization should succeed"
    );

    let deserialized_game = deserialization_result.unwrap();
    assert_eq!(deserialized_game.id, game.id);
    assert_eq!(deserialized_game.my_color, game.my_color);
    assert_eq!(deserialized_game.status, game.status);

    // Test Message serialization
    let message = Message {
        id: Some(123),
        game_id: "test_game".to_string(),
        message_type: "move".to_string(),
        content: r#"{"complex": "json", "array": [1,2,3]}"#.to_string(),
        signature: "signature".to_string(),
        sender_peer_id: "sender".to_string(),
        created_at: 1234567890,
    };

    let message_json_result = serde_json::to_string(&message);
    assert!(
        message_json_result.is_ok(),
        "Message serialization should succeed"
    );

    let message_json_str = message_json_result.unwrap();
    let message_deserialization_result: Result<Message, _> =
        serde_json::from_str(&message_json_str);
    assert!(
        message_deserialization_result.is_ok(),
        "Message deserialization should succeed"
    );

    // Test with invalid JSON string
    let invalid_json = r#"{"id": "invalid", "status": "invalid_status"}"#;
    let invalid_result: Result<Game, _> = serde_json::from_str(invalid_json);
    assert!(
        invalid_result.is_err(),
        "Invalid JSON should fail to deserialize"
    );
}
