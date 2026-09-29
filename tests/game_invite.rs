use mate::chess::Color;
use mate::cli::app::App;
use mate::cli::error_handler::CliError;
use mate::cli::network_manager::{classify_invite_response, InviteOutcome, SendOutcome};
use mate::crypto::Identity;
use mate::messages::chess::{
    generate_game_id, ChessProtocolError, GameAccept, GameDecline, GameInvite,
};
use mate::messages::{Message, RetryStrategy};
use mate::network::{handlers::dispatch, Connection, Server};
use mate::storage::{Database, GameStatus, PlayerColor};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::time::timeout;

async fn app() -> (App, TempDir) {
    let dir = TempDir::new().unwrap();
    let app = App::new_with_data_dir(dir.path().to_path_buf())
        .await
        .unwrap();
    (app, dir)
}

fn db(dir: &TempDir) -> Database {
    Database::new_with_path("test-peer", &dir.path().join("remote.sqlite")).unwrap()
}

fn only_invite(app: &App) -> (String, GameInvite) {
    let games = app
        .database
        .get_games_by_status(GameStatus::Pending)
        .unwrap();
    assert_eq!(games.len(), 1);
    let game = &games[0];
    assert!(!game.opponent_peer_id.is_empty());
    let messages = app.database.get_messages_for_game(&game.id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].message_type, "GameInvite");
    assert_eq!(messages[0].sender_peer_id, app.peer_id());
    (
        game.id.clone(),
        serde_json::from_str(&messages[0].content).unwrap(),
    )
}

async fn cli(dir: &TempDir, args: &[&str]) -> std::process::Output {
    timeout(
        Duration::from_secs(15),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_mate"))
            .args(args)
            .env("MATE_DATA_DIR", dir.path())
            .env("MATE_CONFIG_DIR", dir.path().join("config"))
            .env("RUST_LOG", "error")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap()
}

#[test]
fn response_classification_requires_exact_invite_or_correlated_decline() {
    let request = GameInvite::new(generate_game_id(), Some(Color::Black))
        .with_reply_to("127.0.0.1:9000".to_string());
    let classify = |response| {
        classify_invite_response(
            &request,
            SendOutcome {
                response,
                peer_id: "authenticated-peer".to_string(),
            },
        )
    };
    assert_eq!(
        classify(Message::GameInvite(request.clone())).unwrap(),
        InviteOutcome::Acknowledged {
            acknowledgement: request.clone(),
            peer_id: "authenticated-peer".to_string()
        }
    );
    let decline = GameDecline::new(request.game_id.clone(), Some("busy".to_string()));
    assert_eq!(
        classify(Message::GameDecline(decline.clone())).unwrap(),
        InviteOutcome::Declined {
            decline,
            peer_id: "authenticated-peer".to_string()
        }
    );
    let mut changed = request.clone();
    changed.reply_to = None;
    for response in [
        Message::GameInvite(changed),
        Message::GameInvite(GameInvite::new(request.game_id.clone(), Some(Color::White))),
        Message::GameInvite(GameInvite::new(generate_game_id(), Some(Color::Black))),
        Message::GameDecline(GameDecline::new_no_reason(generate_game_id())),
        Message::GameAccept(GameAccept::new(request.game_id.clone(), Color::Black)),
    ] {
        assert!(matches!(
            classify(response),
            Err(ChessProtocolError::UnexpectedMessage { .. })
        ));
    }
}

#[test]
fn blank_opponent_is_rejected_by_storage() {
    let dir = TempDir::new().unwrap();
    let db = db(&dir);
    for blank in ["", "   "] {
        assert!(db
            .create_game_with_id(
                generate_game_id(),
                blank.to_string(),
                PlayerColor::White,
                None
            )
            .is_err());
        let game = db
            .create_game("peer".to_string(), PlayerColor::White, None)
            .unwrap();
        assert!(db.update_opponent_peer_id(&game.id, blank).is_err());
        assert_eq!(db.get_game(&game.id).unwrap().opponent_peer_id, "peer");
    }
}

#[test]
fn inbound_invite_transaction_rolls_back_and_checks_retry_payload() {
    let dir = TempDir::new().unwrap();
    let db = db(&dir);
    let invite = GameInvite::new(generate_game_id(), Some(Color::Black));
    db.with_connection(|conn| {
        conn.execute_batch("CREATE TRIGGER fail_invite BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL, 'injected message failure'); END")?;
        Ok(())
    }).unwrap();
    let response = dispatch(&db, "peer", Message::GameInvite(invite.clone()))
        .unwrap()
        .unwrap();
    assert!(matches!(response, Message::GameDecline(_)));
    assert!(db.get_game(&invite.game_id).is_err());
    assert!(db
        .get_messages_for_game(&invite.game_id)
        .unwrap()
        .is_empty());
    db.with_connection(|conn| {
        conn.execute_batch("DROP TRIGGER fail_invite")?;
        Ok(())
    })
    .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            dispatch(&db, "peer", Message::GameInvite(invite.clone())).unwrap(),
            Some(Message::GameInvite(ack)) if ack == invite
        ));
    }
    let mut changed = invite.clone();
    changed.reply_to = Some("127.0.0.1:9000".to_string());
    assert!(matches!(
        dispatch(&db, "peer", Message::GameInvite(changed)).unwrap(),
        Some(Message::GameDecline(_))
    ));
    let messages = db.get_messages_for_game(&invite.game_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        serde_json::from_str::<GameInvite>(&messages[0].content).unwrap(),
        invite
    );
    assert_eq!(
        db.get_game(&invite.game_id).unwrap().my_color,
        PlayerColor::Black
    );
}

#[tokio::test]
async fn local_persistence_failure_after_handshake_sends_no_invite() {
    for table in ["games", "messages"] {
        let (app, _dir) = app().await;
        app.database.with_connection(|conn| {
            conn.execute_batch(&format!("CREATE TRIGGER fail_invite BEFORE INSERT ON {table} BEGIN SELECT RAISE(FAIL, 'injected failure'); END"))?;
            Ok(())
        }).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let identity = Arc::new(Identity::generate().unwrap());
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut conn = Connection::new(stream, identity).await;
            conn.handle_handshake_request().await.unwrap();
            assert!(
                conn.receive_message().await.is_err(),
                "no invite may be transmitted before commit"
            );
        });
        assert!(matches!(
            app.handle_invite(address, None).await,
            Err(CliError::Storage(_))
        ));
        timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        assert!(app
            .database
            .get_games_by_status(GameStatus::Pending)
            .unwrap()
            .is_empty());
        assert!(app.database.get_recent_messages(10).unwrap().is_empty());
    }
}

#[tokio::test]
async fn cli_decline_and_protocol_errors_preserve_persisted_invite_without_success() {
    for case in ["decline", "wrong id", "unexpected"] {
        let (app, dir) = app().await;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let identity = Arc::new(Identity::generate().unwrap());
        let expected_peer = identity.peer_id().to_string();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut conn = Connection::new(stream, identity).await;
            conn.handle_handshake_request().await.unwrap();
            let (Message::GameInvite(invite), _) = conn.receive_message().await.unwrap() else {
                panic!("invite expected")
            };
            let response = match case {
                "decline" => Message::GameDecline(GameDecline::new(
                    invite.game_id,
                    Some("transient persistence failure".to_string()),
                )),
                "wrong id" => Message::GameDecline(GameDecline::new_no_reason(generate_game_id())),
                _ => Message::new_ping(1, "unexpected".to_string()),
            };
            conn.send_message(response).await.unwrap();
        });
        let output = cli(&dir, &["invite", &address, "--color", "black"]).await;
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Invitation sent successfully"));
        if case == "decline" {
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("transient persistence failure")
            );
        }
        let (id, _) = only_invite(&app);
        assert_eq!(
            app.database.get_game(&id).unwrap().opponent_peer_id,
            expected_peer
        );
        assert_eq!(
            app.network_manager
                .get_network_stats()
                .await
                .total_pending_messages,
            0
        );
        task.await.unwrap();
    }
}

#[tokio::test]
async fn persisted_retry_after_restart_reuses_exact_payload_without_duplicates() {
    let (app, dir) = app().await;
    let remote_dir = TempDir::new().unwrap();
    let remote = Arc::new(db(&remote_dir));
    let identity = Arc::new(Identity::generate().unwrap());
    let peer = identity.peer_id().to_string();
    let server = Server::bind("127.0.0.1:0", identity, remote.clone())
        .await
        .unwrap();
    let address = server.local_addr().unwrap().to_string();
    let task = tokio::spawn(async move { server.run().await });
    let output = cli(&dir, &["invite", &address, "--color", "black"]).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Invitation sent successfully"));
    let (id, invite) = only_invite(&app);
    assert_eq!(app.database.get_game(&id).unwrap().opponent_peer_id, peer);
    drop(app);
    let mut restarted = App::new_with_data_dir(dir.path().to_path_buf())
        .await
        .unwrap();
    restarted.config.default_bind_addr = "127.0.0.1:9999".to_string();
    restarted.handle_retry_invite(id.clone()).await.unwrap();
    let output = cli(&dir, &["retry-invite", &id]).await;
    assert!(output.status.success());
    assert_eq!(only_invite(&restarted), (id.clone(), invite.clone()));
    let messages = remote.get_messages_for_game(&id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(
        serde_json::from_str::<GameInvite>(&messages[0].content).unwrap(),
        invite
    );
    task.abort();
}

#[tokio::test]
async fn invite_reconnect_checks_peer_before_transmission() {
    let (app, _dir) = app().await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let original = Arc::new(Identity::generate().unwrap());
    let original_peer = original.peer_id().to_string();
    let replacement = Arc::new(Identity::generate().unwrap());
    let task = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut conn = Connection::new(stream, original).await;
        conn.handle_handshake_request().await.unwrap();
        assert!(matches!(
            conn.receive_message().await.unwrap().0,
            Message::GameInvite(_)
        ));
        drop(conn); // Lost acknowledgement forces a reconnect.
                    // Check both the automatic reconnect and a separate explicit retry.
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.unwrap();
            let mut conn = Connection::new(stream, replacement.clone()).await;
            conn.handle_handshake_request().await.unwrap();
            assert!(
                conn.receive_message().await.is_err(),
                "replacement peer must receive no invite"
            );
        }
    });
    let error = app.handle_invite(address, None).await.unwrap_err();
    assert!(error.to_string().contains("opponent mismatch"));
    let (id, _) = only_invite(&app);
    assert_eq!(
        app.database.get_game(&id).unwrap().opponent_peer_id,
        original_peer
    );
    assert!(app
        .handle_retry_invite(id)
        .await
        .unwrap_err()
        .to_string()
        .contains("opponent mismatch"));
    timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn lost_acknowledgements_preserve_pending_or_concurrent_acceptance() {
    for race in [false, true] {
        let (app, dir) = app().await;
        let remote_dir = TempDir::new().unwrap();
        let remote = Arc::new(db(&remote_dir));
        let identity = Arc::new(Identity::generate().unwrap());
        let peer = identity.peer_id().to_string();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let path = app.database_path();
        let remote_copy = remote.clone();
        let peer_copy = peer.clone();
        let local_peer = app.peer_id().to_string();
        let server_identity = identity.clone();
        let task = tokio::spawn(async move {
            let mut id = String::new();
            for attempt in 0..RetryStrategy::Normal.max_attempts() {
                let (stream, _) = listener.accept().await.unwrap();
                let mut conn = Connection::new(stream, identity.clone()).await;
                conn.handle_handshake_request().await.unwrap();
                let (message, sender) = conn.receive_message().await.unwrap();
                let Message::GameInvite(ref invite) = message else {
                    panic!("expected invite")
                };
                id = invite.game_id.clone();
                assert!(matches!(
                    dispatch(&remote_copy, &sender, message).unwrap(),
                    Some(Message::GameInvite(_))
                ));
                if race && attempt == 0 {
                    let local = Database::new_with_path(&local_peer, &path).unwrap();
                    assert!(matches!(
                        dispatch(
                            &local,
                            &peer_copy,
                            Message::GameAccept(GameAccept::new(id.clone(), Color::Black))
                        )
                        .unwrap(),
                        Some(Message::GameAccept(_))
                    ));
                }
                drop(conn); // Commit succeeded, but every acknowledgement is lost.
            }
            id
        });
        let error = timeout(
            Duration::from_secs(15),
            app.handle_invite(address.clone(), None),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(error.to_string().contains("delivery may have succeeded"));
        let id = task.await.unwrap();
        assert_eq!(
            app.network_manager
                .get_network_stats()
                .await
                .total_pending_messages,
            0
        );
        let game = app.database.get_game(&id).unwrap();
        assert_eq!(game.opponent_peer_id, peer);
        assert_eq!(
            game.status,
            if race {
                GameStatus::Active
            } else {
                GameStatus::Pending
            }
        );
        assert_eq!(remote.get_messages_for_game(&id).unwrap().len(), 1);
        if !race {
            // Recover the uncertain delivery in a new process using the
            // persisted payload, without adding a second invitation remotely.
            let original = only_invite(&app);
            let server = Server::bind(&address, server_identity, remote.clone())
                .await
                .unwrap();
            let server_task = tokio::spawn(async move { server.run().await });
            let output = cli(&dir, &["retry-invite", &id]).await;
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(only_invite(&app), original);
            assert_eq!(remote.get_messages_for_game(&id).unwrap().len(), 1);
            server_task.abort();
            assert!(matches!(
                dispatch(
                    &app.database,
                    &peer,
                    Message::GameAccept(GameAccept::new(id.clone(), Color::Black))
                )
                .unwrap(),
                Some(Message::GameAccept(_))
            ));
            assert_eq!(
                app.database.get_game(&id).unwrap().status,
                GameStatus::Active
            );
        }
    }
}
