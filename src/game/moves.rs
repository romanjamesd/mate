use crate::chess::{Board, ChessError, Move as ChessMove};
use crate::game::ops::{GameOps, GameOpsError};
use crate::messages::chess::Move as MoveMessage;
use crate::storage::{models::GameStatus, Database};

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
            let expected_hash = move_message.board_state_hash.clone();
            return Err(MoveProcessingError::BoardStateError(format!(
                "Board state hash mismatch. Expected: {expected_hash}, Got: {actual_hash}"
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
