use mate::chess::Color;
use mate::cli::app::App;
use mate::cli::error_handler::{cli_error_from_anyhow, CliError};
use mate::cli::network_manager::{classify_accept_response, GameAcceptOutcome, SendOutcome};
use mate::crypto::Identity;
use mate::messages::chess::{generate_game_id, ChessProtocolError, GameAccept, GameDecline};
use mate::messages::Message;
use mate::network::{Connection, Server};
use mate::storage::{Database, GameStatus, PlayerColor};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

#[test]
fn accept_response_classification_correlates_payload_and_preserves_peer() {
    let request = GameAccept::new(generate_game_id(), Color::Black);
    let classify = |response| {
        classify_accept_response(
            &request,
            SendOutcome {
                response,
                peer_id: "authenticated-peer".to_string(),
            },
        )
    };
    assert_eq!(
        classify(Message::GameAccept(request.clone())).unwrap(),
        GameAcceptOutcome::Accepted {
            acknowledgement: request.clone(),
            peer_id: "authenticated-peer".to_string(),
        }
    );
    for reason in [None, Some("already abandoned".to_string())] {
        let decline = GameDecline::new(request.game_id.clone(), reason);
        assert_eq!(
            classify(Message::GameDecline(decline.clone())).unwrap(),
            GameAcceptOutcome::Rejected {
                decline,
                peer_id: "authenticated-peer".to_string(),
            }
        );
    }
    for response in [
        Message::GameAccept(GameAccept::new(generate_game_id(), Color::Black)),
        Message::GameAccept(GameAccept::new(request.game_id.clone(), Color::White)),
        Message::GameDecline(GameDecline::new_no_reason(generate_game_id())),
        Message::new_ping(1, "unexpected".to_string()),
    ] {
        let error = classify(response).unwrap_err();
        assert!(matches!(
            error,
            ChessProtocolError::UnexpectedMessage { .. }
        ));
        assert!(matches!(
            cli_error_from_anyhow(anyhow::Error::new(error).context("accept response")),
            CliError::Protocol(_)
        ));
    }
}

async fn test_app() -> (App, TempDir) {
    let dir = TempDir::new().unwrap();
    let app = App::new_with_data_dir(dir.path().to_path_buf())
        .await
        .unwrap();
    (app, dir)
}

fn seed_local(app: &App, game_id: &str, opponent: &str, address: &str) {
    app.database
        .create_game_with_id(
            game_id.to_string(),
            opponent.to_string(),
            PlayerColor::White,
            Some(serde_json::json!({ "dial_address": address })),
        )
        .unwrap();
}

fn assert_pending_unchanged(app: &App, game_id: &str) {
    let game = app.database.get_game(game_id).unwrap();
    assert_eq!(game.status, GameStatus::Pending);
    assert_eq!(game.my_color, PlayerColor::White);
    assert!(app
        .database
        .get_messages_for_game(game_id)
        .unwrap()
        .is_empty());
}

async fn cli_accept(dir: &TempDir, game_id: &str) -> std::process::Output {
    timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_mate"))
            .args(["accept", game_id, "--color", "black"])
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

fn assert_cli_failure(output: &std::process::Output, reason: &str) {
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("Game accepted successfully"), "{stdout}");
    assert!(!stdout.contains("is now active"), "{stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(reason), "{stderr}");
}

#[tokio::test]
async fn accept_remote_rejections_and_identity_errors_leave_invitation_unchanged() {
    // Cover both the remote checking the invitee identity and the invitee
    // checking the authenticated identity on accepted AND rejected responses.
    for case in [
        "abandoned",
        "wrong invitee",
        "wrong inviter",
        "blank inviter",
        "wrong declining inviter",
    ] {
        let (app, local_dir) = test_app().await;
        let remote_dir = TempDir::new().unwrap();
        let identity = Arc::new(Identity::generate().unwrap());
        let peer_id = identity.peer_id().to_string();
        let database = Arc::new(
            Database::new_with_path(&peer_id, &remote_dir.path().join("remote.sqlite")).unwrap(),
        );
        let game_id = generate_game_id();
        database
            .create_game_with_id(
                game_id.clone(),
                if case == "wrong invitee" {
                    "another-invitee"
                } else {
                    app.peer_id()
                }
                .to_string(),
                PlayerColor::White,
                None,
            )
            .unwrap();
        let abandoned = case == "abandoned" || case == "wrong declining inviter";
        if abandoned {
            database
                .update_game_status(&game_id, GameStatus::Abandoned)
                .unwrap();
        }
        let server = Server::bind("127.0.0.1:0", identity, Arc::clone(&database))
            .await
            .unwrap();
        let address = server.local_addr().unwrap().to_string();
        let task = tokio::spawn(async move { server.run().await });
        let opponent = match case {
            "wrong inviter" | "wrong declining inviter" => "another-inviter",
            "blank inviter" => "",
            _ => &peer_id,
        };
        seed_local(
            &app,
            &game_id,
            if opponent.is_empty() {
                "legacy-peer"
            } else {
                opponent
            },
            &address,
        );
        if opponent.is_empty() {
            // Simulate an old branch database; new storage APIs reject blank peers.
            app.database
                .with_connection(|conn| {
                    conn.execute(
                        "UPDATE games SET opponent_peer_id = '' WHERE id = ?1",
                        [&game_id],
                    )?;
                    Ok(())
                })
                .unwrap();
        }
        let error = timeout(
            Duration::from_secs(5),
            app.handle_accept(game_id.clone(), Some("black".to_string())),
        )
        .await
        .unwrap()
        .unwrap_err();
        let reason = match case {
            "abandoned" => "game is not pending (status: Abandoned)",
            "wrong invitee" => "peer is not the game opponent",
            _ => "does not match game opponent",
        };
        assert!(error.to_string().contains(reason), "{case}: {error}");
        assert_pending_unchanged(&app, &game_id);
        assert_eq!(
            app.network_manager
                .get_network_stats()
                .await
                .total_pending_messages,
            0
        );
        assert_cli_failure(&cli_accept(&local_dir, &game_id).await, reason);
        assert_pending_unchanged(&app, &game_id);
        if abandoned || case == "wrong invitee" {
            assert_eq!(
                database.get_game(&game_id).unwrap().status,
                if abandoned {
                    GameStatus::Abandoned
                } else {
                    GameStatus::Pending
                }
            );
            assert!(database.get_messages_for_game(&game_id).unwrap().is_empty());
        }
        task.abort();
    }
}

#[tokio::test]
async fn accept_uncorrelated_or_unexpected_responses_do_not_commit_or_queue() {
    for case in [
        "accept game ID",
        "accept color",
        "decline game ID",
        "unexpected variant",
    ] {
        let (app, dir) = test_app().await;
        let identity = Arc::new(Identity::generate().unwrap());
        let peer_id = identity.peer_id().to_string();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let game_id = generate_game_id();
        seed_local(&app, &game_id, &peer_id, &address);
        let task = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.unwrap();
                let mut connection = Connection::new(stream, Arc::clone(&identity)).await;
                connection.handle_handshake_request().await.unwrap();
                let (message, _) = connection.receive_message().await.unwrap();
                let Message::GameAccept(mut accept) = message else {
                    panic!("expected accept")
                };
                let response = match case {
                    "accept game ID" => {
                        accept.game_id = generate_game_id();
                        Message::GameAccept(accept)
                    }
                    "accept color" => {
                        accept.accepted_color = Color::White;
                        Message::GameAccept(accept)
                    }
                    "decline game ID" => {
                        Message::GameDecline(GameDecline::new_no_reason(generate_game_id()))
                    }
                    _ => Message::new_ping(1, "unexpected".to_string()),
                };
                connection.send_message(response).await.unwrap();
            }
        });
        let error = timeout(
            Duration::from_secs(5),
            app.handle_accept(game_id.clone(), Some("black".to_string())),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(
            matches!(
                error,
                CliError::Protocol(ChessProtocolError::UnexpectedMessage { .. })
            ),
            "{error}"
        );
        assert_pending_unchanged(&app, &game_id);
        assert_eq!(
            app.network_manager
                .get_network_stats()
                .await
                .total_pending_messages,
            0
        );
        assert_cli_failure(&cli_accept(&dir, &game_id).await, "Unexpected message");
        assert_pending_unchanged(&app, &game_id);
        timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
    }
}

#[tokio::test]
async fn accept_matching_authenticated_ack_activates_both_peers() {
    let (app, dir) = test_app().await;
    let remote_dir = TempDir::new().unwrap();
    let identity = Arc::new(Identity::generate().unwrap());
    let peer_id = identity.peer_id().to_string();
    let database = Arc::new(
        Database::new_with_path(&peer_id, &remote_dir.path().join("remote.sqlite")).unwrap(),
    );
    let game_id = generate_game_id();
    database
        .create_game_with_id(
            game_id.clone(),
            app.peer_id().to_string(),
            PlayerColor::Black,
            None,
        )
        .unwrap();
    let server = Server::bind("127.0.0.1:0", identity, Arc::clone(&database))
        .await
        .unwrap();
    let address = server.local_addr().unwrap().to_string();
    let task = tokio::spawn(async move { server.run().await });
    seed_local(&app, &game_id, &peer_id, &address);
    let output = cli_accept(&dir, &game_id).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Game accepted successfully"));
    for (db, color) in [
        (&app.database, PlayerColor::Black),
        (database.as_ref(), PlayerColor::White),
    ] {
        let game = db.get_game(&game_id).unwrap();
        assert_eq!(game.status, GameStatus::Active);
        assert_eq!(game.my_color, color);
        let messages = db.get_messages_for_game(&game_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message_type, "GameAccept");
        assert_eq!(
            serde_json::from_str::<GameAccept>(&messages[0].content).unwrap(),
            GameAccept::new(game_id.clone(), Color::Black)
        );
    }
    task.abort();
}

#[tokio::test]
async fn accept_transport_failure_keeps_local_invitation_pending() {
    let (app, dir) = test_app().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    drop(listener);
    let game_id = generate_game_id();
    seed_local(&app, &game_id, "offline-peer", &address);
    timeout(
        Duration::from_secs(10),
        app.handle_accept(game_id.clone(), Some("black".to_string())),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_pending_unchanged(&app, &game_id);
    assert_eq!(
        app.network_manager
            .get_network_stats()
            .await
            .total_pending_messages,
        1
    );
    assert_cli_failure(
        &cli_accept(&dir, &game_id).await,
        "Failed to send game acceptance",
    );
    assert_pending_unchanged(&app, &game_id);
}

#[tokio::test]
async fn accept_local_write_failures_never_print_success() {
    for trigger in [
        "CREATE TRIGGER fail_accept BEFORE UPDATE OF status ON games BEGIN SELECT RAISE(FAIL, 'injected status failure'); END",
        "CREATE TRIGGER fail_accept BEFORE UPDATE OF my_color ON games BEGIN SELECT RAISE(FAIL, 'injected color failure'); END",
        "CREATE TRIGGER fail_accept BEFORE INSERT ON messages BEGIN SELECT RAISE(FAIL, 'injected message failure'); END",
    ] {
        let (app, dir) = test_app().await;
        let identity = Arc::new(Identity::generate().unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let game_id = generate_game_id();
        seed_local(&app, &game_id, identity.peer_id().as_str(), &address);
        let db = rusqlite::Connection::open(app.database_path()).unwrap();
        db.execute_batch(trigger).unwrap();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut connection = Connection::new(stream, identity).await;
            connection.handle_handshake_request().await.unwrap();
            let (accept, _) = connection.receive_message().await.unwrap();
            assert!(matches!(accept, Message::GameAccept(_)));
            connection.send_message(accept).await.unwrap();
        });
        let output = cli_accept(&dir, &game_id).await;
        assert!(!output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("Game accepted successfully"), "{stdout}");
        assert!(!stdout.contains("is now active"), "{stdout}");
        assert!(app.database.get_messages_for_game(&game_id).unwrap().is_empty());
        timeout(Duration::from_secs(5), task).await.unwrap().unwrap();
    }
}
