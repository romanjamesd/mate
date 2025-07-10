//! CLI Command Handler Integration Tests
//!
//! Tests for CLI command handlers in src/cli/app.rs based on actual implementation

use anyhow::Result;
use mate::cli::app::App;
use mate::storage::models::{GameStatus, PlayerColor};
use tempfile::TempDir;

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

// =============================================================================
// Games Command Tests
// =============================================================================

#[tokio::test]
async fn test_games_empty_database_shows_no_games_message() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let result = app.handle_games().await;

    assert!(
        result.is_ok(),
        "handle_games should succeed with empty database"
    );
}

#[tokio::test]
async fn test_games_single_game_formatted_display() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let _game_id = create_test_game(
        &app,
        "test_opponent",
        PlayerColor::White,
        GameStatus::Active,
    )
    .await
    .expect("Failed to create test game");

    let result = app.handle_games().await;

    assert!(
        result.is_ok(),
        "handle_games should succeed with single game"
    );
}

#[tokio::test]
async fn test_games_multiple_games_ordered_by_recent_activity() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let _pending_game =
        create_test_game(&app, "opponent1", PlayerColor::White, GameStatus::Pending)
            .await
            .expect("Failed to create pending game");

    let _active_game = create_test_game(&app, "opponent2", PlayerColor::Black, GameStatus::Active)
        .await
        .expect("Failed to create active game");

    let result = app.handle_games().await;

    assert!(
        result.is_ok(),
        "handle_games should succeed with multiple games"
    );

    let games = app.database.get_all_games().expect("Failed to get games");
    assert_eq!(games.len(), 2, "Should have 2 games in database");
}

// =============================================================================
// Board Command Tests
// =============================================================================

#[tokio::test]
async fn test_board_no_game_id_finds_most_recent() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let _game_id = create_test_game(
        &app,
        "test_opponent",
        PlayerColor::White,
        GameStatus::Active,
    )
    .await
    .expect("Failed to create test game");

    let result = app.handle_board(None).await;

    assert!(
        result.is_ok(),
        "handle_board should find active game when no ID specified"
    );
}

#[tokio::test]
async fn test_board_specific_game_id_displays_correct_state() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let game_id = create_test_game(
        &app,
        "test_opponent",
        PlayerColor::Black,
        GameStatus::Active,
    )
    .await
    .expect("Failed to create test game");

    let result = app.handle_board(Some(game_id)).await;

    assert!(
        result.is_ok(),
        "handle_board should work with specific game ID"
    );
}

#[tokio::test]
async fn test_board_invalid_game_id_error_handling() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let result = app
        .handle_board(Some("nonexistent_game_id".to_string()))
        .await;

    assert!(
        result.is_err(),
        "handle_board should fail with nonexistent game ID"
    );
}

// =============================================================================
// Move Command Tests
// =============================================================================

#[tokio::test]
async fn test_move_no_active_games_error_handling() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let _game_id = create_test_game(
        &app,
        "test_opponent",
        PlayerColor::White,
        GameStatus::Completed,
    )
    .await
    .expect("Failed to create test game");

    let result = app.handle_move(None, "e4".to_string()).await;

    assert!(result.is_err(), "Should fail when no active games found");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("No active games found"),
        "Error should indicate no active games: {}",
        error_msg
    );
}

#[tokio::test]
async fn test_move_invalid_game_states_error_handling() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let game_id = create_test_game(
        &app,
        "test_opponent",
        PlayerColor::White,
        GameStatus::Pending,
    )
    .await
    .expect("Failed to create test game");

    let result = app.handle_move(Some(game_id), "e4".to_string()).await;

    assert!(result.is_err(), "Should fail when game is not active");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("not active"),
        "Error should indicate game is not active: {}",
        error_msg
    );
}

#[tokio::test]
async fn test_move_empty_move_notation_error_handling() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let game_id = create_test_game(
        &app,
        "test_opponent",
        PlayerColor::White,
        GameStatus::Active,
    )
    .await
    .expect("Failed to create test game");

    let result = app.handle_move(Some(game_id), "".to_string()).await;

    assert!(result.is_err(), "Should fail with empty move");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("cannot be empty"),
        "Error should indicate empty move: {}",
        error_msg
    );
}

// =============================================================================
// Accept Command Tests
// =============================================================================

#[tokio::test]
async fn test_accept_nonexistent_game_error_handling() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let result = app
        .handle_accept("nonexistent_game_id".to_string(), None)
        .await;

    assert!(result.is_err(), "Should fail with nonexistent game ID");
    assert!(
        result.unwrap_err().to_string().contains("Game not found"),
        "Error should indicate game not found"
    );
}

#[tokio::test]
async fn test_accept_non_pending_game_error_handling() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let game_id = create_test_game(
        &app,
        "test_opponent",
        PlayerColor::White,
        GameStatus::Active,
    )
    .await
    .expect("Failed to create test game");

    let result = app.handle_accept(game_id, None).await;

    assert!(result.is_err(), "Should fail when game is not pending");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("not in pending status"),
        "Error should indicate game is not pending: {}",
        error_msg
    );
}

// =============================================================================
// History Command Tests
// =============================================================================

#[tokio::test]
async fn test_history_no_games_shows_helpful_message() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let result = app.handle_history(None).await;

    assert!(
        result.is_ok(),
        "handle_history should handle empty database gracefully"
    );
}

#[tokio::test]
async fn test_history_nonexistent_game_error_handling() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let result = app
        .handle_history(Some("nonexistent_game_id".to_string()))
        .await;

    assert!(
        result.is_err(),
        "handle_history should fail with nonexistent game ID"
    );
}

// =============================================================================
// Invite Command Tests
// =============================================================================

#[tokio::test]
async fn test_invite_invalid_color_input_rejection() {
    let (app, _temp_dir) = create_test_app().await.expect("Failed to create test app");

    let result = app
        .handle_invite("127.0.0.1:8080".to_string(), Some("invalid".to_string()))
        .await;

    assert!(result.is_err(), "Should fail with invalid color");
    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("Invalid color"),
        "Error should indicate invalid color: {}",
        error_msg
    );
}
