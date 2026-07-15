//! Server chess handler integration: GameInvite → pending row + echo reply

use crate::common::test_helpers::test_server_database;
use mate::chess::Color;
use mate::crypto::Identity;
use mate::messages::chess::{generate_game_id, GameInvite};
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
    assert!(receive.is_ok(), "should receive invite reply before timeout");
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
