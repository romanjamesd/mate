//! Sender-side move delivery: response correlation, identity checks, typed
//! transport failures, delivery uncertainty and local commit gating.

use mate::cli::app::App;
use mate::cli::error_handler::CliError;
use mate::cli::network_manager::{
    classify_move_response, MoveProtocolError, MoveSendError, MoveSendOutcome, NetworkManager,
    SendOutcome,
};
use mate::crypto::Identity;
use mate::game::{commit_move, MoveActor, MoveProcessor};
use mate::messages::chess::{
    generate_game_id, ChessProtocolError, GameAccept, GameDecline, Move, MoveAck,
};
use mate::messages::wire::{FramedMessage, WireConfig};
use mate::messages::{Message, SignedEnvelope};
use mate::network::{Connection, ConnectionError, WireProtocolError};
use mate::storage::{Database, GameStatus, PlayerColor};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;

// =============================================================================
// Fixtures
// =============================================================================

async fn test_app() -> (App, TempDir) {
    let dir = TempDir::new().unwrap();
    let app = App::new_with_data_dir(dir.path().to_path_buf())
        .await
        .unwrap();
    (app, dir)
}

/// Active local game where the app plays White against `opponent` at `address`.
fn seed_active_game(app: &App, opponent: &str, address: &str) -> String {
    let game_id = generate_game_id();
    app.database
        .create_game_with_id(
            game_id.clone(),
            opponent.to_string(),
            PlayerColor::White,
            Some(serde_json::json!({ "dial_address": address })),
        )
        .unwrap();
    app.database
        .update_game_status(&game_id, GameStatus::Active)
        .unwrap();
    game_id
}

fn prepared_move(app: &App, game_id: &str, notation: &str) -> Move {
    MoveProcessor::new(&app.database)
        .prepare_move(game_id, notation)
        .unwrap()
        .move_message
}

fn local_moves(app: &App, game_id: &str) -> usize {
    app.database.get_moves_for_game(game_id).unwrap().len()
}

async fn pending_messages(app: &App) -> usize {
    app.network_manager
        .get_network_stats()
        .await
        .total_pending_messages
}

async fn cli_move(dir: &TempDir, game_id: &str, notation: &str) -> std::process::Output {
    timeout(
        Duration::from_secs(20),
        tokio::process::Command::new(env!("CARGO_BIN_EXE_mate"))
            .args(["move", notation, "--game-id", game_id])
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

fn assert_cli_failure_without_success(output: &std::process::Output, reason: &str) {
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains('✓'), "{stdout}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(reason), "{stderr}");
}

fn ack_for(mv: &Move) -> Message {
    Message::MoveAck(MoveAck::for_move(mv))
}

// =============================================================================
// Mock opponent speaking the raw wire protocol, so it can sign responses with
// any identity or write malformed frames.
// =============================================================================

type Respond = Box<dyn FnOnce(&Move) -> Reply + Send>;

enum Reply {
    /// Respond with `message` signed by `signer`.
    Signed {
        message: Message,
        signer: Arc<Identity>,
    },
    /// Respond with a pre-built envelope.
    Envelope(SignedEnvelope),
    /// Respond with a frame that does not decode as an envelope.
    Garbage,
    /// Close the connection after reading the move.
    Drop,
}

/// What the mock does on one accepted connection.
enum Session {
    /// Complete the handshake as `identity`, read the move, then `respond`.
    Serve {
        identity: Arc<Identity>,
        respond: Respond,
    },
    /// Close before the handshake completes.
    RefuseHandshake,
}

fn serve(
    identity: &Arc<Identity>,
    respond: impl FnOnce(&Move) -> Reply + Send + 'static,
) -> Session {
    Session::Serve {
        identity: Arc::clone(identity),
        respond: Box::new(respond),
    }
}

fn reply(identity: &Arc<Identity>, message: Message) -> Reply {
    Reply::Signed {
        message,
        signer: Arc::clone(identity),
    }
}

/// Serve `sessions` in order on `listener`, then drop it. Returns every Move received.
fn spawn_mock(listener: TcpListener, sessions: Vec<Session>) -> JoinHandle<Vec<Move>> {
    tokio::spawn(async move {
        let mut received = Vec::new();
        for session in sessions {
            let (mut stream, _) = listener.accept().await.unwrap();
            match session {
                Session::RefuseHandshake => drop(stream),
                Session::Serve { identity, respond } => {
                    if let Some(mv) = serve_session(&mut stream, &identity, respond).await {
                        received.push(mv);
                    }
                }
            }
        }
        received
    })
}

async fn serve_session(
    stream: &mut TcpStream,
    identity: &Identity,
    respond: Respond,
) -> Option<Move> {
    let framed = FramedMessage::default();
    let hello = framed
        .read_message(stream)
        .await
        .unwrap()
        .get_message()
        .unwrap();
    let pong = Message::new_pong(
        hello.get_nonce(),
        format!("HANDSHAKE_RESPONSE:{}", identity.peer_id()),
    );
    framed
        .write_message(
            stream,
            &SignedEnvelope::create(&pong, identity, None).unwrap(),
        )
        .await
        .unwrap();
    // A client that rejects our identity closes without sending a move.
    let envelope = framed.read_message(stream).await.ok()?;
    let Message::Move(mv) = envelope.get_message().unwrap() else {
        panic!("expected Move");
    };
    match respond(&mv) {
        Reply::Signed { message, signer } => {
            let envelope = SignedEnvelope::create(&message, &signer, None).unwrap();
            framed.write_message(stream, &envelope).await.unwrap();
        }
        Reply::Envelope(envelope) => framed.write_message(stream, &envelope).await.unwrap(),
        Reply::Garbage => {
            let junk = [0xA5u8; 32];
            stream
                .write_all(&(junk.len() as u32).to_be_bytes())
                .await
                .unwrap();
            stream.write_all(&junk).await.unwrap();
            stream.flush().await.unwrap();
        }
        Reply::Drop => {}
    }
    Some(mv)
}

async fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap().to_string();
    (listener, address)
}

async fn run_move(app: &App, game_id: &str, notation: &str) -> Result<(), CliError> {
    timeout(
        Duration::from_secs(20),
        app.handle_move(Some(game_id.to_string()), notation.to_string()),
    )
    .await
    .unwrap()
}

async fn mock_received(task: JoinHandle<Vec<Move>>) -> Vec<Move> {
    timeout(Duration::from_secs(20), task)
        .await
        .unwrap()
        .unwrap()
}

// =============================================================================
// Response classification
// =============================================================================

#[test]
fn move_response_classification_correlates_ack_and_peer() {
    let request = Move::new(generate_game_id(), 3, "e2e4".into(), "a".repeat(64));
    let peer = "opponent";
    let classify = |response: Message, peer_id: &str| {
        classify_move_response(
            &request,
            peer,
            SendOutcome {
                response,
                peer_id: peer_id.to_string(),
            },
        )
    };
    let ack = MoveAck::for_move(&request);

    assert_eq!(
        classify(Message::MoveAck(ack.clone()), peer).unwrap(),
        MoveSendOutcome::Acknowledged {
            ack: ack.clone(),
            peer_id: peer.to_string(),
        }
    );

    let mismatched_acks = [
        MoveAck {
            ply: 4,
            ..ack.clone()
        },
        MoveAck {
            move_digest: "b".repeat(64),
            ..ack.clone()
        },
        MoveAck {
            game_id: generate_game_id(),
            ..ack.clone()
        },
        // Correct ply and game but for a different move at that ply.
        MoveAck::for_move(&Move::new(
            request.game_id.clone(),
            3,
            "d2d4".into(),
            "a".repeat(64),
        )),
    ];
    for wrong in mismatched_acks {
        assert!(matches!(
            classify(Message::MoveAck(wrong), peer),
            Err(ChessProtocolError::UnexpectedMessage { .. })
        ));
    }

    assert!(matches!(
        classify(Message::MoveAck(ack), "someone-else"),
        Err(ChessProtocolError::SecurityViolation { .. })
    ));

    let decline = GameDecline::new(
        request.game_id.clone(),
        Some("ply gap: expected 2, got 3".into()),
    );
    assert_eq!(
        classify(Message::GameDecline(decline.clone()), peer).unwrap(),
        MoveSendOutcome::Rejected {
            decline,
            peer_id: peer.to_string(),
        }
    );

    for unexpected in [
        Message::GameDecline(GameDecline::new_no_reason(generate_game_id())),
        Message::new_pong(1, "pong".into()),
        Message::GameAccept(GameAccept::new(
            request.game_id.clone(),
            mate::chess::Color::Black,
        )),
    ] {
        assert!(matches!(
            classify(unexpected, peer),
            Err(ChessProtocolError::UnexpectedMessage { .. })
        ));
    }
}

// =============================================================================
// handle_move outcomes
// =============================================================================

#[tokio::test]
async fn move_decline_is_rejection_without_local_commit() {
    let (app, dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    let decline = |opp: &Arc<Identity>| {
        let opp = Arc::clone(opp);
        serve(&opp.clone(), move |mv: &Move| {
            reply(
                &opp,
                Message::GameDecline(GameDecline::new(
                    mv.game_id.clone(),
                    Some("ply gap: expected 2, got 1".into()),
                )),
            )
        })
    };
    let task = spawn_mock(listener, vec![decline(&opponent), decline(&opponent)]);

    let error = run_move(&app, &game_id, "e2e4").await.unwrap_err();
    match &error {
        CliError::MoveRejected { ply, reason, .. } => {
            assert_eq!(*ply, 1);
            assert_eq!(reason.as_deref(), Some("ply gap: expected 2, got 1"));
        }
        other => panic!("expected MoveRejected, got {other:?}"),
    }
    assert!(error.to_string().contains("mate board"), "{error}");
    assert_eq!(local_moves(&app, &game_id), 0);
    assert_eq!(pending_messages(&app).await, 0);

    assert_cli_failure_without_success(&cli_move(&dir, &game_id, "e2e4").await, "rejected move 1");
    assert_eq!(local_moves(&app, &game_id), 0);
    // A rejection is final: one connection per run, never retried.
    assert_eq!(mock_received(task).await.len(), 2);
}

#[tokio::test]
async fn uncorrelated_ack_is_protocol_error_without_retry_or_commit() {
    let (app, dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    let wrong_digest = |opp: &Arc<Identity>| {
        let opp = Arc::clone(opp);
        serve(&opp.clone(), move |mv: &Move| {
            let mut ack = MoveAck::for_move(mv);
            ack.move_digest = "0".repeat(64);
            reply(&opp, Message::MoveAck(ack))
        })
    };
    let task = spawn_mock(
        listener,
        vec![wrong_digest(&opponent), wrong_digest(&opponent)],
    );

    let error = run_move(&app, &game_id, "e2e4").await.unwrap_err();
    assert!(
        matches!(
            &error,
            CliError::MoveProtocolError { error, .. } if matches!(
                **error,
                MoveProtocolError::Response(ChessProtocolError::UnexpectedMessage { .. })
            )
        ),
        "{error:?}"
    );
    assert!(
        error.to_string().contains("may still have received it"),
        "{error}"
    );
    assert_eq!(local_moves(&app, &game_id), 0);
    assert_eq!(pending_messages(&app).await, 0);

    assert_cli_failure_without_success(&cli_move(&dir, &game_id, "e2e4").await, "not confirmed");
    assert_eq!(local_moves(&app, &game_id), 0);
    assert_eq!(mock_received(task).await.len(), 2);
}

#[tokio::test]
async fn lost_response_is_uncertain_delivery_and_retransmits_identical_payload() {
    let (app, dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    let expected = prepared_move(&app, &game_id, "e2e4");
    // Two attempts per run (in-process, then the CLI binary), all dropped.
    let sessions = (0..4).map(|_| serve(&opponent, |_| Reply::Drop)).collect();
    let task = spawn_mock(listener, sessions);

    let error = run_move(&app, &game_id, "e2e4").await.unwrap_err();
    match &error {
        CliError::MoveDeliveryUncertain {
            ply,
            notation,
            may_have_been_sent,
            ..
        } => {
            assert_eq!(*ply, 1);
            assert_eq!(notation, "e2e4");
            assert!(*may_have_been_sent);
        }
        other => panic!("expected MoveDeliveryUncertain, got {other:?}"),
    }
    let text = error.to_string();
    assert!(text.contains("may or may not have reached"), "{text}");
    assert!(!text.to_lowercase().contains("not delivered"), "{text}");
    assert!(
        text.contains(&format!("mate move e2e4 --game-id {game_id}")),
        "{text}"
    );
    assert_eq!(local_moves(&app, &game_id), 0);
    assert_eq!(pending_messages(&app).await, 0);

    assert_cli_failure_without_success(
        &cli_move(&dir, &game_id, "e2e4").await,
        "may or may not have reached",
    );
    assert_eq!(local_moves(&app, &game_id), 0);

    let received = mock_received(task).await;
    assert_eq!(received.len(), 4);
    assert!(received.iter().all(|mv| *mv == expected), "{received:?}");
}

#[tokio::test]
async fn correlated_ack_commits_the_sent_payload_after_retransmission() {
    let (app, _dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    let expected = prepared_move(&app, &game_id, "e2e4");
    let opp = Arc::clone(&opponent);
    let task = spawn_mock(
        listener,
        vec![
            serve(&opponent, |_| Reply::Drop),
            serve(&opponent, move |mv: &Move| reply(&opp, ack_for(mv))),
        ],
    );

    run_move(&app, &game_id, "e2e4").await.unwrap();

    let received = mock_received(task).await;
    assert_eq!(received, vec![expected.clone(), expected.clone()]);
    let rows = app.database.get_moves_for_game(&game_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].ply, Some(1));
    assert_eq!(rows[0].sender_peer_id, app.peer_id());
    assert_eq!(
        serde_json::from_str::<Move>(&rows[0].content).unwrap(),
        expected
    );
    assert_eq!(pending_messages(&app).await, 0);
}

#[tokio::test]
async fn ack_followed_by_local_persistence_failure_reports_remote_commit() {
    let (app, dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    rusqlite::Connection::open(app.database_path())
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_move BEFORE INSERT ON messages WHEN NEW.message_type = 'Move' \
             BEGIN SELECT RAISE(ABORT, 'injected move failure'); END",
        )
        .unwrap();
    let ack = |opp: &Arc<Identity>| {
        let opp = Arc::clone(opp);
        serve(&opp.clone(), move |mv: &Move| reply(&opp, ack_for(mv)))
    };
    let task = spawn_mock(listener, vec![ack(&opponent), ack(&opponent)]);

    let error = run_move(&app, &game_id, "e2e4").await.unwrap_err();
    assert!(
        matches!(
            error,
            CliError::MoveRemoteCommittedLocalFailed { ply: 1, .. }
        ),
        "{error:?}"
    );
    let text = error.to_string();
    assert!(text.contains("accepted move 1"), "{text}");
    assert!(
        text.contains("If your local history still ends before move 1"),
        "{text}"
    );
    assert_eq!(local_moves(&app, &game_id), 0);

    assert_cli_failure_without_success(&cli_move(&dir, &game_id, "e2e4").await, "accepted move 1");
    assert_eq!(local_moves(&app, &game_id), 0);
    assert_eq!(mock_received(task).await.len(), 2);
}

#[tokio::test]
async fn local_history_advanced_during_send_is_not_overwritten() {
    let (app, _dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    // Another CLI process commits a different first move while ours is in flight.
    let other = Database::new_with_path(app.peer_id(), &app.database_path()).unwrap();
    let other_move = prepared_move(&app, &game_id, "d2d4");
    let local_peer = app.peer_id().to_string();
    let opp = Arc::clone(&opponent);
    let target = game_id.clone();
    let task = spawn_mock(
        listener,
        vec![serve(&opponent, move |mv: &Move| {
            commit_move(
                &other,
                &target,
                &other_move,
                MoveActor::Local {
                    self_peer_id: &local_peer,
                },
            )
            .unwrap();
            reply(&opp, ack_for(mv))
        })],
    );

    let error = run_move(&app, &game_id, "e2e4").await.unwrap_err();
    match &error {
        CliError::MoveRemoteCommittedLocalFailed { reason, .. } => {
            assert!(reason.contains("conflict"), "{reason}")
        }
        other => panic!("expected MoveRemoteCommittedLocalFailed, got {other:?}"),
    }
    mock_received(task).await;
    let rows = app.database.get_moves_for_game(&game_id).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        serde_json::from_str::<Move>(&rows[0].content)
            .unwrap()
            .chess_move,
        "d2d4"
    );
}

#[tokio::test]
async fn uncertain_delivery_converges_when_the_same_move_is_rerun() {
    let (app, _dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let remote_dir = TempDir::new().unwrap();
    let remote = Arc::new(
        Database::new_with_path(
            opponent.peer_id().as_str(),
            &remote_dir.path().join("remote.sqlite"),
        )
        .unwrap(),
    );
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    remote
        .create_game_with_id(
            game_id.clone(),
            app.peer_id().to_string(),
            PlayerColor::Black,
            None,
        )
        .unwrap();
    remote
        .update_game_status(&game_id, GameStatus::Active)
        .unwrap();

    // The opponent commits authoritatively every time, but loses the first
    // run's responses (both attempts) and answers the re-run.
    let local_peer = app.peer_id().to_string();
    let outcomes = Arc::new(Mutex::new(Vec::new()));
    let sessions = [false, false, true]
        .into_iter()
        .map(|answer| {
            let remote = Arc::clone(&remote);
            let local_peer = local_peer.clone();
            let outcomes = Arc::clone(&outcomes);
            let opp = Arc::clone(&opponent);
            serve(&opponent, move |mv: &Move| {
                let outcome = commit_move(
                    &remote,
                    &mv.game_id,
                    mv,
                    MoveActor::Opponent {
                        peer_id: &local_peer,
                    },
                )
                .unwrap();
                outcomes.lock().unwrap().push(outcome);
                if answer {
                    reply(&opp, ack_for(mv))
                } else {
                    Reply::Drop
                }
            })
        })
        .collect();
    let task = spawn_mock(listener, sessions);

    let first = run_move(&app, &game_id, "e2e4").await.unwrap_err();
    assert!(
        matches!(first, CliError::MoveDeliveryUncertain { .. }),
        "{first:?}"
    );
    assert_eq!(local_moves(&app, &game_id), 0);
    assert_eq!(remote.get_moves_for_game(&game_id).unwrap().len(), 1);

    run_move(&app, &game_id, "e2e4").await.unwrap();

    let received = mock_received(task).await;
    assert_eq!(received.len(), 3);
    assert!(received.iter().all(|mv| *mv == received[0]));
    assert_eq!(
        *outcomes.lock().unwrap(),
        vec![
            mate::game::CommitOutcome::Committed,
            mate::game::CommitOutcome::AlreadyCommitted,
            mate::game::CommitOutcome::AlreadyCommitted,
        ]
    );
    let local = app.database.get_moves_for_game(&game_id).unwrap();
    let remote_rows = remote.get_moves_for_game(&game_id).unwrap();
    assert_eq!(local.len(), 1);
    assert_eq!(remote_rows.len(), 1);
    assert_eq!(local[0].content, remote_rows[0].content);
    assert_eq!(
        serde_json::from_str::<Move>(&local[0].content).unwrap(),
        received[0]
    );
}

// =============================================================================
// Identity and response verification (handle_move and send_chess_move)
// =============================================================================

fn move_for(game_id: &str) -> Move {
    let board = mate::chess::Board::new();
    let mut after = board.clone();
    let chess_move = mate::chess::Move::from_str_with_color("e2e4", board.active_color()).unwrap();
    after.make_move(chess_move).unwrap();
    Move::new(
        game_id.to_string(),
        1,
        "e2e4".into(),
        mate::messages::chess::hash_board_state(&after),
    )
}

async fn send(
    manager: &NetworkManager,
    address: &str,
    opponent: &Identity,
    mv: &Move,
) -> Result<MoveSendOutcome, MoveSendError> {
    timeout(
        Duration::from_secs(20),
        manager.send_chess_move(address, opponent.peer_id().as_str(), mv.clone()),
    )
    .await
    .unwrap()
}

fn manager() -> NetworkManager {
    NetworkManager::new(Arc::new(Identity::generate().unwrap()))
}

#[tokio::test]
async fn correctly_signed_ack_from_another_identity_is_signer_mismatch() {
    let (app, _dir) = test_app().await;
    let opponent = Arc::new(Identity::generate().unwrap());
    let impostor = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
    let mv = prepared_move(&app, &game_id, "e2e4");
    let imp = Arc::clone(&impostor);
    let task = spawn_mock(
        listener,
        vec![serve(&opponent, move |mv: &Move| reply(&imp, ack_for(mv)))],
    );

    let error = run_move(&app, &game_id, "e2e4").await.unwrap_err();
    let diagnostic = error.to_string();
    assert!(diagnostic.contains("was not saved locally and was not retried"));
    assert!(diagnostic.contains("Your opponent may still have received it"));
    let CliError::MoveProtocolError { error, .. } = error else {
        panic!("expected MoveProtocolError, got {error:?}");
    };
    match *error {
        MoveProtocolError::SignerMismatch { expected, actual } => {
            assert_eq!(expected, opponent.peer_id().as_str());
            assert_eq!(actual, impostor.peer_id().as_str());
        }
        other => panic!("expected SignerMismatch, got {other:?}"),
    }
    // Terminal: the single session is all the mock serves; no retry was attempted.
    assert_eq!(mock_received(task).await, vec![mv]);
    assert_eq!(local_moves(&app, &game_id), 0);
    assert_eq!(pending_messages(&app).await, 0);
}

#[tokio::test]
async fn unverifiable_responses_are_terminal_connection_errors() {
    for case in ["bad signature", "stale timestamp", "garbage frame"] {
        let (app, _dir) = test_app().await;
        let opponent = Arc::new(Identity::generate().unwrap());
        let (listener, address) = listener().await;
        let game_id = seed_active_game(&app, opponent.peer_id().as_str(), &address);
        let mv = prepared_move(&app, &game_id, "e2e4");
        let opp = Arc::clone(&opponent);
        let task = spawn_mock(
            listener,
            vec![serve(&opponent, move |mv: &Move| match case {
                "bad signature" => {
                    let mut envelope = SignedEnvelope::create(&ack_for(mv), &opp, None).unwrap();
                    envelope.signature[0] ^= 0xFF;
                    Reply::Envelope(envelope)
                }
                "stale timestamp" => Reply::Envelope(
                    SignedEnvelope::create(&ack_for(mv), &opp, Some(1_000)).unwrap(),
                ),
                _ => Reply::Garbage,
            })],
        );

        let error = run_move(&app, &game_id, "e2e4").await.unwrap_err();
        let CliError::MoveProtocolError { error, .. } = error else {
            panic!("{case}: expected MoveProtocolError, got {error:?}");
        };
        let MoveProtocolError::Connection(connection) = *error else {
            panic!("{case}: expected Protocol(Connection), got {error:?}");
        };
        match case {
            "bad signature" => assert!(
                matches!(connection, ConnectionError::InvalidSignature),
                "{case}: {connection:?}"
            ),
            "stale timestamp" => assert!(
                matches!(connection, ConnectionError::InvalidTimestamp),
                "{case}: {connection:?}"
            ),
            _ => assert!(
                matches!(
                    connection,
                    ConnectionError::WireProtocol(WireProtocolError::CorruptedData { .. })
                ),
                "{case}: {connection:?}"
            ),
        }
        assert_eq!(mock_received(task).await, vec![mv], "{case}");
        assert_eq!(local_moves(&app, &game_id), 0, "{case}");
        assert_eq!(pending_messages(&app).await, 0, "{case}");
    }
}

#[tokio::test]
async fn unexpected_handshake_identity_sends_nothing() {
    let opponent = Arc::new(Identity::generate().unwrap());
    let impostor = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let mv = move_for(&generate_game_id());
    let task = spawn_mock(listener, vec![serve(&impostor, |_| Reply::Drop)]);

    let error = send(&manager(), &address, &opponent, &mv)
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            MoveSendError::Protocol(MoveProtocolError::PeerMismatch { .. })
        ),
        "{error:?}"
    );
    assert!(mock_received(task).await.is_empty());
}

#[tokio::test]
async fn unexpected_identity_on_reconnect_stops_without_resending() {
    let opponent = Arc::new(Identity::generate().unwrap());
    let impostor = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let mv = move_for(&generate_game_id());
    let task = spawn_mock(
        listener,
        vec![
            serve(&opponent, |_| Reply::Drop),
            serve(&impostor, |_| Reply::Drop),
        ],
    );

    let error = send(&manager(), &address, &opponent, &mv)
        .await
        .unwrap_err();
    match error {
        MoveSendError::Protocol(MoveProtocolError::PeerMismatch { expected, actual }) => {
            assert_eq!(expected, opponent.peer_id().as_str());
            assert_eq!(actual, impostor.peer_id().as_str());
        }
        other => panic!("expected PeerMismatch, got {other:?}"),
    }
    assert_eq!(mock_received(task).await, vec![mv]);
}

// =============================================================================
// Delivery tracking
// =============================================================================

#[tokio::test]
async fn failed_move_write_reports_may_have_been_sent() {
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let mut mv = move_for(&generate_game_id());
    // Force send_message's framing write to fail deterministically after the
    // handshake. A TCP reset can instead race with a successful buffered write
    // and exercise the receive-error path, leaving this regression untested.
    mv.board_state_hash = "a".repeat(WireConfig::for_network().max_message_size);
    let task = spawn_mock(listener, vec![serve(&opponent, |_| Reply::Drop)]);
    let manager = manager();

    let error = send(&manager, &address, &opponent, &mv).await.unwrap_err();
    match error {
        MoveSendError::Delivery {
            may_have_been_sent,
            source,
        } => {
            assert!(may_have_been_sent);
            assert!(
                matches!(
                    source.downcast_ref::<ConnectionError>(),
                    Some(ConnectionError::WireProtocol(
                        WireProtocolError::MessageTooLarge { .. }
                    ))
                ),
                "{source:?}"
            );
        }
        other => panic!("expected Delivery, got {other:?}"),
    }
    assert!(mock_received(task).await.is_empty());
    assert_eq!(manager.get_network_stats().await.total_pending_messages, 0);
}

#[tokio::test]
async fn connection_failures_before_any_send_report_not_sent() {
    let opponent = Identity::generate().unwrap();
    let mv = move_for(&generate_game_id());

    // Nothing listening.
    let (closed, address) = listener().await;
    drop(closed);
    match send(&manager(), &address, &opponent, &mv)
        .await
        .unwrap_err()
    {
        MoveSendError::Delivery {
            may_have_been_sent, ..
        } => assert!(!may_have_been_sent),
        other => panic!("expected Delivery, got {other:?}"),
    }

    // TCP connects but every handshake is cut off (client retries internally).
    let (listener, address) = listener().await;
    let task = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                break;
            };
            drop(stream);
        }
    });
    match send(&manager(), &address, &opponent, &mv)
        .await
        .unwrap_err()
    {
        MoveSendError::Delivery {
            may_have_been_sent, ..
        } => assert!(!may_have_been_sent),
        other => panic!("expected Delivery, got {other:?}"),
    }
    task.abort();
}

#[tokio::test]
async fn later_connection_failure_keeps_may_have_been_sent() {
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let mv = move_for(&generate_game_id());
    // One session that reads the move and drops it; then the listener closes,
    // so the reconnect fails before the handshake.
    let task = spawn_mock(listener, vec![serve(&opponent, |_| Reply::Drop)]);

    let error = send(&manager(), &address, &opponent, &mv)
        .await
        .unwrap_err();
    match error {
        MoveSendError::Delivery {
            may_have_been_sent, ..
        } => assert!(may_have_been_sent),
        other => panic!("expected Delivery, got {other:?}"),
    }
    assert_eq!(mock_received(task).await, vec![mv]);
}

#[tokio::test]
async fn refused_handshake_then_lost_response_reports_sent() {
    let opponent = Arc::new(Identity::generate().unwrap());
    let (listener, address) = listener().await;
    let mv = move_for(&generate_game_id());
    // First attempt: handshake refused (connect retries internally), then the
    // move is read and its response lost on every remaining connection.
    let task = spawn_mock(
        listener,
        vec![
            Session::RefuseHandshake,
            serve(&opponent, |_| Reply::Drop),
            serve(&opponent, |_| Reply::Drop),
        ],
    );

    match send(&manager(), &address, &opponent, &mv)
        .await
        .unwrap_err()
    {
        MoveSendError::Delivery {
            may_have_been_sent, ..
        } => assert!(may_have_been_sent),
        other => panic!("expected Delivery, got {other:?}"),
    }
    assert_eq!(mock_received(task).await, vec![mv.clone(), mv]);
}

// =============================================================================
// Typed wire errors through Connection
// =============================================================================

/// Make dropping `stream` send RST instead of FIN.
#[allow(deprecated)]
fn reset_on_drop(stream: &TcpStream) {
    stream.set_linger(Some(Duration::ZERO)).unwrap();
}

async fn connected_pair() -> (Connection, TcpStream) {
    let (listener, address) = listener().await;
    let client = TcpStream::connect(&address).await.unwrap();
    let (server, _) = listener.accept().await.unwrap();
    let connection = Connection::new(client, Arc::new(Identity::generate().unwrap())).await;
    (connection, server)
}

#[tokio::test]
async fn connection_read_failures_keep_their_wire_type() {
    // Peer closes at a frame boundary.
    let (mut connection, server) = connected_pair().await;
    drop(server);
    let error = connection.receive_message().await.unwrap_err();
    assert!(
        matches!(
            error,
            ConnectionError::WireProtocol(WireProtocolError::UnexpectedEof { .. })
        ),
        "{error:?}"
    );

    // Peer closes mid-frame.
    let (mut connection, mut server) = connected_pair().await;
    server.write_all(&64u32.to_be_bytes()).await.unwrap();
    server.write_all(&[0u8; 10]).await.unwrap();
    drop(server);
    let error = connection.receive_message().await.unwrap_err();
    assert!(
        matches!(
            error,
            ConnectionError::WireProtocol(WireProtocolError::UnexpectedEof { .. })
        ),
        "{error:?}"
    );

    // Peer resets the connection.
    let (mut connection, server) = connected_pair().await;
    reset_on_drop(&server);
    drop(server);
    tokio::time::sleep(Duration::from_millis(50)).await;
    let error = connection.receive_message().await.unwrap_err();
    assert!(
        matches!(
            &error,
            ConnectionError::WireProtocol(WireProtocolError::Io(io))
                if io.kind() == std::io::ErrorKind::ConnectionReset
        ) || matches!(
            error,
            ConnectionError::WireProtocol(WireProtocolError::UnexpectedEof { .. })
        ),
        "{error:?}"
    );

    // Peer stays silent past the read timeout.
    let (listener, address) = listener().await;
    let client = TcpStream::connect(&address).await.unwrap();
    let (_server, _) = listener.accept().await.unwrap();
    let config = WireConfig {
        read_timeout: Duration::from_millis(100),
        ..WireConfig::default()
    };
    let mut connection =
        Connection::new_with_config(client, Arc::new(Identity::generate().unwrap()), config).await;
    let error = connection.receive_message().await.unwrap_err();
    assert!(
        matches!(
            error,
            ConnectionError::WireProtocol(WireProtocolError::Timeout(_))
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn connection_write_failures_keep_their_wire_type() {
    let (mut connection, server) = connected_pair().await;
    reset_on_drop(&server);
    drop(server);
    tokio::time::sleep(Duration::from_millis(50)).await;
    // The first write can land in the socket buffer before the reset is seen.
    let mut error = None;
    for _ in 0..10 {
        if let Err(e) = connection
            .send_message(Message::new_ping(1, "x".repeat(1024)))
            .await
        {
            error = Some(e);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let error = error.expect("writes to a reset peer must eventually fail");
    assert!(
        matches!(
            &error,
            ConnectionError::WireProtocol(
                WireProtocolError::Io(_) | WireProtocolError::ConnectionClosed { .. }
            )
        ),
        "{error:?}"
    );
}
