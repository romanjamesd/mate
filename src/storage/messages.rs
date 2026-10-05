use crate::messages::chess::Move;
use crate::storage::database::Database;
use crate::storage::errors::{Result, StorageError};
use crate::storage::models::{Message, StoredMove};
use rusqlite::{named_params, params, OptionalExtension, Row};

impl Database {
    /// Store a new message
    pub fn store_message(
        &self,
        game_id: String,
        message_type: String,
        content: String,
        signature: String,
        sender_peer_id: String,
    ) -> Result<Message> {
        self.with_connection(|conn| {
            Self::store_message_on(
                conn,
                game_id,
                message_type,
                content,
                signature,
                sender_peer_id,
            )
        })
    }

    pub(crate) fn store_message_on(
        conn: &rusqlite::Connection,
        game_id: String,
        message_type: String,
        content: String,
        signature: String,
        sender_peer_id: String,
    ) -> Result<Message> {
        if message_type == "Move" {
            return Err(StorageError::invalid_data(
                "message_type",
                "Move rows must be inserted with insert_move_on",
            ));
        }
        let now = Self::current_timestamp();

        conn.execute(
            r#"
                INSERT INTO messages (
                    game_id, message_type, content, signature, sender_peer_id, created_at
                ) VALUES (
                    :game_id, :message_type, :content, :signature, :sender_peer_id, :created_at
                )
                "#,
            named_params! {
                ":game_id": game_id,
                ":message_type": message_type,
                ":content": content,
                ":signature": signature,
                ":sender_peer_id": sender_peer_id,
                ":created_at": now,
            },
        )?;

        let message_id = conn.last_insert_rowid();

        Ok(Message {
            id: Some(message_id),
            game_id,
            message_type,
            content,
            signature,
            sender_peer_id,
            created_at: now,
        })
    }

    /// Insert a typed payload without validating chronology, actor, or board hash.
    /// The authoritative game commit operation must perform those checks.
    pub(crate) fn insert_move_on(
        conn: &rusqlite::Connection,
        mv: &Move,
        signature: &str,
        sender_peer_id: &str,
    ) -> Result<Message> {
        let content = serde_json::to_string(mv)
            .map_err(|e| StorageError::serialization_error("Move message content", e))?;
        let now = Self::current_timestamp();
        conn.execute(
            "INSERT INTO messages (game_id, message_type, ply, content, signature, sender_peer_id, created_at)
             VALUES (?1, 'Move', ?2, ?3, ?4, ?5, ?6)",
            params![mv.game_id, mv.ply, content, signature, sender_peer_id, now],
        ).map_err(|e| match e {
            rusqlite::Error::SqliteFailure(ref error, _)
                if matches!(error.extended_code,
                    rusqlite::ffi::SQLITE_CONSTRAINT_CHECK | rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE) =>
            {
                StorageError::constraint_violation("messages", "ply", e.to_string())
            }
            other => StorageError::ConnectionFailed(other),
        })?;
        Ok(Message {
            id: Some(conn.last_insert_rowid()),
            game_id: mv.game_id.clone(),
            message_type: "Move".to_string(),
            content,
            signature: signature.to_string(),
            sender_peer_id: sender_peer_id.to_string(),
            created_at: now,
        })
    }

    /// Read chess moves in ascending protocol ply order. Generic history uses id order.
    pub fn get_moves_for_game(&self, game_id: &str) -> Result<Vec<StoredMove>> {
        self.with_connection(|conn| Self::get_moves_for_game_on(conn, game_id))
    }

    pub(crate) fn get_moves_for_game_on(
        conn: &rusqlite::Connection,
        game_id: &str,
    ) -> Result<Vec<StoredMove>> {
        let mut stmt = conn.prepare(
            "SELECT id, game_id, ply, content, signature, sender_peer_id, created_at
             FROM messages WHERE game_id = ?1 AND message_type = 'Move' ORDER BY ply ASC",
        )?;
        let rows = stmt.query_map([game_id], stored_move_from_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn get_move_at_ply_on(
        conn: &rusqlite::Connection,
        game_id: &str,
        ply: u32,
    ) -> Result<Option<StoredMove>> {
        Ok(conn
            .query_row(
                "SELECT id, game_id, ply, content, signature, sender_peer_id, created_at
             FROM messages WHERE game_id = ?1 AND message_type = 'Move' AND ply = ?2",
                params![game_id, ply],
                stored_move_from_row,
            )
            .optional()?)
    }

    /// Get a message by ID
    pub fn get_message(&self, message_id: i64) -> Result<Message> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                WHERE id = ?1
                "#,
                [message_id],
                message_from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    StorageError::message_not_found(message_id.to_string())
                }
                _ => StorageError::ConnectionFailed(e),
            })
        })
    }

    /// Get all messages for a game in local insertion order (ascending `id`).
    ///
    /// Order reflects when rows were stored in this database, not `created_at`
    /// (a wall-clock value kept for display/audit) and not peer-side order.
    pub fn get_messages_for_game(&self, game_id: &str) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                WHERE game_id = ?1
                ORDER BY id ASC
                "#,
            )?;

            let message_iter = stmt.query_map([game_id], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get one page of a game's messages in ascending insertion (`id`) order.
    ///
    /// For an unchanged history, consecutive pages (offset += limit) are complete
    /// and non-overlapping, and newly stored rows appear after existing ones.
    /// Rows deleted between calls can shift later pages; no snapshot is held.
    pub fn get_messages_for_game_paginated(
        &self,
        game_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                WHERE game_id = ?1
                ORDER BY id ASC
                LIMIT ?2 OFFSET ?3
                "#,
            )?;

            let message_iter = stmt.query_map(params![game_id, limit, offset], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get messages by type for a game in local insertion order (ascending `id`).
    pub fn get_messages_by_type(&self, game_id: &str, message_type: &str) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                WHERE game_id = ?1 AND message_type = ?2
                ORDER BY id ASC
                "#,
            )?;

            let message_iter = stmt.query_map([game_id, message_type], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get messages from a sender for a game in local insertion order (ascending `id`).
    pub fn get_messages_from_sender(
        &self,
        game_id: &str,
        sender_peer_id: &str,
    ) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                WHERE game_id = ?1 AND sender_peer_id = ?2
                ORDER BY id ASC
                "#,
            )?;

            let message_iter = stmt.query_map([game_id, sender_peer_id], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get the most recently stored messages across all games, newest first
    /// (descending `id`), for debugging/monitoring.
    pub fn get_recent_messages(&self, limit: u32) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                ORDER BY id DESC
                LIMIT ?1
                "#,
            )?;

            let message_iter = stmt.query_map([limit], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Count messages for a specific game
    pub fn count_messages_for_game(&self, game_id: &str) -> Result<u32> {
        self.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE game_id = ?1",
                [game_id],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        })
    }

    /// Delete all messages for a specific game
    /// Note: This is usually handled by CASCADE DELETE from games table
    pub fn delete_messages_for_game(&self, game_id: &str) -> Result<u32> {
        self.with_connection(|conn| {
            let rows_affected =
                conn.execute("DELETE FROM messages WHERE game_id = ?1", [game_id])?;
            Ok(rows_affected as u32)
        })
    }

    /// Delete a specific message by ID
    pub fn delete_message(&self, message_id: i64) -> Result<()> {
        self.with_connection(|conn| {
            let rows_affected = conn.execute("DELETE FROM messages WHERE id = ?1", [message_id])?;

            if rows_affected == 0 {
                return Err(StorageError::message_not_found(message_id.to_string()));
            }

            Ok(())
        })
    }
}

/// Convert a database row to a Message struct
fn message_from_row(row: &Row) -> rusqlite::Result<Message> {
    Ok(Message {
        id: Some(row.get("id")?),
        game_id: row.get("game_id")?,
        message_type: row.get("message_type")?,
        content: row.get("content")?,
        signature: row.get("signature")?,
        sender_peer_id: row.get("sender_peer_id")?,
        created_at: row.get("created_at")?,
    })
}

fn stored_move_from_row(row: &Row) -> rusqlite::Result<StoredMove> {
    Ok(StoredMove {
        id: row.get("id")?,
        game_id: row.get("game_id")?,
        ply: row.get("ply")?,
        content: row.get("content")?,
        signature: row.get("signature")?,
        sender_peer_id: row.get("sender_peer_id")?,
        created_at: row.get("created_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{schema, PlayerColor};
    use rusqlite::Connection;

    #[test]
    fn typed_move_storage_derives_columns_and_orders_by_ply() {
        let conn = Connection::open_in_memory().unwrap();
        schema::initialize_schema(&conn).unwrap();
        Database::create_game_on(
            &conn,
            "game".into(),
            "peer".into(),
            PlayerColor::White,
            None,
        )
        .unwrap();
        let generic_error = Database::store_message_on(
            &conn,
            "game".into(),
            "Move".into(),
            "{}".into(),
            "sig".into(),
            "peer".into(),
        )
        .unwrap_err();
        assert!(
            matches!(generic_error, StorageError::InvalidData { field, .. } if field == "message_type")
        );
        let second = Move::new("game".into(), 2, "e7e5".into(), "hash2".into());
        let first = Move::new("game".into(), 1, "e2e4".into(), "hash1".into());
        let second_row = Database::insert_move_on(&conn, &second, "sig2", "peer").unwrap();
        Database::store_message_on(
            &conn,
            "game".into(),
            "Chat".into(),
            "hi".into(),
            "sig".into(),
            "self".into(),
        )
        .unwrap();
        let first_row = Database::insert_move_on(&conn, &first, "sig1", "self").unwrap();
        conn.execute("UPDATE messages SET created_at = -id", [])
            .unwrap();
        let moves = Database::get_moves_for_game_on(&conn, "game").unwrap();
        assert_eq!(
            moves.iter().map(|row| row.ply).collect::<Vec<_>>(),
            [Some(1), Some(2)]
        );
        assert_eq!(moves[0].id, first_row.id.unwrap());
        assert_eq!(moves[1].id, second_row.id.unwrap());
        assert_eq!(moves[0].signature, "sig1");
        assert_eq!(moves[0].sender_peer_id, "self");
        assert_eq!(
            serde_json::from_str::<Move>(&moves[0].content).unwrap(),
            first
        );
        assert!(Database::get_moves_for_game_on(&conn, "unknown")
            .unwrap()
            .is_empty());
        let found = Database::get_move_at_ply_on(&conn, "game", 2)
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Move>(&found.content).unwrap(),
            second
        );
        assert!(Database::get_move_at_ply_on(&conn, "game", 3)
            .unwrap()
            .is_none());
        for mv in [
            &first,
            &Move::new("game".into(), 0, "e2e4".into(), "hash".into()),
        ] {
            assert!(matches!(Database::insert_move_on(&conn, mv, "sig", "peer"),
                Err(StorageError::ConstraintViolation { table, column, .. }) if table == "messages" && column == "ply"));
        }
        let missing_game = Move::new("missing".into(), 1, "e2e4".into(), "hash".into());
        assert!(
            matches!(Database::insert_move_on(&conn, &missing_game, "sig", "peer"),
            Err(StorageError::ConnectionFailed(rusqlite::Error::SqliteFailure(code, _)))
            if code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY)
        );
        conn.execute_batch(
            "CREATE TRIGGER reject_move BEFORE INSERT ON messages
            BEGIN SELECT RAISE(ABORT, 'test failure'); END;",
        )
        .unwrap();
        let third = Move::new("game".into(), 3, "g1f3".into(), "hash3".into());
        assert!(
            matches!(Database::insert_move_on(&conn, &third, "sig", "peer"),
            Err(StorageError::ConnectionFailed(rusqlite::Error::SqliteFailure(code, _)))
            if code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER)
        );
    }

    #[test]
    fn move_reader_preserves_invalid_ply_for_game_validation() {
        let conn = Connection::open_in_memory().unwrap();
        schema::initialize_schema(&conn).unwrap();
        Database::create_game_on(
            &conn,
            "game".into(),
            "peer".into(),
            PlayerColor::White,
            None,
        )
        .unwrap();
        conn.execute_batch("PRAGMA ignore_check_constraints = ON;
            INSERT INTO messages (game_id, message_type, content, signature, sender_peer_id, created_at, ply)
            VALUES ('game', 'Move', '{}', 'sig', 'peer', 1, NULL),
                   ('game', 'Move', '{}', 'sig', 'peer', 1, 4294967296);").unwrap();
        let rows = Database::get_moves_for_game_on(&conn, "game").unwrap();
        assert_eq!(rows[0].ply, None);
        assert_eq!(rows[1].ply, Some(4294967296));
    }
}
