use crate::storage::database::Database;
use crate::storage::errors::{Result, StorageError};
use crate::storage::models::{Game, GameResult, GameStatus, PlayerColor};
use rusqlite::{named_params, Row};

impl Database {
    /// Create a new game record
    pub fn create_game(
        &self,
        opponent_peer_id: String,
        my_color: PlayerColor,
        metadata: Option<serde_json::Value>,
    ) -> Result<Game> {
        let game_id = self.generate_game_id();
        let now = Self::current_timestamp();

        let game = Game {
            id: game_id.clone(),
            opponent_peer_id,
            my_color,
            status: GameStatus::Pending,
            created_at: now,
            updated_at: now,
            completed_at: None,
            result: None,
            metadata,
        };

        // Serialize metadata outside the named_params! macro
        let serialized_metadata = game
            .metadata
            .as_ref()
            .map(|m| {
                serde_json::to_string(m)
                    .map_err(|e| StorageError::serialization_error("game metadata", e))
            })
            .transpose()?;

        self.with_connection(|conn| {
            conn.execute(
                r#"
                INSERT INTO games (
                    id, opponent_peer_id, my_color, status, 
                    created_at, updated_at, completed_at, result, metadata
                ) VALUES (
                    :id, :opponent_peer_id, :my_color, :status,
                    :created_at, :updated_at, :completed_at, :result, :metadata
                )
                "#,
                named_params! {
                    ":id": game.id,
                    ":opponent_peer_id": game.opponent_peer_id,
                    ":my_color": game.my_color.as_str(),
                    ":status": game.status.as_str(),
                    ":created_at": game.created_at,
                    ":updated_at": game.updated_at,
                    ":completed_at": game.completed_at,
                    ":result": game.result.as_ref().map(|r| r.as_str()),
                    ":metadata": serialized_metadata,
                },
            )?;
            Ok(game)
        })
    }

    /// Get a game by ID
    pub fn get_game(&self, game_id: &str) -> Result<Game> {
        self.with_connection(|conn| {
            conn.query_row(
                r#"
                SELECT id, opponent_peer_id, my_color, status, 
                       created_at, updated_at, completed_at, result, metadata
                FROM games 
                WHERE id = ?1
                "#,
                [game_id],
                game_from_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StorageError::game_not_found(game_id),
                _ => StorageError::ConnectionFailed(e),
            })
        })
    }

    /// Update game status
    pub fn update_game_status(&self, game_id: &str, status: GameStatus) -> Result<()> {
        let now = Self::current_timestamp();
        let completed_at = if matches!(status, GameStatus::Completed | GameStatus::Abandoned) {
            Some(now)
        } else {
            None
        };

        self.with_connection(|conn| {
            let rows_affected = conn.execute(
                r#"
                UPDATE games 
                SET status = ?1, updated_at = ?2, completed_at = ?3
                WHERE id = ?4
                "#,
                (status.as_str(), now, completed_at, game_id),
            )?;

            if rows_affected == 0 {
                return Err(StorageError::game_not_found(game_id));
            }

            Ok(())
        })
    }

    /// Update game result
    pub fn update_game_result(&self, game_id: &str, result: GameResult) -> Result<()> {
        let now = Self::current_timestamp();

        self.with_connection(|conn| {
            let rows_affected = conn.execute(
                r#"
                UPDATE games 
                SET result = ?1, updated_at = ?2, status = ?3, completed_at = ?4
                WHERE id = ?5
                "#,
                (
                    result.as_str(),
                    now,
                    GameStatus::Completed.as_str(),
                    now,
                    game_id,
                ),
            )?;

            if rows_affected == 0 {
                return Err(StorageError::game_not_found(game_id));
            }

            Ok(())
        })
    }

    /// Get all games for a specific opponent
    pub fn get_games_with_opponent(&self, opponent_peer_id: &str) -> Result<Vec<Game>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, opponent_peer_id, my_color, status, 
                       created_at, updated_at, completed_at, result, metadata
                FROM games 
                WHERE opponent_peer_id = ?1
                ORDER BY created_at DESC
                "#,
            )?;

            let game_iter = stmt.query_map([opponent_peer_id], game_from_row)?;
            let games = game_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(games)
        })
    }

    /// Get games by status
    pub fn get_games_by_status(&self, status: GameStatus) -> Result<Vec<Game>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, opponent_peer_id, my_color, status, 
                       created_at, updated_at, completed_at, result, metadata
                FROM games 
                WHERE status = ?1
                ORDER BY created_at DESC
                "#,
            )?;

            let game_iter = stmt.query_map([status.as_str()], game_from_row)?;
            let games = game_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(games)
        })
    }

    /// Get recent games (limited count)
    pub fn get_recent_games(&self, limit: u32) -> Result<Vec<Game>> {
        self.with_connection(|conn| {
            let mut stmt = conn.prepare(
                r#"
                SELECT id, opponent_peer_id, my_color, status, 
                       created_at, updated_at, completed_at, result, metadata
                FROM games 
                ORDER BY created_at DESC
                LIMIT ?1
                "#,
            )?;

            let game_iter = stmt.query_map([limit], game_from_row)?;
            let games = game_iter.collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(games)
        })
    }

    /// Delete a game and all associated messages
    pub fn delete_game(&self, game_id: &str) -> Result<()> {
        self.with_connection(|conn| {
            let rows_affected = conn.execute("DELETE FROM games WHERE id = ?1", [game_id])?;

            if rows_affected == 0 {
                return Err(StorageError::game_not_found(game_id));
            }

            Ok(())
        })
    }
}

/// Convert a database row to a Game struct
fn game_from_row(row: &Row) -> rusqlite::Result<Game> {
    let metadata_str: Option<String> = row.get("metadata")?;
    let metadata = match metadata_str {
        Some(s) => Some(serde_json::from_str(&s).map_err(|_e| {
            rusqlite::Error::InvalidColumnType(
                0,
                "metadata".to_string(),
                rusqlite::types::Type::Text,
            )
        })?),
        None => None,
    };

    let my_color_str: String = row.get("my_color")?;
    let my_color = my_color_str.parse::<PlayerColor>().map_err(|_e| {
        rusqlite::Error::InvalidColumnType(0, "my_color".to_string(), rusqlite::types::Type::Text)
    })?;

    let status_str: String = row.get("status")?;
    let status = status_str.parse::<GameStatus>().map_err(|_e| {
        rusqlite::Error::InvalidColumnType(0, "status".to_string(), rusqlite::types::Type::Text)
    })?;

    let result_str: Option<String> = row.get("result")?;
    let result = match result_str {
        Some(s) => Some(s.parse::<GameResult>().map_err(|_e| {
            rusqlite::Error::InvalidColumnType(0, "result".to_string(), rusqlite::types::Type::Text)
        })?),
        None => None,
    };

    Ok(Game {
        id: row.get("id")?,
        opponent_peer_id: row.get("opponent_peer_id")?,
        my_color,
        status,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        completed_at: row.get("completed_at")?,
        result,
        metadata,
    })
}
