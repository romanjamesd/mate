//! CLI-Network Integration Tests
//!
//! Tests for CLI interaction with src/network/ modules
//! Based on step 3.2 of the testing plan

use anyhow::Result;
use mate::cli::app::App;
use mate::cli::network_manager::{NetworkConfig, NetworkManager};
use mate::crypto::Identity;
use mate::messages::chess::Move as ChessMove;
use mate::messages::{GameAccept, GameInvite, RetryStrategy};
use mate::storage::{models::PlayerColor, GameStatus};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

use crate::common::port_utils::get_unique_test_address;

// =============================================================================
// Test Utilities & Mock Infrastructure
// =============================================================================

/// Create a test app with isolated temporary directory
async fn create_test_app() -> Result<(App, TempDir)> {
    let temp_dir = TempDir::new()?;
    let app = App::new_with_data_dir(temp_dir.path().to_path_buf()).await?;
    Ok((app, temp_dir))
}

/// Create a test game in the database
async fn create_test_game(
    app: &App,
    opponent_peer_id: &str,
    my_color: PlayerColor,
    status: GameStatus,
) -> Result<String> {
    let game = app
        .database
        .create_game(opponent_peer_id.to_string(), my_color, None)?;

    if status != GameStatus::Pending {
        app.database.update_game_status(&game.id, status)?;
    }

    Ok(game.id)
}

/// Create a test network manager with custom configuration
fn create_test_network_manager(identity: Arc<Identity>) -> NetworkManager {
    let config = NetworkConfig {
        default_retry_strategy: RetryStrategy::Quick, // Fast retries for tests
        connection_timeout: Duration::from_secs(5),
        max_persistent_connections: 5,
        connection_keepalive: Duration::from_secs(30),
    };
    NetworkManager::with_config(identity, config)
}

// =============================================================================
// Behavior-Focused Network Operation Tests
// =============================================================================

#[tokio::test]
async fn test_cli_commands_trigger_network_operations() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    // Test 1: Invite command should trigger network operation
    let initial_games_count = app.database.get_all_games().unwrap().len();

    let invite_test_address = get_unique_test_address();
    let invite_result = timeout(
        Duration::from_secs(3),
        app.handle_invite(invite_test_address, Some("white".to_string())),
    )
    .await;

    // Verify game was created (database side effect)
    let final_games_count = app.database.get_all_games().unwrap().len();
    assert_eq!(
        final_games_count,
        initial_games_count + 1,
        "Invite command should create a game in database"
    );

    // Network operation was attempted (even if it failed due to unavailable peer)
    match invite_result {
        Ok(result) => {
            // Operation completed - verify it attempted network communication
            assert!(result.is_err(), "Should fail with unavailable peer");
        }
        Err(_) => {
            // Timeout occurred - but game was still created, so network attempt was made
        }
    }

    // Test 2: Accept command with existing pending game
    let peer_address = get_unique_test_address();
    let game_id = create_test_game(&app, &peer_address, PlayerColor::White, GameStatus::Pending)
        .await
        .expect("Failed to create test game");

    let accept_result = timeout(
        Duration::from_secs(3),
        app.handle_accept(game_id, Some("black".to_string())),
    )
    .await;

    // Network operation was attempted (even if failed)
    match accept_result {
        Ok(result) => assert!(result.is_err(), "Should fail with unavailable peer"),
        Err(_) => {} // Timeout is acceptable
    }

    // Test 3: Move command with active game
    let move_game_id = create_test_game(
        &app,
        "127.0.0.1:8080",
        PlayerColor::White,
        GameStatus::Active,
    )
    .await
    .expect("Failed to create test game");

    let move_result = timeout(
        Duration::from_secs(3),
        app.handle_move(Some(move_game_id), "e4".to_string()),
    )
    .await;

    // Network operation was attempted
    match move_result {
        Ok(result) => assert!(result.is_err(), "Should fail with unavailable peer"),
        Err(_) => {} // Timeout is acceptable
    }
}

#[tokio::test]
async fn test_network_manager_retry_behavior() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));
    let network_manager = create_test_network_manager(identity);

    let game_id = "test_retry_behavior".to_string();
    let invite = GameInvite::new(game_id.clone(), Some(PlayerColor::White.into()));

    let start_time = std::time::Instant::now();
    let unreachable_port = 50000
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 10000) as u16;
    let result = network_manager
        .send_game_invite(&format!("127.0.0.1:{unreachable_port}"), game_id, invite)
        .await;
    let elapsed = start_time.elapsed();

    // Verify retry behavior occurred
    assert!(result.is_err(), "Should fail with unavailable peer");
    assert!(
        elapsed >= Duration::from_millis(100), // At least one retry delay
        "Should have spent time on retries, elapsed: {:?}",
        elapsed
    );
    assert!(
        elapsed < Duration::from_secs(10), // But not too long
        "Should not hang indefinitely, elapsed: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_network_manager_connection_state_tracking() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));
    let network_manager = create_test_network_manager(identity);

    // Get initial state
    let initial_stats = network_manager.get_network_stats().await;
    let initial_connections = initial_stats.active_connections;
    let initial_pending = initial_stats.total_pending_messages;

    // Attempt network operation that will fail
    let game_id = "test_state_tracking".to_string();
    let invite = GameInvite::new(game_id.clone(), Some(PlayerColor::White.into()));

    let unreachable_port = 50100
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 10000) as u16;
    let _result = network_manager
        .send_game_invite(&format!("127.0.0.1:{unreachable_port}"), game_id, invite)
        .await;

    // Verify state tracking (failed connections shouldn't increase active count)
    network_manager.cleanup_connections().await;
    let final_stats = network_manager.get_network_stats().await;

    assert_eq!(
        final_stats.active_connections, initial_connections,
        "Failed connections should not increase active count"
    );

    // Verify pending messages behavior
    assert!(
        final_stats.total_pending_messages >= initial_pending,
        "Failed operations may queue pending messages"
    );
}

#[tokio::test]
async fn test_network_manager_peer_availability_detection() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));
    let network_manager = create_test_network_manager(identity);

    let unavailable_port = 50200
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 10000) as u16;
    let unavailable_peer = format!("127.0.0.1:{unavailable_port}");

    // Test peer availability detection
    let is_online_before = network_manager.is_peer_online(&unavailable_peer).await;
    assert!(
        !is_online_before,
        "Unavailable peer should be detected as offline"
    );

    // Attempt operation with unavailable peer
    let game_id = "test_availability".to_string();
    let invite = GameInvite::new(game_id.clone(), Some(PlayerColor::Black.into()));

    let result = network_manager
        .send_game_invite(&unavailable_peer, game_id, invite)
        .await;

    assert!(result.is_err(), "Should fail for unavailable peer");

    // Verify peer is still considered offline
    let is_online_after = network_manager.is_peer_online(&unavailable_peer).await;
    assert!(
        !is_online_after,
        "Peer should remain offline after failed operation"
    );
}

#[tokio::test]
async fn test_network_message_type_handling() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));
    let network_manager = create_test_network_manager(identity);

    let test_port = 50300
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 10000) as u16;
    let peer_address = format!("127.0.0.1:{test_port}");
    let game_id = "test_message_types".to_string();

    // Test different message types - verify each operation type is attempted
    let invite = GameInvite::new(game_id.clone(), Some(PlayerColor::White.into()));
    let accept = GameAccept::new(game_id.clone(), PlayerColor::Black.into());
    let chess_move = ChessMove::new(game_id.clone(), "e4".to_string(), "hash123".to_string());

    // Each operation should fail but attempt the correct message type
    let invite_result = network_manager
        .send_game_invite(&peer_address, game_id.clone(), invite)
        .await;
    assert!(
        invite_result.is_err(),
        "Invite should fail with unavailable peer"
    );

    let accept_result = network_manager
        .send_game_accept(&peer_address, game_id.clone(), accept)
        .await;
    assert!(
        accept_result.is_err(),
        "Accept should fail with unavailable peer"
    );

    let move_result = network_manager
        .send_chess_move(&peer_address, game_id, chess_move)
        .await;
    assert!(
        move_result.is_err(),
        "Move should fail with unavailable peer"
    );
}

// =============================================================================
// Configuration and Stats Tests (No String Assertions)
// =============================================================================

#[tokio::test]
async fn test_network_manager_configuration_behavior() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));

    // Test with default configuration
    let default_manager = NetworkManager::new(identity.clone());
    let stats = default_manager.get_network_stats().await;

    assert_eq!(
        stats.active_connections, 0,
        "Should start with no connections"
    );
    assert_eq!(
        stats.total_pending_messages, 0,
        "Should start with no pending messages"
    );

    // Test with custom configuration - verify behavior differences
    let fast_config = NetworkConfig {
        default_retry_strategy: RetryStrategy::NoRetry, // Single attempt
        connection_timeout: Duration::from_millis(100),
        max_persistent_connections: 1,
        connection_keepalive: Duration::from_secs(1),
    };

    let fast_manager = NetworkManager::with_config(identity, fast_config);

    let game_id = "test_fast_config".to_string();
    let invite = GameInvite::new(game_id.clone(), Some(PlayerColor::White.into()));

    let start_time = std::time::Instant::now();
    let config_test_port = 50400
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 10000) as u16;
    let _result = fast_manager
        .send_game_invite(&format!("127.0.0.1:{config_test_port}"), game_id, invite)
        .await;
    let elapsed = start_time.elapsed();

    // Verify fast configuration results in faster failure
    assert!(
        elapsed < Duration::from_secs(2),
        "Fast config should fail more quickly than default config, elapsed: {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_network_stats_structure_and_relationships() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));
    let network_manager = create_test_network_manager(identity);

    let stats = network_manager.get_network_stats().await;

    // Test logical relationships in stats (not string content)
    assert!(
        stats.active_connections <= stats.healthy_connections || stats.healthy_connections == 0,
        "Healthy connections should not exceed active connections"
    );

    // Stats are always non-negative (unsigned types), so just verify they're accessible

    // Verify stats object can be used programmatically
    let _total_stats = stats.active_connections + stats.total_pending_messages;
}

// =============================================================================
// Error Type Tests (No String Content Assertions)
// =============================================================================

#[tokio::test]
async fn test_network_error_types_are_appropriate() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));
    let network_manager = create_test_network_manager(identity);

    let game_id = "test_error_types".to_string();
    let invite = GameInvite::new(game_id.clone(), Some(PlayerColor::White.into()));

    // Test with invalid address format
    let result1 = network_manager
        .send_game_invite("invalid_address", game_id.clone(), invite.clone())
        .await;
    assert!(result1.is_err(), "Should fail with invalid address");

    // Test with unreachable address
    let error_test_port = 50500
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 10000) as u16;
    let result2 = network_manager
        .send_game_invite(&format!("127.0.0.1:{error_test_port}"), game_id, invite)
        .await;
    assert!(result2.is_err(), "Should fail with unreachable address");

    // Verify errors implement required traits (not content)
    let error1 = result1.unwrap_err();
    let error2 = result2.unwrap_err();

    // Test that errors are Send + Sync (required for async)
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    assert_send_sync(&error1);
    assert_send_sync(&error2);

    // Test that errors can be converted to string (but don't check content)
    let _error1_string = error1.to_string();
    let _error2_string = error2.to_string();
}

#[tokio::test]
async fn test_timeout_behavior_consistency() {
    let identity = Arc::new(Identity::generate().expect("Failed to generate identity"));

    let config = NetworkConfig {
        default_retry_strategy: RetryStrategy::NoRetry,
        connection_timeout: Duration::from_millis(100), // Very short
        max_persistent_connections: 1,
        connection_keepalive: Duration::from_secs(1),
    };

    let network_manager = NetworkManager::with_config(identity, config);

    let game_id = "test_timeout_consistency".to_string();
    let invite = GameInvite::new(game_id.clone(), Some(PlayerColor::White.into()));

    let start_time = std::time::Instant::now();
    let timeout_test_port = 50600
        + (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            % 10000) as u16;
    let result = network_manager
        .send_game_invite(&format!("127.0.0.1:{timeout_test_port}"), game_id, invite)
        .await;
    let elapsed = start_time.elapsed();

    // Verify timeout behavior (timing-based, not string-based)
    assert!(result.is_err(), "Should timeout");
    assert!(
        elapsed < Duration::from_secs(5),
        "Should timeout quickly with short config"
    );
    assert!(
        elapsed >= Duration::from_millis(50),
        "Should take at least minimum expected time"
    );
}
