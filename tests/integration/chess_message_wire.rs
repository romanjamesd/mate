//! Regression tests for the connection-layer panic on chess messages.
//!
//! See `CONNECTION_LAYER_PANIC.md` for the full root-cause analysis. In short:
//! `Message::get_nonce()`/`get_payload()` panic for every chess variant, but
//! several generic send/receive code paths call them unconditionally on any
//! `Message`. These tests exercise those exact code paths end-to-end with
//! real chess messages, asserting only on public, black-box outcomes
//! (`Result::is_ok()`/`is_err()`, `message_type()`, `get_game_id()`) so that
//! any valid fix satisfies them - none of these tests assert on log text or
//! exact panic messages, and none hardcode a single chess variant.
//!
//! Written before any `src/` change per the project's Step 0 process: at the
//! time of writing, the round-trip and client tests below panic, and the
//! handshake tests panic one layer up inside `receive_message`.

use anyhow::Context;
use mate::chess::{Board, Color};
use mate::crypto::Identity;
use mate::messages::chess::{generate_game_id, hash_board_state};
use mate::messages::{FramedMessage, Message, SignedEnvelope};
use mate::network::{Client, Connection};
use std::sync::{Arc, Once};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::timeout;

static TRACING_INIT: Once = Once::new();
const TEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Ensure a real `tracing` subscriber is installed at `DEBUG` level.
///
/// This is load-bearing, not cosmetic: `tracing`'s macros skip evaluating
/// their argument expressions entirely when there is no global subscriber
/// registered (which is the default in a plain `cargo test` run). Since the
/// bug these tests guard against only manifests when a panicking accessor is
/// actually evaluated inside a `debug!`/`info!` call, these tests would
/// silently pass regardless of whether the bug exists unless a subscriber
/// that enables at least `DEBUG` is active. This mirrors the manual repro's
/// `RUST_LOG=debug` and exercises every affected call site deterministically,
/// without depending on the ambient environment.
fn init_test_tracing() {
    TRACING_INIT.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("chess_message_wire test target must control the global tracing subscriber");
    });

    assert!(
        tracing::enabled!(
            target: "mate::network::connection",
            tracing::Level::DEBUG
        ),
        "connection DEBUG events must be enabled for panic regression coverage"
    );
}

/// Build one instance of every chess `Message` variant, all sharing `game_id`,
/// so tests can assert round-tripping via `get_game_id()` without depending on
/// any single variant's internal shape.
fn all_chess_test_variants(game_id: &str) -> Vec<Message> {
    let board = Board::new();
    vec![
        Message::new_game_invite(game_id.to_string(), Some(Color::White)),
        Message::new_game_accept(game_id.to_string(), Color::Black),
        Message::new_game_decline(game_id.to_string(), Some("busy".to_string())),
        Message::new_move(
            game_id.to_string(),
            "e2e4".to_string(),
            hash_board_state(&board),
        ),
        Message::new_move_ack(game_id.to_string(), Some("move-1".to_string())),
        Message::new_sync_request(game_id.to_string()),
        Message::new_sync_response(
            game_id.to_string(),
            board.to_fen(),
            vec!["e2e4".to_string(), "e7e5".to_string()],
            hash_board_state(&board),
        ),
    ]
}

/// Write a valid signed message directly through the wire layer.
///
/// Handshake rejection fixtures use this instead of `Connection::send_message`
/// so setup cannot fail in the generic connection logging path that the tests
/// are intended to exercise on receipt.
async fn write_signed_message(
    stream: &mut TcpStream,
    identity: &Identity,
    message: &Message,
) -> anyhow::Result<()> {
    let envelope =
        SignedEnvelope::create(message, identity, None).context("create signed envelope failed")?;
    FramedMessage::default()
        .write_message_with_default_timeout(stream, &envelope)
        .await
        .context("write signed envelope failed")
}

/// Build a pair of raw, unauthenticated `Connection`s over a real loopback
/// TCP socket. `Connection::send_message`/`receive_message` don't require a
/// prior handshake (only the `SignedEnvelope`'s own embedded sender is
/// checked), so this is sufficient to exercise those two methods directly.
async fn build_raw_connection_pair() -> (Connection, Connection) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind loopback listener");
    let addr = listener.local_addr().expect("failed to get local addr");

    let identity_a = Arc::new(Identity::generate().expect("failed to generate identity"));
    let identity_b = Arc::new(Identity::generate().expect("failed to generate identity"));

    let (accept_result, connect_result) = tokio::join!(listener.accept(), TcpStream::connect(addr));
    let (accepted_stream, _) = accept_result.expect("failed to accept connection");
    let connected_stream = connect_result.expect("failed to connect");

    let side_a = Connection::new(connected_stream, identity_a).await;
    let side_b = Connection::new(accepted_stream, identity_b).await;
    (side_a, side_b)
}

/// Minimal test-only fixture that authenticates via the real handshake
/// protocol (unaffected by this bug - it only ever exchanges Ping/Pong), then
/// echoes back whatever single message it receives, regardless of type. The
/// production `Server` only ever echoes `Ping`, so a custom fixture is needed
/// to exercise the response-handling side of `Client::send_message_to` with a
/// chess message.
async fn spawn_echo_any_fixture(
    identity: Arc<Identity>,
) -> (String, tokio::task::JoinHandle<anyhow::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind loopback listener");
    let addr = listener.local_addr().expect("failed to get local addr");

    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.context("accept failed")?;
        let mut connection = Connection::new(stream, identity).await;
        connection
            .handle_handshake_request()
            .await
            .context("handshake failed")?;
        let (message, _sender) = connection
            .receive_message()
            .await
            .context("receive failed")?;
        connection
            .send_message(message)
            .await
            .context("echo send failed")?;
        Ok(())
    });

    (addr.to_string(), handle)
}

/// Wait for a fixture task without allowing an internal network timeout to
/// stall the test. Abort timed-out fixtures so they cannot continue running
/// after the test has failed.
async fn await_fixture(handle: JoinHandle<anyhow::Result<()>>, fixture_name: &str) {
    let mut handle = handle;
    let join_result = match timeout(TEST_TIMEOUT, &mut handle).await {
        Ok(join_result) => join_result,
        Err(_) => {
            handle.abort();
            panic!(
                "{} fixture task timed out after {:?}",
                fixture_name, TEST_TIMEOUT
            );
        }
    };

    join_result
        .unwrap_or_else(|error| panic!("{} fixture task panicked: {}", fixture_name, error))
        .unwrap_or_else(|error| {
            panic!(
                "{} fixture task returned an error: {:#}",
                fixture_name, error
            )
        });
}

/// Test for Steps 1 & 2: sending and receiving every chess variant over a
/// real `Connection` pair must succeed and round-trip correctly.
///
/// Run today (pre-fix): panics on `send_message` (call site #1 in
/// `Connection::send_message`).
#[tokio::test]
async fn test_connection_send_receive_all_chess_variants_round_trip() {
    init_test_tracing();
    let (mut side_a, mut side_b) = build_raw_connection_pair().await;
    let game_id = generate_game_id();

    for msg in all_chess_test_variants(&game_id) {
        let expected_type = msg.message_type();
        let expected_game_id = msg.get_game_id().map(|s| s.to_string());

        let send_result = side_a.send_message(msg).await;
        assert!(
            send_result.is_ok(),
            "send_message should succeed for {}: {:?}",
            expected_type,
            send_result.err()
        );

        let receive_result = timeout(TEST_TIMEOUT, side_b.receive_message())
            .await
            .unwrap_or_else(|_| panic!("receive_message timed out for {}", expected_type));
        assert!(
            receive_result.is_ok(),
            "receive_message should succeed for {}: {:?}",
            expected_type,
            receive_result.err()
        );

        let (received_message, _sender) = receive_result.unwrap();
        assert_eq!(received_message.message_type(), expected_type);
        assert_eq!(
            received_message.get_game_id().map(|s| s.to_string()),
            expected_game_id
        );
    }
}

/// Test for Step 3: sending every chess variant through
/// `Client::send_message_to` (the one-shot connect+send+receive helper) must
/// succeed and round-trip correctly.
///
/// Run today (pre-fix): panics (call site #3 if `RUST_LOG=debug`, otherwise
/// call site #4 once the echo response arrives).
#[tokio::test]
async fn test_client_send_message_to_chess_variant_does_not_panic() {
    init_test_tracing();
    let server_identity = Arc::new(Identity::generate().expect("failed to generate identity"));
    let client_identity = Arc::new(Identity::generate().expect("failed to generate identity"));
    let game_id = generate_game_id();

    for msg in all_chess_test_variants(&game_id) {
        let expected_type = msg.message_type();
        let expected_game_id = msg.get_game_id().map(|s| s.to_string());

        let (addr, fixture_handle) = spawn_echo_any_fixture(server_identity.clone()).await;

        let client = Client::new(client_identity.clone());
        let response = timeout(TEST_TIMEOUT, client.send_message_to(&addr, msg))
            .await
            .unwrap_or_else(|_| panic!("send_message_to timed out for {}", expected_type));

        assert!(
            response.is_ok(),
            "send_message_to should succeed for {}: {:?}",
            expected_type,
            response.err()
        );

        let response_message = response.unwrap();
        assert_eq!(response_message.message_type(), expected_type);
        assert_eq!(
            response_message.get_game_id().map(|s| s.to_string()),
            expected_game_id
        );

        await_fixture(fixture_handle, &format!("echo ({expected_type})")).await;
    }
}

/// Test for Step 4 (client side): if a peer responds to a handshake `Ping`
/// with a chess message instead of the expected `Pong`, `handshake()` must
/// return a clean `Err`, not panic.
///
/// Run today (pre-fix): panics one layer down, inside `receive_message`
/// (call site #2), before the handshake's own variant check is ever reached.
#[tokio::test]
async fn test_handshake_rejects_non_pong_response() {
    init_test_tracing();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind loopback listener");
    let addr = listener.local_addr().expect("failed to get local addr");

    let peer_identity = Arc::new(Identity::generate().expect("failed to generate identity"));
    let peer_handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.context("accept failed")?;
        let framed = FramedMessage::default();

        // Consume the client's handshake Ping request...
        let request_envelope = framed
            .read_message_with_default_timeout(&mut stream)
            .await
            .context("receive handshake ping failed")?;
        anyhow::ensure!(
            request_envelope.verify_signature(),
            "handshake Ping signature was invalid"
        );
        let request_message = request_envelope
            .get_message()
            .context("deserialize handshake ping failed")?;
        anyhow::ensure!(
            request_message.is_ping(),
            "expected handshake Ping, got {}",
            request_message.message_type()
        );

        // ...but respond with a chess message instead of the expected Pong.
        let bogus_response = Message::new_game_invite(generate_game_id(), None);
        write_signed_message(&mut stream, peer_identity.as_ref(), &bogus_response)
            .await
            .context("send bogus response failed")?;
        anyhow::Ok(())
    });

    let client_identity = Arc::new(Identity::generate().expect("failed to generate identity"));
    let client_stream = TcpStream::connect(addr).await.expect("failed to connect");
    let mut client_connection = Connection::new(client_stream, client_identity).await;

    let handshake_result = timeout(TEST_TIMEOUT, client_connection.handshake())
        .await
        .expect("handshake() should not hang when the peer sends an unexpected message type");

    assert!(
        handshake_result.is_err(),
        "handshake() should return an Err (not succeed) when the peer responds with a non-Pong message"
    );

    await_fixture(peer_handle, "handshake peer").await;
}

/// Test for Step 4 (server side): if a peer sends a chess message instead of
/// the expected handshake `Ping`, `handle_handshake_request()` must return a
/// clean `Err`, not panic.
///
/// Run today (pre-fix): panics one layer down, inside `receive_message`
/// (call site #2), before the handshake's own variant check is ever reached.
#[tokio::test]
async fn test_handshake_request_rejects_non_ping() {
    init_test_tracing();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind loopback listener");
    let addr = listener.local_addr().expect("failed to get local addr");

    let client_identity = Arc::new(Identity::generate().expect("failed to generate identity"));
    let client_handle = tokio::spawn(async move {
        let mut stream = TcpStream::connect(addr).await.context("connect failed")?;

        // Send a chess message instead of the expected handshake Ping.
        let bogus_request = Message::new_sync_request(generate_game_id());
        write_signed_message(&mut stream, client_identity.as_ref(), &bogus_request)
            .await
            .context("send bogus request failed")?;
        anyhow::Ok(())
    });

    let server_identity = Arc::new(Identity::generate().expect("failed to generate identity"));
    let (server_stream, _) = listener
        .accept()
        .await
        .expect("failed to accept connection");
    let mut server_connection = Connection::new(server_stream, server_identity).await;

    let handshake_result = timeout(TEST_TIMEOUT, server_connection.handle_handshake_request())
    .await
    .expect(
        "handle_handshake_request() should not hang when the peer sends an unexpected message type",
    );

    assert!(
        handshake_result.is_err(),
        "handle_handshake_request() should return an Err (not succeed) when the peer sends a non-Ping message"
    );

    await_fixture(client_handle, "handshake client").await;
}
