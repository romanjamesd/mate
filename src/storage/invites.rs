use crate::messages::chess::GameInvite;
use crate::storage::{Database, Game, GameStatus, PlayerColor, StorageError};
use rusqlite::OptionalExtension;

impl Database {
    /// Commit a new outbound invitation before any payload is sent.
    pub fn create_outbound_invite(
        &self,
        invite: &GameInvite,
        opponent: &str,
        my_color: PlayerColor,
        address: &str,
        sender: &str,
    ) -> Result<Game, StorageError> {
        self.with_transaction(|conn| {
            let game = Self::create_game_on(
                conn,
                invite.game_id.clone(),
                opponent.to_string(),
                my_color,
                Some(serde_json::json!({ "dial_address": address })),
            )?;
            insert_invite(conn, invite, "local", sender)?;
            Ok(game)
        })
    }

    /// Persist an inbound invitation atomically, or validate an identical retry.
    pub fn receive_invite(&self, invite: &GameInvite, peer: &str) -> Result<(), StorageError> {
        self.with_transaction(|conn| {
            let existing = conn.query_row(
                "SELECT * FROM games WHERE id = ?1", [&invite.game_id],
                super::games::game_from_row,
            ).optional()?;
            if let Some(game) = existing {
                let conflict = if game.opponent_peer_id != peer {
                    Some("game id already exists for another peer")
                } else if game.status != GameStatus::Pending {
                    Some("game already exists and is not pending")
                } else {
                    None
                };
                if let Some(reason) = conflict {
                    return Err(StorageError::invalid_data("GameInvite", reason));
                }
                let mut stmt = conn.prepare(
                    "SELECT content, sender_peer_id, signature FROM messages WHERE game_id = ?1 AND message_type = 'GameInvite'",
                )?;
                let rows = stmt.query_map([&invite.game_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
                })?.collect::<Result<Vec<_>, _>>()?;
                if rows.len() != 1 || rows[0].1 != peer || rows[0].2 != "remote" ||
                    serde_json::from_str::<GameInvite>(&rows[0].0).ok().as_ref() != Some(invite) {
                    return Err(StorageError::invalid_data("GameInvite", "conflicting stored invitation payload"));
                }
                return Ok(());
            }
            Self::create_game_on(
                conn, invite.game_id.clone(), peer.to_string(),
                invite.suggested_color.map(PlayerColor::from).unwrap_or(PlayerColor::White),
                invite.reply_to.as_ref().map(|address| serde_json::json!({ "dial_address": address })),
            )?;
            insert_invite(conn, invite, "remote", peer)
        })
    }
}

fn insert_invite(
    conn: &rusqlite::Connection,
    invite: &GameInvite,
    signature: &str,
    sender: &str,
) -> Result<(), StorageError> {
    let content = serde_json::to_string(invite)
        .map_err(|e| StorageError::serialization_error("GameInvite", e))?;
    Database::store_message_on(
        conn,
        invite.game_id.clone(),
        "GameInvite".to_string(),
        content,
        signature.to_string(),
        sender.to_string(),
    )?;
    Ok(())
}
