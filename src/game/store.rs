//! Persist chess wire payloads using PascalCase `StoredMessageType` strings.
//!
//! These helpers serialize and insert only — they do not own game lifecycle
//! transitions or idempotent retry logic.

use serde::Serialize;

use crate::messages::chess::{GameAccept, GameDecline, GameInvite, Move};
use crate::storage::{Database, StorageError};

use super::StoredMessageType;

fn store_typed_message<T: Serialize>(
    database: &Database,
    game_id: String,
    message_type: StoredMessageType,
    payload: &T,
    signature: &str,
    sender_peer_id: &str,
    serialize_context: &str,
) -> Result<(), StorageError> {
    let content = serde_json::to_string(payload)
        .map_err(|e| StorageError::serialization_error(serialize_context, e))?;
    database.store_message(
        game_id,
        message_type.as_str().to_string(),
        content,
        signature.to_string(),
        sender_peer_id.to_string(),
    )?;
    Ok(())
}

/// Serialize and store a `GameInvite` row.
pub fn store_game_invite_message(
    database: &Database,
    invite: &GameInvite,
    signature: &str,
    sender_peer_id: &str,
) -> Result<(), StorageError> {
    store_typed_message(
        database,
        invite.game_id.clone(),
        StoredMessageType::GameInvite,
        invite,
        signature,
        sender_peer_id,
        "GameInvite message content",
    )
}

/// Serialize and store a `GameAccept` row.
pub fn store_game_accept_message(
    database: &Database,
    accept: &GameAccept,
    signature: &str,
    sender_peer_id: &str,
) -> Result<(), StorageError> {
    store_typed_message(
        database,
        accept.game_id.clone(),
        StoredMessageType::GameAccept,
        accept,
        signature,
        sender_peer_id,
        "GameAccept message content",
    )
}

/// Serialize and store a `Move` row.
pub fn store_game_move_message(
    database: &Database,
    mv: &Move,
    signature: &str,
    sender_peer_id: &str,
) -> Result<(), StorageError> {
    store_typed_message(
        database,
        mv.game_id.clone(),
        StoredMessageType::Move,
        mv,
        signature,
        sender_peer_id,
        "Move message content",
    )
}

/// Serialize and store a `GameDecline` row.
pub fn store_game_decline_message(
    database: &Database,
    decline: &GameDecline,
    signature: &str,
    sender_peer_id: &str,
) -> Result<(), StorageError> {
    store_typed_message(
        database,
        decline.game_id.clone(),
        StoredMessageType::GameDecline,
        decline,
        signature,
        sender_peer_id,
        "GameDecline message content",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chess::Color;
    use crate::messages::chess::generate_game_id;
    use crate::storage::models::PlayerColor;
    use tempfile::TempDir;

    fn test_db() -> Database {
        let temp_dir = TempDir::new().expect("temp dir");
        let db_path = temp_dir.path().join("store.sqlite");
        let database =
            Database::new_with_path("test-peer-store", &db_path).expect("create test database");
        // Keep DB files alive for the duration of the test process section.
        std::mem::forget(temp_dir);
        database
    }

    fn seed_game(database: &Database, game_id: &str) {
        database
            .create_game_with_id(
                game_id.to_string(),
                "opponent".to_string(),
                PlayerColor::White,
                None,
            )
            .expect("create game");
    }

    #[test]
    fn store_invite_uses_pascal_case_and_round_trips() {
        let database = test_db();
        let game_id = generate_game_id();
        seed_game(&database, &game_id);

        let invite = GameInvite::new(game_id.clone(), Some(Color::Black));
        store_game_invite_message(&database, &invite, "remote", "peer-1").expect("store invite");

        let messages = database
            .get_messages_for_game(&game_id)
            .expect("load messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].message_type,
            StoredMessageType::GameInvite.as_str()
        );
        assert_eq!(messages[0].signature, "remote");
        assert_eq!(messages[0].sender_peer_id, "peer-1");

        let parsed: GameInvite =
            serde_json::from_str(&messages[0].content).expect("parse invite content");
        assert_eq!(parsed, invite);
    }

    #[test]
    fn store_accept_uses_pascal_case_and_round_trips() {
        let database = test_db();
        let game_id = generate_game_id();
        seed_game(&database, &game_id);

        let accept = GameAccept::new(game_id.clone(), Color::White);
        store_game_accept_message(&database, &accept, "local", "self").expect("store accept");

        let messages = database
            .get_messages_for_game(&game_id)
            .expect("load messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].message_type,
            StoredMessageType::GameAccept.as_str()
        );

        let parsed: GameAccept =
            serde_json::from_str(&messages[0].content).expect("parse accept content");
        assert_eq!(parsed, accept);
    }

    #[test]
    fn store_move_uses_pascal_case_and_round_trips() {
        let database = test_db();
        let game_id = generate_game_id();
        seed_game(&database, &game_id);

        let mv = Move::new(game_id.clone(), "e2e4".to_string(), "hash-e2e4".to_string());
        store_game_move_message(&database, &mv, "remote", "peer-3").expect("store move");

        let messages = database
            .get_messages_for_game(&game_id)
            .expect("load messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].message_type,
            StoredMessageType::Move.as_str()
        );
        assert_eq!(messages[0].signature, "remote");
        assert_eq!(messages[0].sender_peer_id, "peer-3");

        let parsed: Move =
            serde_json::from_str(&messages[0].content).expect("parse move content");
        assert_eq!(parsed, mv);
    }

    #[test]
    fn store_decline_uses_pascal_case_and_round_trips() {
        let database = test_db();
        let game_id = generate_game_id();
        seed_game(&database, &game_id);

        let decline = GameDecline::new(game_id.clone(), Some("busy".to_string()));
        store_game_decline_message(&database, &decline, "remote", "peer-2").expect("store decline");

        let messages = database
            .get_messages_for_game(&game_id)
            .expect("load messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].message_type,
            StoredMessageType::GameDecline.as_str()
        );

        let parsed: GameDecline =
            serde_json::from_str(&messages[0].content).expect("parse decline content");
        assert_eq!(parsed, decline);
    }
}
