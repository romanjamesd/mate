//! Server chess handler integration: invite / accept / decline / move / sync lifecycle

use crate::common::test_helpers::test_server_database;
use mate::chess::{Board, Color};
use mate::crypto::Identity;
use mate::messages::chess::{
    generate_game_id, hash_board_state, GameAccept, GameDecline, GameInvite, Move,
};
use mate::messages::Message;
use mate::network::{Client, Server};
use mate::storage::models::{GameStatus, PlayerColor};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Live Server + Client: GameInvite is echoed and persisted as Pending.
#[tokio::test]
async fn server_game_invite_echoes_and_persists_pending() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());
    let client_peer_id = client_identity.peer_id().to_string();

    let database = test_server_database(server_identity.peer_id().as_str());
    let db_for_assert = Arc::clone(&database);

    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    let game_id = generate_game_id();
    let invite = Message::new_game_invite(game_id.clone(), Some(Color::Black));
    connection
        .send_message(invite)
        .await
        .expect("send GameInvite");

    let receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        receive.is_ok(),
        "should receive invite reply before timeout"
    );
    let (response, _sender) = receive.unwrap().expect("receive ok");

    match response {
        Message::GameInvite(echo) => {
            assert_eq!(echo.game_id, game_id);
            assert_eq!(echo.suggested_color, Some(Color::Black));
        }
        other => panic!("expected GameInvite echo, got {}", other.message_type()),
    }

    let game = db_for_assert
        .get_game(&game_id)
        .expect("pending game should exist on server DB");
    assert_eq!(game.id, game_id);
    assert_eq!(game.opponent_peer_id, client_peer_id);
    assert_eq!(game.my_color, PlayerColor::Black);
    assert_eq!(game.status, GameStatus::Pending);

    let messages = db_for_assert
        .get_messages_for_game(&game_id)
        .expect("messages for game");
    let invite_msg = messages
        .iter()
        .find(|m| m.message_type == "GameInvite")
        .expect("stored GameInvite message");
    assert_eq!(invite_msg.sender_peer_id, client_peer_id);
    let parsed: GameInvite =
        serde_json::from_str(&invite_msg.content).expect("parse stored invite JSON");
    assert_eq!(parsed.game_id, game_id);
    assert_eq!(parsed.suggested_color, Some(Color::Black));

    let _ = connection.close().await;
    server_handle.abort();
}

/// Live Server + Client: invite then accept → Active with opposite color.
#[tokio::test]
async fn server_game_invite_then_accept_activates() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());
    let client_peer_id = client_identity.peer_id().to_string();

    let database = test_server_database(server_identity.peer_id().as_str());
    let db_for_assert = Arc::clone(&database);

    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    let game_id = generate_game_id();
    connection
        .send_message(Message::new_game_invite(
            game_id.clone(),
            Some(Color::White),
        ))
        .await
        .expect("send GameInvite");

    let invite_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        invite_receive.is_ok(),
        "should receive invite reply before timeout"
    );
    let (invite_response, _) = invite_receive.unwrap().expect("invite receive ok");
    assert!(
        matches!(invite_response, Message::GameInvite(_)),
        "expected GameInvite echo"
    );

    connection
        .send_message(Message::new_game_accept(game_id.clone(), Color::White))
        .await
        .expect("send GameAccept");

    let accept_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        accept_receive.is_ok(),
        "should receive accept reply before timeout"
    );
    let (accept_response, _) = accept_receive.unwrap().expect("accept receive ok");

    match accept_response {
        Message::GameAccept(echo) => {
            assert_eq!(echo.game_id, game_id);
            assert_eq!(echo.accepted_color, Color::White);
        }
        other => panic!("expected GameAccept echo, got {}", other.message_type()),
    }

    let game = db_for_assert
        .get_game(&game_id)
        .expect("game should exist on server DB");
    assert_eq!(game.opponent_peer_id, client_peer_id);
    assert_eq!(game.status, GameStatus::Active);
    assert_eq!(
        game.my_color,
        PlayerColor::Black,
        "server takes opposite of accepted_color"
    );

    let messages = db_for_assert
        .get_messages_for_game(&game_id)
        .expect("messages for game");
    let accept_msg = messages
        .iter()
        .find(|m| m.message_type == "GameAccept")
        .expect("stored GameAccept message");
    assert_eq!(accept_msg.sender_peer_id, client_peer_id);
    let parsed: GameAccept =
        serde_json::from_str(&accept_msg.content).expect("parse stored accept JSON");
    assert_eq!(parsed.game_id, game_id);
    assert_eq!(parsed.accepted_color, Color::White);

    let _ = connection.close().await;
    server_handle.abort();
}

/// Live Server + Client: invite then decline → Abandoned.
#[tokio::test]
async fn server_game_invite_then_decline_abandons() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());
    let client_peer_id = client_identity.peer_id().to_string();

    let database = test_server_database(server_identity.peer_id().as_str());
    let db_for_assert = Arc::clone(&database);

    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    let game_id = generate_game_id();
    connection
        .send_message(Message::new_game_invite(
            game_id.clone(),
            Some(Color::Black),
        ))
        .await
        .expect("send GameInvite");

    let invite_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        invite_receive.is_ok(),
        "should receive invite reply before timeout"
    );
    let (invite_response, _) = invite_receive.unwrap().expect("invite receive ok");
    assert!(
        matches!(invite_response, Message::GameInvite(_)),
        "expected GameInvite echo"
    );

    connection
        .send_message(Message::new_game_decline(
            game_id.clone(),
            Some("busy".to_string()),
        ))
        .await
        .expect("send GameDecline");

    let decline_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        decline_receive.is_ok(),
        "should receive decline reply before timeout"
    );
    let (decline_response, _) = decline_receive.unwrap().expect("decline receive ok");

    match decline_response {
        Message::GameDecline(echo) => {
            assert_eq!(echo.game_id, game_id);
            assert_eq!(echo.reason.as_deref(), Some("busy"));
        }
        other => panic!("expected GameDecline echo, got {}", other.message_type()),
    }

    let game = db_for_assert
        .get_game(&game_id)
        .expect("game should exist on server DB");
    assert_eq!(game.opponent_peer_id, client_peer_id);
    assert_eq!(game.status, GameStatus::Abandoned);

    let messages = db_for_assert
        .get_messages_for_game(&game_id)
        .expect("messages for game");
    let decline_msg = messages
        .iter()
        .find(|m| m.message_type == "GameDecline")
        .expect("stored GameDecline message");
    assert_eq!(decline_msg.sender_peer_id, client_peer_id);
    let parsed: GameDecline =
        serde_json::from_str(&decline_msg.content).expect("parse stored decline JSON");
    assert_eq!(parsed.game_id, game_id);
    assert_eq!(parsed.reason.as_deref(), Some("busy"));

    let _ = connection.close().await;
    server_handle.abort();
}

/// Live Server + Client: invite → accept → Move → MoveAck + stored message.
#[tokio::test]
async fn server_move_acks_and_persists() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());
    let client_peer_id = client_identity.peer_id().to_string();

    let database = test_server_database(server_identity.peer_id().as_str());
    let db_for_assert = Arc::clone(&database);

    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    let game_id = generate_game_id();
    connection
        .send_message(Message::new_game_invite(
            game_id.clone(),
            Some(Color::White),
        ))
        .await
        .expect("send GameInvite");

    let invite_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        invite_receive.is_ok(),
        "should receive invite reply before timeout"
    );
    let (invite_response, _) = invite_receive.unwrap().expect("invite receive ok");
    assert!(
        matches!(invite_response, Message::GameInvite(_)),
        "expected GameInvite echo"
    );

    connection
        .send_message(Message::new_game_accept(game_id.clone(), Color::White))
        .await
        .expect("send GameAccept");

    let accept_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        accept_receive.is_ok(),
        "should receive accept reply before timeout"
    );
    let (accept_response, _) = accept_receive.unwrap().expect("accept receive ok");
    assert!(
        matches!(accept_response, Message::GameAccept(_)),
        "expected GameAccept echo"
    );

    let board_hash = hash_board_state(&Board::new());
    connection
        .send_message(Message::new_move(
            game_id.clone(),
            "e2e4".to_string(),
            board_hash.clone(),
        ))
        .await
        .expect("send Move");

    let move_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        move_receive.is_ok(),
        "should receive MoveAck before timeout"
    );
    let (move_response, _) = move_receive.unwrap().expect("move receive ok");

    match move_response {
        Message::MoveAck(ack) => {
            assert_eq!(ack.game_id, game_id);
            assert!(ack.move_id.is_none());
        }
        other => panic!("expected MoveAck, got {}", other.message_type()),
    }

    let game = db_for_assert
        .get_game(&game_id)
        .expect("game should exist on server DB");
    assert_eq!(game.opponent_peer_id, client_peer_id);
    assert_eq!(game.status, GameStatus::Active);

    let messages = db_for_assert
        .get_messages_for_game(&game_id)
        .expect("messages for game");
    let move_msg = messages
        .iter()
        .find(|m| m.message_type == "Move")
        .expect("stored Move message");
    assert_eq!(move_msg.sender_peer_id, client_peer_id);
    let parsed: Move = serde_json::from_str(&move_msg.content).expect("parse stored move JSON");
    assert_eq!(parsed.game_id, game_id);
    assert_eq!(parsed.chess_move, "e2e4");
    assert_eq!(parsed.board_state_hash, board_hash);

    let _ = connection.close().await;
    server_handle.abort();
}

/// Live Server + Client: invite → accept → move → SyncRequest → SyncResponse.
#[tokio::test]
async fn server_sync_request_returns_sync_response() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    let database = test_server_database(server_identity.peer_id().as_str());

    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    let game_id = generate_game_id();
    connection
        .send_message(Message::new_game_invite(
            game_id.clone(),
            Some(Color::White),
        ))
        .await
        .expect("send GameInvite");

    let invite_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        invite_receive.is_ok(),
        "should receive invite reply before timeout"
    );
    let (invite_response, _) = invite_receive.unwrap().expect("invite receive ok");
    assert!(
        matches!(invite_response, Message::GameInvite(_)),
        "expected GameInvite echo"
    );

    connection
        .send_message(Message::new_game_accept(game_id.clone(), Color::White))
        .await
        .expect("send GameAccept");

    let accept_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        accept_receive.is_ok(),
        "should receive accept reply before timeout"
    );
    let (accept_response, _) = accept_receive.unwrap().expect("accept receive ok");
    assert!(
        matches!(accept_response, Message::GameAccept(_)),
        "expected GameAccept echo"
    );

    let board_hash = hash_board_state(&Board::new());
    connection
        .send_message(Message::new_move(
            game_id.clone(),
            "e2e4".to_string(),
            board_hash,
        ))
        .await
        .expect("send Move");

    let move_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        move_receive.is_ok(),
        "should receive MoveAck before timeout"
    );
    let (move_response, _) = move_receive.unwrap().expect("move receive ok");
    assert!(
        matches!(move_response, Message::MoveAck(_)),
        "expected MoveAck"
    );

    connection
        .send_message(Message::new_sync_request(game_id.clone()))
        .await
        .expect("send SyncRequest");

    let sync_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        sync_receive.is_ok(),
        "should receive SyncResponse before timeout"
    );
    let (sync_response, _) = sync_receive.unwrap().expect("sync receive ok");

    match sync_response {
        Message::SyncResponse(resp) => {
            assert_eq!(resp.game_id, game_id);
            assert_eq!(resp.move_history, vec!["e2e4".to_string()]);
            assert!(
                Board::from_fen(&resp.board_state).is_ok(),
                "SyncResponse board_state should be a valid FEN"
            );
            assert_eq!(
                resp.board_state_hash,
                hash_board_state(&Board::from_fen(&resp.board_state).expect("parse FEN"))
            );
        }
        other => panic!("expected SyncResponse, got {}", other.message_type()),
    }

    let _ = connection.close().await;
    server_handle.abort();
}

/// Live Server + Clients: wrong peer Move → GameDecline, no Move stored.
#[tokio::test]
async fn server_wrong_peer_move_soft_declines() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_a_identity = Arc::new(Identity::generate().unwrap());
    let client_b_identity = Arc::new(Identity::generate().unwrap());

    let database = test_server_database(server_identity.peer_id().as_str());
    let db_for_assert = Arc::clone(&database);

    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client_a = Client::new(client_a_identity);
    let mut connection_a = client_a.connect(&server_addr).await.unwrap();

    let game_id = generate_game_id();
    connection_a
        .send_message(Message::new_game_invite(
            game_id.clone(),
            Some(Color::White),
        ))
        .await
        .expect("send GameInvite");

    let invite_receive = timeout(Duration::from_secs(5), connection_a.receive_message()).await;
    assert!(
        invite_receive.is_ok(),
        "should receive invite reply before timeout"
    );
    let (invite_response, _) = invite_receive.unwrap().expect("invite receive ok");
    assert!(
        matches!(invite_response, Message::GameInvite(_)),
        "expected GameInvite echo"
    );

    connection_a
        .send_message(Message::new_game_accept(game_id.clone(), Color::White))
        .await
        .expect("send GameAccept");

    let accept_receive = timeout(Duration::from_secs(5), connection_a.receive_message()).await;
    assert!(
        accept_receive.is_ok(),
        "should receive accept reply before timeout"
    );
    let (accept_response, _) = accept_receive.unwrap().expect("accept receive ok");
    assert!(
        matches!(accept_response, Message::GameAccept(_)),
        "expected GameAccept echo"
    );

    let client_b = Client::new(client_b_identity);
    let mut connection_b = client_b.connect(&server_addr).await.unwrap();

    let board_hash = hash_board_state(&Board::new());
    connection_b
        .send_message(Message::new_move(
            game_id.clone(),
            "e2e4".to_string(),
            board_hash,
        ))
        .await
        .expect("send Move as wrong peer");

    let decline_receive = timeout(Duration::from_secs(5), connection_b.receive_message()).await;
    assert!(
        decline_receive.is_ok(),
        "should receive GameDecline before timeout"
    );
    let (decline_response, _) = decline_receive.unwrap().expect("decline receive ok");

    match decline_response {
        Message::GameDecline(decline) => {
            assert_eq!(decline.game_id, game_id);
            assert!(decline.reason.is_some());
        }
        other => panic!("expected GameDecline, got {}", other.message_type()),
    }

    let messages = db_for_assert
        .get_messages_for_game(&game_id)
        .expect("messages for game");
    assert!(
        messages.iter().all(|m| m.message_type != "Move"),
        "wrong peer must not store a Move"
    );

    assert!(
        !server_handle.is_finished(),
        "server should still be running after soft reject"
    );

    let _ = connection_a.close().await;
    let _ = connection_b.close().await;
    server_handle.abort();
}

/// Live Server + Client: invalid Move game_id → GameDecline, no panic.
#[tokio::test]
async fn server_invalid_move_soft_declines() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    let database = test_server_database(server_identity.peer_id().as_str());

    let server = Server::bind("127.0.0.1:0", server_identity, database)
        .await
        .unwrap();
    let server_addr = server.local_addr().unwrap().to_string();

    let server_handle = tokio::spawn(async move { server.run().await });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new(client_identity);
    let mut connection = client.connect(&server_addr).await.unwrap();

    connection
        .send_message(Message::new_move(
            "not-a-uuid".to_string(),
            "e2e4".to_string(),
            hash_board_state(&Board::new()),
        ))
        .await
        .expect("send invalid Move");

    let decline_receive = timeout(Duration::from_secs(5), connection.receive_message()).await;
    assert!(
        decline_receive.is_ok(),
        "should receive GameDecline before timeout"
    );
    let (decline_response, _) = decline_receive.unwrap().expect("decline receive ok");

    match decline_response {
        Message::GameDecline(decline) => {
            assert_eq!(decline.game_id, "not-a-uuid");
            assert!(
                decline
                    .reason
                    .as_deref()
                    .is_some_and(|r| r.contains("validation failed")),
                "decline should explain validation failure"
            );
        }
        other => panic!("expected GameDecline, got {}", other.message_type()),
    }

    assert!(
        !server_handle.is_finished(),
        "server should still be running after soft reject"
    );

    let _ = connection.close().await;
    server_handle.abort();
}
