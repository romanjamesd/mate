//! Non-Move callers must terminate on a verified signer change, without retrying.
use mate::chess::Color;
use mate::cli::app::App;
use mate::cli::error_handler::CliError;
use mate::crypto::Identity;
use mate::messages::chess::generate_game_id;
use mate::messages::wire::FramedMessage;
use mate::messages::{Message, SignedEnvelope};
use mate::network::{Client, ConnectionError};
use mate::storage::{GameStatus, PlayerColor};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::time::timeout;

// Keep the socket raw so the handshake and response can use different signers.
// The caller joins this future, so assertion failures cannot leave a detached task.
async fn wrong_signer(listener: &TcpListener, peer: &Identity, other: &Identity) -> Message {
    let (mut stream, _) = listener.accept().await.unwrap();
    let framing = FramedMessage::default();
    let hello = framing.read_message(&mut stream).await.unwrap();
    assert!(hello.verify_signature());
    let pong = Message::new_pong(
        hello.get_message().unwrap().get_nonce(),
        format!("HANDSHAKE_RESPONSE:{}", peer.peer_id()),
    );
    framing
        .write_message(
            &mut stream,
            &SignedEnvelope::create(&pong, peer, None).unwrap(),
        )
        .await
        .unwrap();
    let request = framing.read_message(&mut stream).await.unwrap();
    assert!(request.verify_signature());
    let request = request.get_message().unwrap();
    let response = match &request {
        Message::Ping { .. } => {
            Message::new_pong(request.get_nonce(), request.get_payload().to_string())
        }
        _ => request.clone(),
    };
    framing
        .write_message(
            &mut stream,
            &SignedEnvelope::create(&response, other, None).unwrap(),
        )
        .await
        .unwrap();
    assert!(
        framing.read_message(&mut stream).await.is_err(),
        "no further request may be sent on the compromised connection"
    );
    request
}

async fn no_reconnect(listener: &TcpListener) {
    assert!(
        timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err(),
        "signer mismatch must not reconnect or resend"
    );
}

fn assert_mismatch(error: CliError, peer: &Identity, other: &Identity) {
    let CliError::Connection(ConnectionError::PeerIdentityMismatch { expected, actual }) = error
    else {
        panic!("expected typed signer mismatch, got {error:?}");
    };
    assert_eq!(expected, peer.peer_id().as_str());
    assert_eq!(actual, other.peer_id().as_str());
}

#[tokio::test]
async fn invite_and_accept_mismatch_preserve_pending_state_and_are_not_queued() {
    for accept in [false, true] {
        let dir = tempfile::TempDir::new().unwrap();
        let app = App::new_with_data_dir(dir.path().to_path_buf())
            .await
            .unwrap();
        let peer = Identity::generate().unwrap();
        let other = Identity::generate().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let game_id = generate_game_id();
        if accept {
            app.database
                .create_game_with_id(
                    game_id.clone(),
                    peer.peer_id().to_string(),
                    PlayerColor::White,
                    Some(serde_json::json!({"dial_address": address})),
                )
                .unwrap();
        }
        let before_game = if accept {
            Some(serde_json::to_value(app.database.get_game(&game_id).unwrap()).unwrap())
        } else {
            None
        };
        let operation = async {
            let error = if accept {
                app.handle_accept(game_id.clone(), Some("black".into()))
                    .await
                    .unwrap_err()
            } else {
                app.handle_invite(address.clone(), Some("black".into()))
                    .await
                    .unwrap_err()
            };
            assert_mismatch(error, &peer, &other);
        };
        let (_, request) = timeout(Duration::from_secs(10), async {
            tokio::join!(operation, wrong_signer(&listener, &peer, &other))
        })
        .await
        .unwrap();
        no_reconnect(&listener).await;
        let games = app
            .database
            .get_games_by_status(GameStatus::Pending)
            .unwrap();
        assert_eq!(games.len(), 1);
        let game = &games[0];
        assert_eq!(game.opponent_peer_id, peer.peer_id().as_str());
        if accept {
            assert!(matches!(request, Message::GameAccept(_)));
            assert_eq!(serde_json::to_value(game).unwrap(), before_game.unwrap());
            assert!(app.database.get_recent_messages(10).unwrap().is_empty());
        } else {
            let Message::GameInvite(invite) = request else {
                panic!("expected invitation")
            };
            assert_eq!(invite.suggested_color, Some(Color::Black));
            assert_eq!(game.id, invite.game_id);
            // Only the pre-send persisted invite exists; no response is recorded.
            let messages = app.database.get_recent_messages(10).unwrap();
            assert_eq!(messages.len(), 1);
            assert_eq!(messages[0].sender_peer_id, app.peer_id());
            assert_eq!(messages[0].message_type, "GameInvite");
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&messages[0].content).unwrap(),
                serde_json::to_value(invite).unwrap()
            );
        }
        assert_eq!(
            app.network_manager
                .get_network_stats()
                .await
                .total_pending_messages,
            0
        );
    }
}

#[tokio::test]
async fn client_echo_and_one_shot_surface_typed_mismatch_without_more_messages() {
    for echo in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let peer = Identity::generate().unwrap();
        let other = Identity::generate().unwrap();
        let operation = async {
            let client = Client::new(Arc::new(Identity::generate().unwrap()));
            let error = if echo {
                let mut conn = client.connect(&address).await.unwrap();
                client.echo_session(&mut conn).await.unwrap_err()
            } else {
                client
                    .send_message_to(&address, Message::new_ping(42, "hello".into()))
                    .await
                    .unwrap_err()
            };
            assert_mismatch(
                mate::cli::error_handler::cli_error_from_anyhow(error),
                &peer,
                &other,
            );
        };
        timeout(Duration::from_secs(10), async {
            tokio::join!(operation, wrong_signer(&listener, &peer, &other))
        })
        .await
        .unwrap();
        no_reconnect(&listener).await;
    }
}

#[tokio::test]
async fn interactive_mismatch_ends_session_without_reconnect_or_second_input() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let peer = Identity::generate().unwrap();
    let other = Identity::generate().unwrap();
    let dir = tempfile::TempDir::new().unwrap();
    let operation = async {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_mate"))
            .args(["connect", &address])
            .env("MATE_DATA_DIR", dir.path())
            .env("MATE_CONFIG_DIR", dir.path().join("config"))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"hello\nsecond message\n")
            .await
            .unwrap();
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Ending session; the request may have been received."),
            "{stdout}"
        );
        assert!(!stdout.contains("Attempting to reconnect"), "{stdout}");
        assert!(!stdout.contains("Received echo"), "{stdout}");
    };
    timeout(Duration::from_secs(10), async {
        tokio::join!(operation, wrong_signer(&listener, &peer, &other))
    })
    .await
    .unwrap();
    no_reconnect(&listener).await;
}
