//! Unit tests for server message dispatch scaffolding

use mate::chess::{Board, Color};
use mate::messages::chess::{generate_game_id, hash_board_state};
use mate::messages::Message;
use mate::network::{dispatch, HandlerError};
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
fn dispatch_valid_game_invite_is_stubbed_no_reply() {
    let db = test_db();
    let invite = Message::new_game_invite(generate_game_id(), Some(Color::White));

    let result = dispatch(db.as_ref(), "peer-a", invite).expect("dispatch should succeed");
    assert!(
        result.is_none(),
        "GameInvite stub must not reply until invite persistence exists"
    );
}

#[test]
fn dispatch_invalid_game_invite_soft_rejects_without_panic() {
    let db = test_db();
    let invite = Message::new_game_invite("not-a-uuid".to_string(), None);

    let result = dispatch(db.as_ref(), "peer-a", invite).expect("validation soft-fails as Ok");
    assert!(result.is_none(), "invalid invite should not produce a reply");
}

#[test]
fn dispatch_all_valid_chess_variants_stubbed_without_panic() {
    let db = test_db();
    let game_id = generate_game_id();
    let board = Board::new();
    let board_hash = hash_board_state(&board);

    let messages = [
        Message::new_game_invite(game_id.clone(), Some(Color::Black)),
        Message::new_game_accept(game_id.clone(), Color::White),
        Message::new_game_decline(game_id.clone(), Some("busy".to_string())),
        Message::new_move(game_id.clone(), "e2e4".to_string(), board_hash.clone()),
        Message::new_move_ack(game_id.clone(), None),
        Message::new_sync_request(game_id.clone()),
        Message::new_sync_response(game_id, board.to_fen(), Vec::new(), board_hash),
    ];

    for message in messages {
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

/// Optional cleanup: ensure the old stringly catch-all is gone and the stub
/// log wording lives in the handlers module instead.
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
