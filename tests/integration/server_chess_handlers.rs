//! Server chess handler integration: invite / accept / decline / move / sync lifecycle

use crate::common::test_helpers::{next_move, play_move, test_server_database};
use mate::chess::{Board, Color};
use mate::crypto::Identity;
use mate::messages::chess::{
    generate_game_id, hash_board_state, GameAccept, GameDecline, GameInvite, Move, MoveAck,
};
use mate::messages::wire::FramedMessage;
use mate::messages::{Message, SignedEnvelope};
use mate::network::{Client, Server};
use mate::storage::models::{GameStatus, PlayerColor};
use mate::storage::Database;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Authenticate as A while retaining the raw socket so an application envelope
/// can subsequently be signed by B. Ordinary Connection sends cannot do this.
async fn raw_server_peer(address: &str, peer: &Identity, server: &Identity) -> TcpStream {
    let mut stream = TcpStream::connect(address).await.unwrap();
    let hello = Message::new_ping(42, format!("HANDSHAKE_REQUEST:{}", peer.peer_id()));
    let framing = FramedMessage::default();
    framing
        .write_message(
            &mut stream,
            &SignedEnvelope::create(&hello, peer, None).unwrap(),
        )
        .await
        .unwrap();
    let response = framing.read_message(&mut stream).await.unwrap();
    assert!(response.verify_signature());
    assert!(response.is_timestamp_valid(300));
    assert_eq!(response.sender(), server.peer_id().as_str());
    let response = response.get_message().unwrap();
    assert!(response.is_pong());
    assert_eq!(response.get_nonce(), hello.get_nonce());
    assert_eq!(
        response.get_payload(),
        format!("HANDSHAKE_RESPONSE:{}", server.peer_id())
    );
    stream
}

async fn raw_move_ack(stream: &mut TcpStream, peer: &Identity, server: &Identity, mv: &Move) {
    let framing = FramedMessage::default();
    framing
        .write_message(
            stream,
            &SignedEnvelope::create(&Message::Move(mv.clone()), peer, None).unwrap(),
        )
        .await
        .unwrap();
    let response = framing.read_message(stream).await.unwrap();
    assert!(response.verify_signature());
    assert!(response.is_timestamp_valid(300));
    assert_eq!(response.sender(), server.peer_id().as_str());
    let Message::MoveAck(ack) = response.get_message().unwrap() else {
        panic!("expected correlated MoveAck, got {response:?}");
    };
    assert_eq!(ack, MoveAck::for_move(mv));
}

/// Include every game/message field, especially activity and record IDs.
fn game_snapshot(database: &Database, game_id: &str) -> serde_json::Value {
    serde_json::json!({
        "game": database.get_game(game_id).unwrap(),
        "messages": database.get_messages_for_game(game_id).unwrap(),
    })
}

async fn server_rejects_switched_move_signer(historical_status: Option<GameStatus>) {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let peer = Identity::generate().unwrap();
    let other = Identity::generate().unwrap();
    let dir = tempfile::TempDir::new().unwrap();
    let database = Arc::new(
        Database::new_with_path(
            server_identity.peer_id().as_str(),
            &dir.path().join("server.sqlite"),
        )
        .unwrap(),
    );
    let game_id = generate_game_id();
    database
        .create_game_with_id(
            game_id.clone(),
            peer.peer_id().to_string(),
            PlayerColor::Black,
            None,
        )
        .unwrap();
    database
        .update_game_status(&game_id, GameStatus::Active)
        .unwrap();
    let first = next_move(&database, &game_id, "e2e4");
    let server = Server::bind(
        "127.0.0.1:0",
        Arc::clone(&server_identity),
        Arc::clone(&database),
    )
    .await
    .unwrap();
    let address = server.local_addr().unwrap().to_string();

    let scenario = async {
        let historical = historical_status.is_some();
        if let Some(status) = historical_status {
            // Establish the first Move through the real server, then advance
            // beyond it so the rejected payload exercises historical retry logic.
            let mut stream = raw_server_peer(&address, &peer, &server_identity).await;
            raw_move_ack(&mut stream, &peer, &server_identity, &first).await;
            play_move(
                &database,
                &game_id,
                "e7e5",
                server_identity.peer_id().as_str(),
            );
            play_move(
                &database,
                &game_id,
                "g1f3",
                server_identity.peer_id().as_str(),
            );
            database.update_game_status(&game_id, status).unwrap();
        }
        // A same-second accidental activity update must be visible in the snapshot.
        database
            .with_connection(|conn| {
                conn.execute(
                    "UPDATE games SET updated_at = 1000 WHERE id = ?1",
                    [&game_id],
                )?;
                Ok(())
            })
            .unwrap();
        let before = game_snapshot(&database, &game_id);
        let expected_rows = if historical { 3 } else { 0 };
        assert_eq!(
            database.get_moves_for_game(&game_id).unwrap().len(),
            expected_rows
        );

        let mut stream = raw_server_peer(&address, &peer, &server_identity).await;
        let envelope = SignedEnvelope::create(&Message::Move(first.clone()), &other, None).unwrap();
        assert!(envelope.verify_signature());
        assert!(envelope.is_timestamp_valid(300));
        FramedMessage::default()
            .write_message(&mut stream, &envelope)
            .await
            .unwrap();
        // Peek at the first byte: even an application rejection is a
        // failure here. Timeout or malformed response cannot masquerade as EOF.
        let mut byte = [0];
        match timeout(Duration::from_secs(5), stream.peek(&mut byte))
            .await
            .expect("server must close the offending connection, not leave it idle")
        {
            Ok(0) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                ) => {}
            Ok(_) => {
                let reply = FramedMessage::default().read_message(&mut stream).await;
                let reply = reply.map(|envelope| envelope.get_message());
                panic!("expected closure without any application reply, got {reply:?}");
            }
            Err(error) => panic!("expected connection closure, got {error:?}"),
        }
        assert_eq!(game_snapshot(&database, &game_id), before);
        assert_eq!(
            database.get_moves_for_game(&game_id).unwrap().len(),
            expected_rows
        );

        // Rejection is isolated to that connection. A fresh A-authenticated
        // connection must accept the same payload, including completed retries.
        let mut legitimate = raw_server_peer(&address, &peer, &server_identity).await;
        raw_move_ack(&mut legitimate, &peer, &server_identity, &first).await;
        let rows = database.get_moves_for_game(&game_id).unwrap();
        assert_eq!(rows.len(), if historical { 3 } else { 1 });
        assert_eq!(rows[0].sender_peer_id, peer.peer_id().as_str());
        assert_eq!(
            serde_json::from_str::<Move>(&rows[0].content).unwrap(),
            first
        );
        if historical {
            assert_eq!(game_snapshot(&database, &game_id), before);
        }
    };
    // No detached server task: completion, panic, or timeout drops Server::run
    // and its JoinSet, cancelling every connection handler as well.
    timeout(Duration::from_secs(15), async {
        tokio::select! {
            result = server.run() => panic!("server stopped before scenario finished: {result:?}"),
            () = scenario => {}
        }
    })
    .await
    .expect("live-server signer regression timed out");
}

#[tokio::test]
async fn server_new_move_rejects_switched_signer_without_reply_or_state_change() {
    server_rejects_switched_move_signer(None).await;
}

#[tokio::test]
async fn server_historical_retry_rejects_switched_signer_without_reply_or_state_change() {
    for status in [GameStatus::Active, GameStatus::Completed] {
        server_rejects_switched_move_signer(Some(status)).await;
    }
}

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
    let invite = Message::GameInvite(
        GameInvite::new(game_id.clone(), Some(Color::Black)).with_reply_to("10.0.0.1:7777"),
    );
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
            assert_eq!(echo.reply_to.as_deref(), Some("10.0.0.1:7777"));
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
    assert_eq!(
        game.metadata,
        Some(serde_json::json!({ "dial_address": "10.0.0.1:7777" })),
        "server should stamp dial_address from reply_to"
    );

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
    assert_eq!(parsed.reply_to.as_deref(), Some("10.0.0.1:7777"));

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

    let mv = next_move(&db_for_assert, &game_id, "e2e4");
    connection
        .send_message(Message::Move(mv.clone()))
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
            assert_eq!(ack, mate::messages::chess::MoveAck::for_move(&mv));
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
    assert_eq!(parsed, mv);

    let _ = connection.close().await;
    server_handle.abort();
}

/// Live Server + Client: invite → accept → move → SyncRequest → SyncResponse.
#[tokio::test]
async fn server_sync_request_returns_sync_response() {
    let server_identity = Arc::new(Identity::generate().unwrap());
    let client_identity = Arc::new(Identity::generate().unwrap());

    let database = test_server_database(server_identity.peer_id().as_str());
    let db_for_fixture = Arc::clone(&database);

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

    let mv = next_move(&db_for_fixture, &game_id, "e2e4");
    connection
        .send_message(Message::Move(mv))
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

    let before = game_snapshot(&db_for_assert, &game_id);
    // A valid payload signed by the connection peer still needs game authorization.
    let mv = next_move(&db_for_assert, &game_id, "e2e4");
    connection_b
        .send_message(Message::Move(mv))
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
            assert_eq!(
                decline.reason.as_deref(),
                Some("peer is not the game opponent")
            );
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
    assert_eq!(game_snapshot(&db_for_assert, &game_id), before);

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
            1,
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
