use crate::chess::{Board, ChessError, Move as ChessMove};
use crate::game::message_type::StoredMessageType;
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

/// Validated move ready to send / commit — no DB write yet.
#[derive(Debug, Clone)]
pub struct PreparedMove {
    pub game_id: String,
    pub move_message: MoveMessage,
    pub move_number: u32,
    pub updated_board: Board,
    pub is_capture: bool,
    pub is_check: bool,
    pub is_checkmate: bool,
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

    /// Validate, parse, and build a wire `Move` without persisting.
    pub fn prepare_move(
        &self,
        game_id: &str,
        move_notation: &str,
        validate_turn: bool,
    ) -> MoveResult<PreparedMove> {
        self.validate_move_preconditions(game_id, move_notation, validate_turn)?;

        let game_state = self.game_ops.reconstruct_game_state(game_id)?;

        if validate_turn && !game_state.your_turn {
            return Err(MoveProcessingError::InvalidGameState(
                "It's not your turn to move".to_string(),
            ));
        }

        let chess_move = self.parse_and_validate_move(move_notation, &game_state.board)?;

        let mut updated_board = game_state.board.clone();
        updated_board.make_move(chess_move)?;

        let board_hash = crate::messages::chess::hash_board_state(&updated_board);
        let move_message =
            MoveMessage::new(game_id.to_string(), move_notation.to_string(), board_hash);

        let move_info = self.analyze_move(&game_state.board, &updated_board, chess_move)?;

        Ok(PreparedMove {
            game_id: game_id.to_string(),
            move_message,
            move_number: game_state.move_history.len() as u32 + 1,
            updated_board,
            is_capture: move_info.is_capture,
            is_check: move_info.is_check,
            is_checkmate: move_info.is_checkmate,
        })
    }

    /// Persist a previously prepared (or inbound) move.
    pub fn commit_move(
        &self,
        game_id: &str,
        move_message: &MoveMessage,
        sender_peer_id: &str,
        signature: &str,
    ) -> MoveResult<()> {
        let content = serde_json::to_string(move_message).map_err(|e| {
            MoveProcessingError::TransactionError(format!("Failed to serialize move: {e}"))
        })?;

        // Individual operations are atomic at the SQLite level; the storage
        // layer does not expose multi-statement transactions here.
        self.game_ops
            .database
            .store_message(
                game_id.to_string(),
                StoredMessageType::Move.as_str().to_string(),
                content,
                signature.to_string(),
                sender_peer_id.to_string(),
            )
            .map_err(|e| MoveProcessingError::TransactionError(format!("Database error: {e}")))?;

        self.game_ops
            .database
            .update_game_status(game_id, GameStatus::Active)
            .map_err(|e| {
                MoveProcessingError::TransactionError(format!("Failed to update game: {e}"))
            })?;

        Ok(())
    }

    /// Prepare then commit — convenience for DB-only callers (e.g. tests).
    pub fn process_move(
        &self,
        game_id: &str,
        move_notation: &str,
        validate_turn: bool,
    ) -> MoveResult<MoveProcessingResult> {
        let prepared = self.prepare_move(game_id, move_notation, validate_turn)?;
        self.commit_move(game_id, &prepared.move_message, "self", "")?;
        self.update_game_status_if_needed(game_id, &prepared.updated_board)?;

        Ok(MoveProcessingResult {
            game_id: prepared.game_id,
            move_notation: prepared.move_message.chess_move.clone(),
            board_state_hash: prepared.move_message.board_state_hash.clone(),
            move_number: prepared.move_number,
            is_capture: prepared.is_capture,
            is_check: prepared.is_check,
            is_checkmate: prepared.is_checkmate,
            updated_board: prepared.updated_board,
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
        sender_peer_id: &str,
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

        self.commit_move(game_id, move_message, sender_peer_id, "")?;

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

        // Legal move generation is not implemented yet.
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
        let move_type = StoredMessageType::Move.as_str();

        for message in messages {
            if message.message_type == move_type {
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

    /// Update game status if game is completed
    fn update_game_status_if_needed(&self, game_id: &str, board: &Board) -> MoveResult<()> {
        // Game-end detection (checkmate, stalemate, etc.) is not implemented yet.
        let _ = board;
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
        // Capture / check / checkmate analysis is not implemented yet.
        Ok(MoveAnalysis {
            is_capture: false,
            is_check: false,
            is_checkmate: false,
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
