use crate::chess::{Board, ChessError};
use crate::messages::chess::Move as MoveMessage;
use crate::storage::{
    models::{Game, GameStatus, PlayerColor},
    Database,
};

use super::{rebuild_chronology, Chronology, ChronologyError};
use crate::storage::StoredMove;

/// Result type for game operations
pub type GameOpsResult<T> = Result<T, GameOpsError>;

/// Errors that can occur during game operations
#[derive(Debug)]
pub enum GameOpsError {
    /// Stored protocol chronology is invalid.
    Chronology(ChronologyError),
    /// Database operation failed
    Database(crate::storage::errors::StorageError),
    /// Chess engine error
    Chess(ChessError),
    /// Message parsing/serialization error
    Serialization(String),
    /// Game state is invalid or corrupted
    InvalidGameState(String),
    /// No current game found
    NoCurrentGame,
    /// Game not found
    GameNotFound(String),
}

impl std::fmt::Display for GameOpsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameOpsError::Chronology(e) => write!(f, "Invalid move chronology: {e}"),
            GameOpsError::Database(e) => write!(f, "Database error: {e}"),
            GameOpsError::Chess(e) => write!(f, "Chess error: {e}"),
            GameOpsError::Serialization(e) => write!(f, "Serialization error: {e}"),
            GameOpsError::InvalidGameState(e) => write!(f, "Invalid game state: {e}"),
            GameOpsError::NoCurrentGame => write!(f, "No current game found"),
            GameOpsError::GameNotFound(id) => write!(f, "Game not found: {id}"),
        }
    }
}

impl std::error::Error for GameOpsError {}

impl From<crate::storage::errors::StorageError> for GameOpsError {
    fn from(err: crate::storage::errors::StorageError) -> Self {
        GameOpsError::Database(err)
    }
}

impl From<ChessError> for GameOpsError {
    fn from(err: ChessError) -> Self {
        GameOpsError::Chess(err)
    }
}

impl From<ChronologyError> for GameOpsError {
    fn from(err: ChronologyError) -> Self {
        Self::Chronology(err)
    }
}

/// Rows and validated payloads captured in the same database snapshot.
pub(crate) struct LoadedChronology {
    pub game: Game,
    pub chronology: Chronology,
    pub rows: Vec<StoredMove>,
}

/// Caller must supply a connection with an active transaction.
pub(crate) fn load_chronology_on(
    conn: &rusqlite::Connection,
    game_id: &str,
) -> GameOpsResult<LoadedChronology> {
    let game = Database::get_game_on(conn, game_id)?;
    let rows = Database::get_moves_for_game_on(conn, game_id)?;
    let chronology = rebuild_chronology(game_id, &rows)?;
    Ok(LoadedChronology {
        game,
        chronology,
        rows,
    })
}

/// A deferred read transaction keeps game and moves consistent across connections.
pub(crate) fn load_chronology(db: &Database, game_id: &str) -> GameOpsResult<LoadedChronology> {
    db.with_transaction(|conn| Ok(load_chronology_on(conn, game_id)))?
}

/// Extended game information for display purposes
#[derive(Debug, Clone)]
pub struct GameRecord {
    pub game: Game,
    pub opponent_name: Option<String>,
    pub last_move: Option<String>,
    pub your_turn: bool,
    pub move_count: u32,
}

/// Reconstructed game state with board and history
#[derive(Debug, Clone)]
pub struct GameState {
    pub game: Game,
    pub board: Board,
    pub move_history: Vec<String>,
    pub your_turn: bool,
}

/// Game operations manager
pub struct GameOps<'a> {
    pub(crate) database: &'a Database,
}

impl<'a> GameOps<'a> {
    /// Create a new game operations manager
    pub fn new(database: &'a Database) -> Self {
        Self { database }
    }

    /// List all games with extended information
    pub fn list_games(&self) -> GameOpsResult<Vec<GameRecord>> {
        let games = self.database.get_all_games()?;
        let mut records = Vec::new();

        for game in games {
            let record = self.create_game_record(game)?;
            records.push(record);
        }

        Ok(records)
    }

    /// List games filtered by status
    pub fn list_games_by_status(&self, status: GameStatus) -> GameOpsResult<Vec<GameRecord>> {
        let games = self.database.get_games_by_status(status)?;
        let mut records = Vec::new();

        for game in games {
            let record = self.create_game_record(game)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Get active games (pending or in progress)
    pub fn list_active_games(&self) -> GameOpsResult<Vec<GameRecord>> {
        let pending_games = self.database.get_games_by_status(GameStatus::Pending)?;
        let active_games = self.database.get_games_by_status(GameStatus::Active)?;

        let mut records = Vec::new();

        for game in pending_games.into_iter().chain(active_games) {
            let record = self.create_game_record(game)?;
            records.push(record);
        }

        // Sort by most recently updated
        records.sort_by_key(|b| std::cmp::Reverse(b.game.updated_at));

        Ok(records)
    }

    /// Reconstruct board state from move history
    pub fn reconstruct_game_state(&self, game_id: &str) -> GameOpsResult<GameState> {
        let loaded = load_chronology(self.database, game_id)?;
        let game = loaded.game;
        let board = loaded.chronology.board;
        let history = loaded.chronology.history;
        let move_history: Vec<String> = history.iter().map(|m| m.to_string()).collect();

        // Determine whose turn it is
        let your_turn = self.is_your_turn(&game, &board)?;

        Ok(GameState {
            game,
            board,
            move_history,
            your_turn,
        })
    }

    /// Get the current game (most recently active)
    pub fn get_current_game(&self) -> GameOpsResult<Game> {
        let active_games = self.list_active_games()?;

        active_games
            .into_iter()
            .next()
            .map(|record| record.game)
            .ok_or(GameOpsError::NoCurrentGame)
    }

    /// Get current game ID if available
    pub fn get_current_game_id(&self) -> GameOpsResult<String> {
        let game = self.get_current_game()?;
        Ok(game.id)
    }

    /// Find game by partial ID match (for user convenience)
    pub fn find_game_by_partial_id(&self, partial_id: &str) -> GameOpsResult<Game> {
        let all_games = self.database.get_all_games()?;

        // First try exact match
        if let Ok(game) = self.database.get_game(partial_id) {
            return Ok(game);
        }

        // Then try prefix match
        let matches: Vec<_> = all_games
            .into_iter()
            .filter(|game| game.id.starts_with(partial_id))
            .collect();

        match matches.len() {
            0 => Err(GameOpsError::GameNotFound(partial_id.to_string())),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => Err(GameOpsError::InvalidGameState(format!(
                "Ambiguous game ID '{partial_id}' matches multiple games"
            ))),
        }
    }

    /// Count games by status
    pub fn count_games_by_status(&self, status: GameStatus) -> GameOpsResult<usize> {
        let games = self.database.get_games_by_status(status)?;
        Ok(games.len())
    }

    /// Get game statistics
    pub fn get_game_statistics(&self) -> GameOpsResult<GameStatistics> {
        let all_games = self.database.get_all_games()?;
        let mut stats = GameStatistics::default();

        for game in all_games {
            stats.total_games += 1;

            match game.status {
                GameStatus::Pending => stats.pending_games += 1,
                GameStatus::Active => stats.active_games += 1,
                GameStatus::Completed => {
                    stats.completed_games += 1;
                    if let Some(result) = game.result {
                        match result {
                            crate::storage::models::GameResult::Win => stats.wins += 1,
                            crate::storage::models::GameResult::Loss => stats.losses += 1,
                            crate::storage::models::GameResult::Draw => stats.draws += 1,
                            crate::storage::models::GameResult::Abandoned => stats.abandoned += 1,
                        }
                    }
                }
                GameStatus::Abandoned => stats.abandoned += 1,
            }
        }

        Ok(stats)
    }

    /// Helper function to create a game record with extended information
    fn create_game_record(&self, game: Game) -> GameOpsResult<GameRecord> {
        // Best-effort display only: corrupt payloads do not prevent listing games.
        let messages = self.database.get_moves_for_game(&game.id)?;

        // Find the last move
        let last_move = messages.last().and_then(|m| {
            serde_json::from_str::<MoveMessage>(&m.content)
                .ok()
                .map(|move_msg| move_msg.chess_move)
        });

        // Count moves (each move message represents one move)
        let move_count = messages.len() as u32;

        // Determine if it's our turn (simplified - could be more sophisticated)
        let your_turn = match game.status {
            GameStatus::Active => {
                // If move count is even and we're white, or odd and we're black, it's our turn
                match game.my_color {
                    PlayerColor::White => move_count.is_multiple_of(2),
                    PlayerColor::Black => move_count % 2 == 1,
                }
            }
            GameStatus::Pending => false, // Not our turn until game starts
            _ => false,                   // Game is over
        };

        Ok(GameRecord {
            game,
            opponent_name: None, // Could be enhanced with peer name lookup
            last_move,
            your_turn,
            move_count,
        })
    }

    /// Helper to determine if it's the current player's turn
    pub(crate) fn is_your_turn(&self, game: &Game, board: &Board) -> GameOpsResult<bool> {
        if game.status != GameStatus::Active {
            return Ok(false);
        }

        // Check if the board's active color matches our color
        let our_chess_color = match game.my_color {
            PlayerColor::White => crate::chess::Color::White,
            PlayerColor::Black => crate::chess::Color::Black,
        };

        Ok(board.active_color() == our_chess_color)
    }
}

/// Game statistics summary
#[derive(Debug, Default)]
pub struct GameStatistics {
    pub total_games: usize,
    pub active_games: usize,
    pub pending_games: usize,
    pub completed_games: usize,
    pub wins: usize,
    pub losses: usize,
    pub draws: usize,
    pub abandoned: usize,
}

impl GameStatistics {
    /// Calculate win rate as a percentage
    pub fn win_rate(&self) -> f64 {
        if self.completed_games == 0 {
            0.0
        } else {
            (self.wins as f64 / self.completed_games as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod chronology_tests {
    use super::*;
    use crate::chess::Move as ChessMove;
    use crate::game::{store_game_move_message, MoveProcessor};
    use crate::messages::chess::hash_board_state;

    #[test]
    fn reader_keeps_game_moves_and_timestamps_in_one_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("game.sqlite");
        let reader = Database::new_with_path("self", &path).unwrap();
        let writer = Database::new_with_path("self", &path).unwrap();
        reader
            .with_connection(|conn| {
                conn.pragma_update(None, "journal_mode", "WAL")?;
                Ok(())
            })
            .unwrap();
        writer
            .with_connection(|conn| {
                conn.pragma_update(None, "journal_mode", "WAL")?;
                Ok(())
            })
            .unwrap();
        let game = reader
            .create_game_with_id(
                uuid::Uuid::new_v4().to_string(),
                "peer".into(),
                PlayerColor::White,
                None,
            )
            .unwrap();
        let mut board = Board::new();
        board
            .make_move(ChessMove::from_str_with_color("e2e4", board.active_color()).unwrap())
            .unwrap();
        let mv = MoveMessage::new(game.id.clone(), 1, "e2e4".into(), hash_board_state(&board));
        reader
            .with_transaction(|conn| {
                // Establish the snapshot before another independent connection commits.
                assert_eq!(
                    Database::get_game_on(conn, &game.id)?.status,
                    GameStatus::Pending
                );
                writer.with_immediate_transaction(|tx| {
                    Database::insert_move_on(tx, &mv, "sig", "self")?;
                    tx.execute("UPDATE games SET status='active' WHERE id=?1", [&game.id])?;
                    tx.execute(
                        "UPDATE messages SET created_at=42 WHERE game_id=?1",
                        [&game.id],
                    )?;
                    Ok(())
                })?;
                let loaded = load_chronology_on(conn, &game.id).unwrap();
                assert_eq!(loaded.game.status, GameStatus::Pending);
                assert!(loaded.rows.is_empty());
                assert!(loaded.chronology.moves.is_empty());
                Ok(())
            })
            .unwrap();
        let loaded = load_chronology(&reader, &game.id).unwrap();
        assert_eq!(loaded.game.status, GameStatus::Active);
        assert_eq!(loaded.chronology.moves[0], mv);
        assert_eq!(loaded.rows[0].created_at, 42);
        let processor = MoveProcessor::new(&reader);
        let analyzed = processor.get_move_history_with_analysis(&game.id).unwrap();
        assert_eq!(analyzed[0].timestamp, 42);
        assert_eq!(analyzed[0].notation, "e2e4");
        assert_eq!(analyzed[0].board_hash, mv.board_state_hash);
        assert!(processor.prepare_move(&game.id, "e7e5").is_err());
        assert!(processor.process_move(&game.id, "e7e5").is_err());
        assert_eq!(reader.get_moves_for_game(&game.id).unwrap().len(), 1);
    }

    #[test]
    fn consumers_reject_corrupt_history_but_listing_remains_available() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::new_with_path("self", &dir.path().join("game.sqlite")).unwrap();
        let game = db
            .create_game_with_id(
                uuid::Uuid::new_v4().to_string(),
                "peer".into(),
                PlayerColor::White,
                None,
            )
            .unwrap();
        db.update_game_status(&game.id, GameStatus::Active).unwrap();
        let mv = MoveMessage::new(game.id.clone(), 1, "e2e4".into(), "wrong hash".into());
        store_game_move_message(&db, &mv, "sig", "self").unwrap();
        let ops = GameOps::new(&db);
        assert!(matches!(
            ops.reconstruct_game_state(&game.id),
            Err(GameOpsError::Chronology(ChronologyError::HashMismatch {
                ply: 1
            }))
        ));
        let processor = MoveProcessor::new(&db);
        assert!(processor.get_move_history_with_analysis(&game.id).is_err());
        assert!(processor.prepare_move(&game.id, "e7e5").is_err());
        assert_eq!(ops.list_games().unwrap().len(), 1);
        let response = crate::network::handlers::dispatch(
            &db,
            "peer",
            crate::messages::Message::new_sync_request(game.id.clone()),
        )
        .unwrap();
        assert!(
            matches!(response, Some(crate::messages::Message::GameDecline(decline)) if decline.reason.as_deref().unwrap().contains("failed to rebuild board"))
        );
        assert_eq!(db.get_moves_for_game(&game.id).unwrap().len(), 1);
    }
}
