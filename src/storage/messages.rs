use crate::storage::database::Database;
use crate::storage::errors::{Result, StorageError};
use crate::storage::models::Message;
use rusqlite::{named_params, Row};

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
        let now = Self::current_timestamp();

        self.with_connection(|conn| {
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
        })
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

    /// Get all messages for a specific game
    pub fn get_messages_for_game(&self, game_id: &str) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                WHERE game_id = ?1
                ORDER BY created_at ASC
                "#,
            )?;

            let message_iter = stmt.query_map([game_id], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get messages for a game with pagination
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
                ORDER BY created_at ASC
                LIMIT ?2 OFFSET ?3
                "#,
            )?;

            let message_iter = stmt.query_map(
                [game_id, &limit.to_string(), &offset.to_string()],
                message_from_row,
            )?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get messages by type for a specific game
    pub fn get_messages_by_type(&self, game_id: &str, message_type: &str) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                WHERE game_id = ?1 AND message_type = ?2
                ORDER BY created_at ASC
                "#,
            )?;

            let message_iter = stmt.query_map([game_id, message_type], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get messages from a specific sender
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
                ORDER BY created_at ASC
                "#,
            )?;

            let message_iter = stmt.query_map([game_id, sender_peer_id], message_from_row)?;
            let messages = message_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(messages)
        })
    }

    /// Get recent messages across all games (for debugging/monitoring)
    pub fn get_recent_messages(&self, limit: u32) -> Result<Vec<Message>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, game_id, message_type, content, signature, sender_peer_id, created_at
                FROM messages 
                ORDER BY created_at DESC
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
