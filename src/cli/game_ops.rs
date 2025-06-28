use crate::chess::{Board, ChessError, Move as ChessMove};
use crate::messages::chess::{GameInvite, Move as MoveMessage};
use crate::storage::{
    models::{Game, GameStatus, PlayerColor},
    Database,
};
use serde_json;
use std::str::FromStr;

/// Result type for game operations
pub type GameOpsResult<T> = Result<T, GameOpsError>;

/// Errors that can occur during game operations
#[derive(Debug)]
pub enum GameOpsError {
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
            GameOpsError::Database(e) => write!(f, "Database error: {}", e),
            GameOpsError::Chess(e) => write!(f, "Chess error: {}", e),
            GameOpsError::Serialization(e) => write!(f, "Serialization error: {}", e),
            GameOpsError::InvalidGameState(e) => write!(f, "Invalid game state: {}", e),
            GameOpsError::NoCurrentGame => write!(f, "No current game found"),
            GameOpsError::GameNotFound(id) => write!(f, "Game not found: {}", id),
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

/// Extended game information for display purposes
#[derive(Debug, Clone)]
pub struct GameRecord {
    pub game: Game,
    pub opponent_name: Option<String>,
    pub last_move: Option<String>,
    pub your_turn: bool,
    pub move_count: u32,
}

/// Game invitation with tracking information
#[derive(Debug, Clone)]
pub struct InvitationRecord {
    pub game_id: String,
    pub opponent_peer_id: String,
    pub suggested_color: Option<PlayerColor>,
    pub created_at: i64,
    pub status: InvitationStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Declined,
    Expired,
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
    database: &'a Database,
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

        for game in pending_games.into_iter().chain(active_games.into_iter()) {
            let record = self.create_game_record(game)?;
            records.push(record);
        }

        // Sort by most recently updated
        records.sort_by(|a, b| b.game.updated_at.cmp(&a.game.updated_at));

        Ok(records)
    }

    /// Reconstruct board state from move history
    pub fn reconstruct_game_state(&self, game_id: &str) -> GameOpsResult<GameState> {
        let game = self.database.get_game(game_id)?;
        let messages = self.database.get_messages_for_game(game_id)?;

        // Start with initial board position
        let mut board = Board::new();
        let mut move_history = Vec::new();

        // Apply all moves in chronological order
        for message in messages {
            if message.message_type == "Move" {
                let move_msg: MoveMessage =
                    serde_json::from_str(&message.content).map_err(|e| {
                        GameOpsError::Serialization(format!("Failed to parse move message: {}", e))
                    })?;

                // Parse the chess move from algebraic notation
                let chess_move = ChessMove::from_str(&move_msg.chess_move)?;

                // Apply the move to the board
                board.make_move(chess_move)?;
                move_history.push(move_msg.chess_move);
            }
        }

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
                "Ambiguous game ID '{}' matches multiple games",
                partial_id
            ))),
        }
    }

    /// Track pending invitations
    pub fn list_pending_invitations(&self) -> GameOpsResult<Vec<InvitationRecord>> {
        let pending_games = self.database.get_games_by_status(GameStatus::Pending)?;
        let mut invitations = Vec::new();

        for game in pending_games {
            // Check if this is an invitation we sent or received
            let messages = self.database.get_messages_for_game(&game.id)?;

            if let Some(invite_msg) = messages.iter().find(|m| m.message_type == "GameInvite") {
                let invite: GameInvite =
                    serde_json::from_str(&invite_msg.content).map_err(|e| {
                        GameOpsError::Serialization(format!("Failed to parse invitation: {}", e))
                    })?;

                let suggested_color = invite.suggested_color.map(|c| match c {
                    crate::chess::Color::White => PlayerColor::White,
                    crate::chess::Color::Black => PlayerColor::Black,
                });

                invitations.push(InvitationRecord {
                    game_id: game.id,
                    opponent_peer_id: game.opponent_peer_id,
                    suggested_color,
                    created_at: game.created_at,
                    status: InvitationStatus::Pending,
                });
            }
        }

        Ok(invitations)
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
        let messages = self.database.get_messages_for_game(&game.id)?;

        // Find the last move
        let last_move = messages
            .iter()
            .rev()
            .find(|m| m.message_type == "Move")
            .and_then(|m| {
                serde_json::from_str::<MoveMessage>(&m.content)
                    .ok()
                    .map(|move_msg| move_msg.chess_move)
            });

        // Count moves (each move message represents one move)
        let move_count = messages.iter().filter(|m| m.message_type == "Move").count() as u32;

        // Determine if it's our turn (simplified - could be more sophisticated)
        let your_turn = match game.status {
            GameStatus::Active => {
                // If move count is even and we're white, or odd and we're black, it's our turn
                match game.my_color {
                    PlayerColor::White => move_count % 2 == 0,
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
    fn is_your_turn(&self, game: &Game, board: &Board) -> GameOpsResult<bool> {
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
