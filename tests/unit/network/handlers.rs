//! Unit tests for server message dispatch and GameInvite handling

use mate::chess::{Board, Color};
use mate::messages::chess::{generate_game_id, hash_board_state, GameInvite};
use mate::messages::Message;
use mate::network::{dispatch, HandlerError};
use mate::storage::models::{GameStatus, PlayerColor};
use mate::Database;
use std::sync::Arc;

fn test_db() -> Arc<Database> {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let db_path = temp_dir.path().join("handlers.sqlite");
    let database =
        Database::new_with_path("test-peer-handlers", &db_path).expect("create test database");
    // Keep DB files alive for the duration of each test process section.
    std::mem::forget(temp_dir);
    Arc::new(database)
}

#[test]
fn dispatch_ping_echoes() {
    let db = test_db();
    let ping = Message::new_ping(42, "hello".to_string());

    let result = dispatch(db.as_ref(), "peer-a", ping).expect("dispatch should succeed");
    let response = result.expect("Ping should produce a reply");

    assert!(response.is_ping());
    assert_eq!(response.get_nonce(), 42);
    assert_eq!(response.get_payload(), "hello");
}

#[test]
fn dispatch_pong_returns_no_reply() {
    let db = test_db();
    let pong = Message::new_pong(7, "unexpected".to_string());

    let result = dispatch(db.as_ref(), "peer-a", pong).expect("dispatch should succeed");
    assert!(result.is_none(), "unexpected Pong should not produce a reply");
}

#[test]
fn dispatch_valid_game_invite_echoes_and_persists_pending() {
    let db = test_db();
    let game_id = generate_game_id();
    let invite = Message::new_game_invite(game_id.clone(), Some(Color::White));

    let result = dispatch(db.as_ref(), "peer-a", invite).expect("dispatch should succeed");
    let response = result.expect("GameInvite should produce a reply");

    match response {
        Message::GameInvite(echo) => {
            assert_eq!(echo.game_id, game_id);
            assert_eq!(echo.suggested_color, Some(Color::White));
        }
        other => panic!("expected GameInvite echo, got {other:?}"),
    }

    let game = db.get_game(&game_id).expect("pending game should exist");
    assert_eq!(game.id, game_id);
    assert_eq!(game.opponent_peer_id, "peer-a");
    assert_eq!(game.my_color, PlayerColor::White);
    assert_eq!(game.status, GameStatus::Pending);

    let messages = db.get_messages_for_game(&game_id).expect("messages");
    let invite_msg = messages
        .iter()
        .find(|m| m.message_type == "GameInvite")
        .expect("GameInvite message row");
    assert_eq!(invite_msg.sender_peer_id, "peer-a");
    let parsed: GameInvite =
        serde_json::from_str(&invite_msg.content).expect("parse stored invite");
    assert_eq!(parsed.game_id, game_id);
    assert_eq!(parsed.suggested_color, Some(Color::White));
}

#[test]
fn dispatch_game_invite_color_conventions() {
    let db = test_db();

    let white_id = generate_game_id();
    dispatch(
        db.as_ref(),
        "peer-a",
        Message::new_game_invite(white_id.clone(), Some(Color::White)),
    )
    .expect("dispatch")
    .expect("reply");
    assert_eq!(
        db.get_game(&white_id).unwrap().my_color,
        PlayerColor::White
    );

    let black_id = generate_game_id();
    dispatch(
        db.as_ref(),
        "peer-a",
        Message::new_game_invite(black_id.clone(), Some(Color::Black)),
    )
    .expect("dispatch")
    .expect("reply");
    assert_eq!(
        db.get_game(&black_id).unwrap().my_color,
        PlayerColor::Black
    );

    let none_id = generate_game_id();
    dispatch(
        db.as_ref(),
        "peer-a",
        Message::new_game_invite(none_id.clone(), None),
    )
    .expect("dispatch")
    .expect("reply");
    assert_eq!(
        db.get_game(&none_id).unwrap().my_color,
        PlayerColor::White,
        "absent suggested_color uses provisional White"
    );
}

#[test]
fn dispatch_game_invite_idempotent_for_same_peer() {
    let db = test_db();
    let game_id = generate_game_id();
    let invite = Message::new_game_invite(game_id.clone(), Some(Color::Black));

    let first = dispatch(db.as_ref(), "peer-a", invite.clone()).expect("first");
    assert!(matches!(first, Some(Message::GameInvite(_))));

    let second = dispatch(db.as_ref(), "peer-a", invite).expect("second");
    assert!(
        matches!(second, Some(Message::GameInvite(_))),
        "duplicate invite from same peer should echo"
    );

    let games = db.get_games_by_status(GameStatus::Pending).expect("games");
    let matching: Vec<_> = games.into_iter().filter(|g| g.id == game_id).collect();
    assert_eq!(matching.len(), 1, "still exactly one game row");

    let messages = db.get_messages_for_game(&game_id).expect("messages");
    let invite_count = messages
        .iter()
        .filter(|m| m.message_type == "GameInvite")
        .count();
    assert_eq!(invite_count, 1, "idempotent retry should not duplicate invite message");
}

#[test]
fn dispatch_game_invite_conflict_different_peer_declines() {
    let db = test_db();
    let game_id = generate_game_id();
    let invite = Message::new_game_invite(game_id.clone(), Some(Color::White));

    dispatch(db.as_ref(), "peer-a", invite.clone())
        .expect("first")
        .expect("echo");

    let conflict = dispatch(db.as_ref(), "peer-b", invite).expect("conflict dispatch");
    match conflict {
        Some(Message::GameDecline(decline)) => {
            assert_eq!(decline.game_id, game_id);
            assert!(decline.reason.is_some());
        }
        other => panic!("expected GameDecline, got {other:?}"),
    }

    let game = db.get_game(&game_id).expect("original game");
    assert_eq!(game.opponent_peer_id, "peer-a");
    assert_eq!(game.status, GameStatus::Pending);
}

#[test]
fn dispatch_invalid_game_invite_soft_rejects_with_decline() {
    let db = test_db();
    let invite = Message::new_game_invite("not-a-uuid".to_string(), None);

    let result = dispatch(db.as_ref(), "peer-a", invite).expect("validation soft-fails as Ok");
    match result {
        Some(Message::GameDecline(decline)) => {
            assert_eq!(decline.game_id, "not-a-uuid");
            assert!(
                decline
                    .reason
                    .as_deref()
                    .is_some_and(|r| r.contains("validation failed")),
                "decline should explain validation failure"
            );
        }
        other => panic!("invalid invite should GameDecline, got {other:?}"),
    }
    assert!(
        db.get_game("not-a-uuid").is_err(),
        "invalid invite must not create a game row"
    );
}

#[test]
fn dispatch_stubbed_chess_variants_return_no_reply() {
    let db = test_db();
    let game_id = generate_game_id();
    let board = Board::new();
    let board_hash = hash_board_state(&board);

    let stubbed = [
        Message::new_game_accept(game_id.clone(), Color::White),
        Message::new_game_decline(game_id.clone(), Some("busy".to_string())),
        Message::new_move(game_id.clone(), "e2e4".to_string(), board_hash.clone()),
        Message::new_move_ack(game_id.clone(), None),
        Message::new_sync_request(game_id.clone()),
        Message::new_sync_response(game_id, board.to_fen(), Vec::new(), board_hash),
    ];

    for message in stubbed {
        let message_type = message.message_type();
        let result = dispatch(db.as_ref(), "peer-a", message);
        assert!(
            result.is_ok(),
            "{message_type} dispatch should not error: {result:?}"
        );
        assert!(
            result.unwrap().is_none(),
            "{message_type} stub should return no reply"
        );
    }
}

#[test]
fn handler_error_display_covers_variants() {
    let not_implemented = HandlerError::NotImplemented("GameInvite");
    assert!(not_implemented.to_string().contains("GameInvite"));

    let validation = Message::new_game_invite("bad".to_string(), None)
        .validate()
        .expect_err("invalid game id");
    let wrapped = HandlerError::from(validation);
    assert!(wrapped.to_string().contains("validation failed"));
}

/// Optional cleanup: ensure the old stringly catch-all is gone and stub wording
/// remains for unimplemented chess handlers.
#[test]
fn old_catch_all_log_replaced_by_handler_stubs() {
    let server_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/network/server.rs"
    ));
    let handlers_src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/network/handlers.rs"
    ));

    assert!(
        !server_src.contains("no specific handler"),
        "server loop should no longer use the old catch-all log string"
    );
    assert!(
        !server_src.contains("message.message_type()"),
        "server loop should dispatch via typed handlers, not stringly message_type match"
    );
    assert!(
        server_src.contains("handlers::dispatch"),
        "server loop should call handlers::dispatch"
    );
    assert!(
        handlers_src.contains("handler not yet implemented"),
        "handlers module should log the stub-not-implemented message"
    );
}
