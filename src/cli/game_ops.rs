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
                        GameOpsError::Serialization(format!("Failed to parse move message: {e}"))
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
                        GameOpsError::Serialization(format!("Failed to parse invitation: {e}"))
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

/// Move processing result type
pub type MoveResult<T> = Result<T, MoveProcessingError>;

/// Errors that can occur during move processing
#[derive(Debug)]
pub enum MoveProcessingError {
    /// Game operations error
    GameOps(GameOpsError),
    /// Chess engine validation error
    Chess(ChessError),
    /// Invalid move format or content
    InvalidMove(String),
    /// Game is not in a state that allows moves
    InvalidGameState(String),
    /// Database transaction error
    TransactionError(String),
    /// Board state verification error
    BoardStateError(String),
    /// Move history inconsistency
    HistoryError(String),
}

impl std::fmt::Display for MoveProcessingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveProcessingError::GameOps(e) => write!(f, "Game operations error: {}", e),
            MoveProcessingError::Chess(e) => write!(f, "Chess validation error: {}", e),
            MoveProcessingError::InvalidMove(e) => write!(f, "Invalid move: {}", e),
            MoveProcessingError::InvalidGameState(e) => write!(f, "Invalid game state: {}", e),
            MoveProcessingError::TransactionError(e) => write!(f, "Transaction error: {}", e),
            MoveProcessingError::BoardStateError(e) => write!(f, "Board state error: {}", e),
            MoveProcessingError::HistoryError(e) => write!(f, "Move history error: {}", e),
        }
    }
}

impl std::error::Error for MoveProcessingError {}

impl From<GameOpsError> for MoveProcessingError {
    fn from(err: GameOpsError) -> Self {
        MoveProcessingError::GameOps(err)
    }
}

impl From<ChessError> for MoveProcessingError {
    fn from(err: ChessError) -> Self {
        MoveProcessingError::Chess(err)
    }
}

/// Move processing result with detailed information
#[derive(Debug, Clone)]
pub struct MoveProcessingResult {
    pub game_id: String,
    pub move_notation: String,
    pub board_state_hash: String,
    pub move_number: u32,
    pub is_capture: bool,
    pub is_check: bool,
    pub is_checkmate: bool,
    pub updated_board: Board,
}

/// Transaction-safe move processor
pub struct MoveProcessor<'a> {
    game_ops: GameOps<'a>,
}

impl<'a> MoveProcessor<'a> {
    /// Create a new move processor
    pub fn new(database: &'a Database) -> Self {
        Self {
            game_ops: GameOps::new(database),
        }
    }

    /// Process and validate a move for a game
    /// This is the main entry point for move processing that handles all validation,
    /// board updates, database transactions, and history management
    pub fn process_move(
        &self,
        game_id: &str,
        move_notation: &str,
        validate_turn: bool,
    ) -> MoveResult<MoveProcessingResult> {
        // Start with comprehensive validation
        self.validate_move_preconditions(game_id, move_notation, validate_turn)?;

        // Reconstruct current game state
        let game_state = self.game_ops.reconstruct_game_state(game_id)?;

        // Validate it's the player's turn if requested
        if validate_turn && !game_state.your_turn {
            return Err(MoveProcessingError::InvalidGameState(
                "It's not your turn to move".to_string(),
            ));
        }

        // Parse and validate the move
        let chess_move = self.parse_and_validate_move(move_notation, &game_state.board)?;

        // Create a copy of the board to test the move
        let mut test_board = game_state.board.clone();

        // Apply the move to validate it's legal
        test_board.make_move(chess_move)?;

        // Create move message with board state hash
        let board_hash = crate::messages::chess::hash_board_state(&test_board);
        let move_message = MoveMessage::new(
            game_id.to_string(),
            move_notation.to_string(),
            board_hash.clone(),
        );

        // Store the move in database with transaction safety
        self.store_move_with_transaction(game_id, &move_message)?;

        // Update game state if it's now completed
        self.update_game_status_if_needed(game_id, &test_board)?;

        // Analyze move characteristics
        let move_info = self.analyze_move(&game_state.board, &test_board, chess_move)?;

        Ok(MoveProcessingResult {
            game_id: game_id.to_string(),
            move_notation: move_notation.to_string(),
            board_state_hash: board_hash,
            move_number: game_state.move_history.len() as u32 + 1,
            is_capture: move_info.is_capture,
            is_check: move_info.is_check,
            is_checkmate: move_info.is_checkmate,
            updated_board: test_board,
        })
    }

    /// Validate a move without applying it
    pub fn validate_move(
        &self,
        game_id: &str,
        move_notation: &str,
        validate_turn: bool,
    ) -> MoveResult<bool> {
        // Basic precondition validation
        self.validate_move_preconditions(game_id, move_notation, validate_turn)?;

        // Reconstruct current game state
        let game_state = self.game_ops.reconstruct_game_state(game_id)?;

        // Check turn if requested
        if validate_turn && !game_state.your_turn {
            return Ok(false);
        }

        // Try to parse and apply the move
        match self.parse_and_validate_move(move_notation, &game_state.board) {
            Ok(chess_move) => {
                let mut test_board = game_state.board.clone();
                match test_board.make_move(chess_move) {
                    Ok(()) => Ok(true),
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }

    /// Apply a move from an opponent (from network message)
    pub fn apply_opponent_move(
        &self,
        game_id: &str,
        move_message: &MoveMessage,
    ) -> MoveResult<MoveProcessingResult> {
        // Validate message format and security
        crate::messages::chess::validate_move_message(move_message).map_err(|e| {
            MoveProcessingError::InvalidMove(format!("Message validation failed: {e}"))
        })?;

        // Reconstruct current game state
        let game_state = self.game_ops.reconstruct_game_state(game_id)?;

        // Validate it's the opponent's turn
        if game_state.your_turn {
            return Err(MoveProcessingError::InvalidGameState(
                "Received move when it's not opponent's turn".to_string(),
            ));
        }

        // Parse and validate the move
        let chess_move =
            self.parse_and_validate_move(&move_message.chess_move, &game_state.board)?;

        // Apply move to board
        let mut updated_board = game_state.board.clone();
        updated_board.make_move(chess_move)?;

        // Verify board hash matches message
        let actual_hash = crate::messages::chess::hash_board_state(&updated_board);
        if actual_hash != move_message.board_state_hash {
            return Err(MoveProcessingError::BoardStateError(format!(
                "Board state hash mismatch. Expected: {}, Got: {}",
                move_message.board_state_hash, actual_hash
            )));
        }

        // Store the move with transaction safety
        self.store_move_with_transaction(game_id, move_message)?;

        // Update game status if needed
        self.update_game_status_if_needed(game_id, &updated_board)?;

        // Analyze move characteristics
        let move_info = self.analyze_move(&game_state.board, &updated_board, chess_move)?;

        Ok(MoveProcessingResult {
            game_id: game_id.to_string(),
            move_notation: move_message.chess_move.clone(),
            board_state_hash: move_message.board_state_hash.clone(),
            move_number: game_state.move_history.len() as u32 + 1,
            is_capture: move_info.is_capture,
            is_check: move_info.is_check,
            is_checkmate: move_info.is_checkmate,
            updated_board,
        })
    }

    /// Get all legal moves for current position
    pub fn get_legal_moves(&self, game_id: &str) -> MoveResult<Vec<String>> {
        let _game_state = self.game_ops.reconstruct_game_state(game_id)?;

        // TODO: Implement comprehensive legal move generation
        // For now, return empty vector as this requires complex chess logic
        // This would be enhanced in a future phase
        Ok(Vec::new())
    }

    /// Get move history with analysis
    pub fn get_move_history_with_analysis(
        &self,
        game_id: &str,
    ) -> MoveResult<Vec<MoveHistoryEntry>> {
        let _game_state = self.game_ops.reconstruct_game_state(game_id)?;
        let messages = self
            .game_ops
            .database
            .get_messages_for_game(game_id)
            .map_err(|e| MoveProcessingError::GameOps(GameOpsError::Database(e)))?;

        let mut history = Vec::new();
        let mut board = Board::new();
        let mut move_number = 1;

        for message in messages {
            if message.message_type == "Move" {
                let move_message: MoveMessage =
                    serde_json::from_str(&message.content).map_err(|e| {
                        MoveProcessingError::HistoryError(format!("Failed to parse move: {e}"))
                    })?;

                let chess_move =
                    ChessMove::from_str_with_color(&move_message.chess_move, board.active_color())?;
                let old_board = board.clone();

                board.make_move(chess_move)?;

                let move_info = self.analyze_move(&old_board, &board, chess_move)?;

                history.push(MoveHistoryEntry {
                    move_number,
                    notation: move_message.chess_move,
                    timestamp: message.created_at,
                    is_capture: move_info.is_capture,
                    is_check: move_info.is_check,
                    is_checkmate: move_info.is_checkmate,
                    board_hash: move_message.board_state_hash,
                });

                move_number += 1;
            }
        }

        Ok(history)
    }

    /// Validate move preconditions
    fn validate_move_preconditions(
        &self,
        game_id: &str,
        move_notation: &str,
        _validate_turn: bool,
    ) -> MoveResult<()> {
        // Validate game exists and is active
        let game = self
            .game_ops
            .database
            .get_game(game_id)
            .map_err(|e| MoveProcessingError::GameOps(GameOpsError::Database(e)))?;

        if game.status != GameStatus::Active {
            return Err(MoveProcessingError::InvalidGameState(format!(
                "Game is not active (status: {:?})",
                game.status
            )));
        }

        // Basic move notation validation
        if move_notation.trim().is_empty() {
            return Err(MoveProcessingError::InvalidMove(
                "Move notation cannot be empty".to_string(),
            ));
        }

        // Security validation
        crate::messages::chess::security::validate_secure_chess_move(move_notation, game_id)
            .map_err(|e| {
                MoveProcessingError::InvalidMove(format!("Security validation failed: {e}"))
            })?;

        Ok(())
    }

    /// Parse and validate move notation
    fn parse_and_validate_move(&self, move_notation: &str, board: &Board) -> MoveResult<ChessMove> {
        ChessMove::from_str_with_color(move_notation, board.active_color()).map_err(|e| {
            MoveProcessingError::InvalidMove(format!("Failed to parse move '{move_notation}': {e}"))
        })
    }

    /// Store move in database with transaction safety
    fn store_move_with_transaction(
        &self,
        game_id: &str,
        move_message: &MoveMessage,
    ) -> MoveResult<()> {
        // Serialize move message
        let content = serde_json::to_string(move_message).map_err(|e| {
            MoveProcessingError::TransactionError(format!("Failed to serialize move: {e}"))
        })?;

        // Store in database
        // Note: The current storage layer doesn't expose transaction APIs,
        // but the individual operations are atomic at the SQLite level
        self.game_ops
            .database
            .store_message(
                game_id.to_string(),
                "Move".to_string(),
                content,
                "".to_string(),     // Signature would be added in networking layer
                "self".to_string(), // Sender peer ID would be determined by context
            )
            .map_err(|e| MoveProcessingError::TransactionError(format!("Database error: {e}")))?;

        // Update game timestamp
        self.game_ops
            .database
            .update_game_status(game_id, GameStatus::Active)
            .map_err(|e| {
                MoveProcessingError::TransactionError(format!("Failed to update game: {e}"))
            })?;

        Ok(())
    }

    /// Update game status if game is completed
    fn update_game_status_if_needed(&self, game_id: &str, board: &Board) -> MoveResult<()> {
        // TODO: Implement game end detection (checkmate, stalemate, etc.)
        // This requires comprehensive chess logic that would be added in future phases

        // For now, just ensure the game remains active
        // Real implementation would check for:
        // - Checkmate
        // - Stalemate
        // - Insufficient material
        // - 50-move rule
        // - Threefold repetition

        let _ = board; // Suppress unused variable warning
        let _ = game_id;

        Ok(())
    }

    /// Analyze move characteristics
    fn analyze_move(
        &self,
        _old_board: &Board,
        _new_board: &Board,
        _chess_move: ChessMove,
    ) -> MoveResult<MoveAnalysis> {
        // TODO: Implement comprehensive move analysis
        // This requires chess logic for detecting checks, captures, etc.

        // For now, return basic analysis
        Ok(MoveAnalysis {
            is_capture: false,   // Would detect by checking if piece was captured
            is_check: false,     // Would require check detection
            is_checkmate: false, // Would require checkmate detection
        })
    }
}

/// Move analysis result
#[derive(Debug, Clone)]
struct MoveAnalysis {
    is_capture: bool,
    is_check: bool,
    is_checkmate: bool,
}

/// Move history entry with analysis
#[derive(Debug, Clone)]
pub struct MoveHistoryEntry {
    pub move_number: u32,
    pub notation: String,
    pub timestamp: i64,
    pub is_capture: bool,
    pub is_check: bool,
    pub is_checkmate: bool,
    pub board_hash: String,
}
