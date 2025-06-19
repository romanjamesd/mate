use crate::chess::Board;
use crate::chess::Color;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Chess game invitation message
/// Sent to invite another player to a chess game
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameInvite {
    /// Unique identifier for the game
    pub game_id: String,
    /// Suggested color for the invitee (None means invitee can choose)
    pub suggested_color: Option<Color>,
}

impl GameInvite {
    /// Create a new game invitation
    pub fn new(game_id: String, suggested_color: Option<Color>) -> Self {
        Self {
            game_id,
            suggested_color,
        }
    }

    /// Create a game invitation without color suggestion
    pub fn new_no_color_preference(game_id: String) -> Self {
        Self::new(game_id, None)
    }

    /// Create a game invitation with a specific color suggestion
    pub fn new_with_color(game_id: String, color: Color) -> Self {
        Self::new(game_id, Some(color))
    }
}

/// Chess game acceptance message
/// Sent in response to a game invitation to accept the game
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameAccept {
    /// Unique identifier for the game being accepted
    pub game_id: String,
    /// Color the accepter wants to play as
    pub accepted_color: Color,
}

impl GameAccept {
    /// Create a new game acceptance
    pub fn new(game_id: String, accepted_color: Color) -> Self {
        Self {
            game_id,
            accepted_color,
        }
    }
}

/// Chess game decline message
/// Sent in response to a game invitation to decline the game
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameDecline {
    /// Unique identifier for the game being declined
    pub game_id: String,
    /// Optional reason for declining the game
    pub reason: Option<String>,
}

impl GameDecline {
    /// Create a new game decline
    pub fn new(game_id: String, reason: Option<String>) -> Self {
        Self { game_id, reason }
    }

    /// Create a game decline without a reason
    pub fn new_no_reason(game_id: String) -> Self {
        Self::new(game_id, None)
    }

    /// Create a game decline with a specific reason
    pub fn new_with_reason(game_id: String, reason: String) -> Self {
        Self::new(game_id, Some(reason))
    }
}

/// Chess move message
/// Sent to communicate a chess move to the opponent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    /// Unique identifier for the game
    pub game_id: String,
    /// Chess move in algebraic notation (e.g., "e2e4", "O-O")
    pub chess_move: String,
    /// SHA-256 hash of the board state after the move for verification
    pub board_state_hash: String,
}

impl Move {
    /// Create a new chess move message
    pub fn new(game_id: String, chess_move: String, board_state_hash: String) -> Self {
        Self {
            game_id,
            chess_move,
            board_state_hash,
        }
    }
}

/// Chess move acknowledgment message
/// Sent to acknowledge receipt of a chess move
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveAck {
    /// Unique identifier for the game
    pub game_id: String,
    /// Optional move identifier for tracking specific moves
    pub move_id: Option<String>,
}

impl MoveAck {
    /// Create a new move acknowledgment
    pub fn new(game_id: String, move_id: Option<String>) -> Self {
        Self { game_id, move_id }
    }

    /// Create a move acknowledgment without a move ID
    pub fn new_no_move_id(game_id: String) -> Self {
        Self::new(game_id, None)
    }

    /// Create a move acknowledgment with a specific move ID
    pub fn new_with_move_id(game_id: String, move_id: String) -> Self {
        Self::new(game_id, Some(move_id))
    }
}

/// Chess game synchronization request message
/// Sent to request the current game state from the opponent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncRequest {
    /// Unique identifier for the game
    pub game_id: String,
}

impl SyncRequest {
    /// Create a new synchronization request
    pub fn new(game_id: String) -> Self {
        Self { game_id }
    }
}

/// Chess game synchronization response message
/// Sent in response to a sync request to provide current game state
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncResponse {
    /// Unique identifier for the game
    pub game_id: String,
    /// Current board state in FEN notation
    pub board_state: String,
    /// Complete move history in algebraic notation
    pub move_history: Vec<String>,
    /// SHA-256 hash of the current board state for verification
    pub board_state_hash: String,
}

impl SyncResponse {
    /// Create a new synchronization response
    pub fn new(
        game_id: String,
        board_state: String,
        move_history: Vec<String>,
        board_state_hash: String,
    ) -> Self {
        Self {
            game_id,
            board_state,
            move_history,
            board_state_hash,
        }
    }
}

/// Generate a unique game ID using UUID v4
///
/// Creates a cryptographically secure, collision-resistant game identifier
/// that can be used to uniquely identify chess games across the network.
///
/// # Returns
///
/// A string representation of a UUID v4 that serves as a unique game identifier
///
/// # Examples
///
/// ```
/// use mate::messages::chess::generate_game_id;
///
/// let game_id = generate_game_id();
/// assert!(!game_id.is_empty());
/// assert_eq!(game_id.len(), 36); // Standard UUID string length
/// ```
pub fn generate_game_id() -> String {
    Uuid::new_v4().to_string()
}

/// Validate that a string is a properly formatted UUID game ID
///
/// Checks if the provided string conforms to the UUID format expected
/// for game identifiers. This validation ensures that game IDs are
/// consistently formatted and helps catch malformed identifiers.
///
/// # Arguments
///
/// * `id` - The game ID string to validate
///
/// # Returns
///
/// `true` if the ID is a valid UUID format, `false` otherwise
///
/// # Examples
///
/// ```
/// use mate::messages::chess::{generate_game_id, validate_game_id};
///
/// let valid_id = generate_game_id();
/// assert!(validate_game_id(&valid_id));
///
/// let invalid_id = "not-a-uuid";
/// assert!(!validate_game_id(invalid_id));
/// ```
pub fn validate_game_id(id: &str) -> bool {
    Uuid::parse_str(id).is_ok()
}

/// Generate a SHA-256 hash of the current board state
///
/// Creates a deterministic, cryptographically secure hash of the board state
/// using FEN notation as the canonical representation. This ensures that
/// identical board positions always produce identical hashes, making it
/// suitable for integrity checking and synchronization verification.
///
/// The hash includes:
/// - All piece positions
/// - Active color (current player to move)
/// - Castling rights
/// - En passant target square
/// - Halfmove clock (50-move rule counter)
/// - Fullmove number
///
/// # Arguments
///
/// * `board` - The chess board to hash
///
/// # Returns
///
/// A lowercase hexadecimal string representation of the SHA-256 hash
///
/// # Examples
///
/// ```
/// use mate::chess::Board;
/// use mate::messages::chess::hash_board_state;
///
/// let board = Board::new();
/// let hash = hash_board_state(&board);
/// assert_eq!(hash.len(), 64); // SHA-256 produces 64-character hex strings
/// ```
pub fn hash_board_state(board: &Board) -> String {
    // Use FEN notation as the canonical representation for consistent hashing
    let fen = board.to_fen();

    // Create SHA-256 hasher
    let mut hasher = Sha256::new();

    // Hash the FEN string (canonical board representation)
    hasher.update(fen.as_bytes());

    // Get the hash result and convert to lowercase hex string
    let result = hasher.finalize();
    format!("{:x}", result)
}

/// Verify that a board state matches the expected hash
///
/// Computes the SHA-256 hash of the given board state and compares it
/// against the expected hash value. This is used to verify board state
/// integrity in chess messages and detect any desynchronization between
/// players.
///
/// # Arguments
///
/// * `board` - The chess board to verify
/// * `expected_hash` - The expected SHA-256 hash in hexadecimal format
///
/// # Returns
///
/// `true` if the computed hash matches the expected hash, `false` otherwise
///
/// # Examples
///
/// ```
/// use mate::chess::Board;
/// use mate::messages::chess::{hash_board_state, verify_board_hash};
///
/// let board = Board::new();
/// let hash = hash_board_state(&board);
/// assert!(verify_board_hash(&board, &hash));
///
/// let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
/// assert!(!verify_board_hash(&board, wrong_hash));
/// ```
pub fn verify_board_hash(board: &Board, expected_hash: &str) -> bool {
    let computed_hash = hash_board_state(board);

    // Compare hashes in a case-insensitive manner
    computed_hash.eq_ignore_ascii_case(expected_hash)
}

/// Comprehensive error type for chess message validation failures
///
/// This enum covers all types of validation errors that can occur when
/// validating chess messages, from malformed game IDs to invalid chess moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// Invalid game ID format (not a valid UUID)
    InvalidGameId(String),
    /// Invalid chess move format or illegal move
    InvalidMove(String),
    /// Invalid board state hash format or verification failure
    InvalidBoardHash(String),
    /// Invalid FEN notation in sync messages
    InvalidFen(String),
    /// Message contains invalid or missing required fields
    InvalidMessageFormat(String),
    /// Board state hash mismatch during verification
    BoardHashMismatch { expected: String, actual: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::InvalidGameId(msg) => write!(f, "Invalid game ID: {}", msg),
            ValidationError::InvalidMove(msg) => write!(f, "Invalid chess move: {}", msg),
            ValidationError::InvalidBoardHash(msg) => write!(f, "Invalid board hash: {}", msg),
            ValidationError::InvalidFen(msg) => write!(f, "Invalid FEN notation: {}", msg),
            ValidationError::InvalidMessageFormat(msg) => {
                write!(f, "Invalid message format: {}", msg)
            }
            ValidationError::BoardHashMismatch { expected, actual } => {
                write!(
                    f,
                    "Board hash mismatch: expected '{}', got '{}'",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// Validate a game invitation message
///
/// Checks that all fields in a GameInvite message are properly formatted and valid.
/// This includes validating the game ID format and ensuring suggested colors are valid.
///
/// # Arguments
///
/// * `invite` - The game invitation message to validate
///
/// # Returns
///
/// * `Ok(())` - If the invitation is valid
/// * `Err(ValidationError)` - If validation fails, with details about the error
///
/// # Examples
///
/// ```
/// use mate::messages::chess::{GameInvite, generate_game_id, validate_game_invite};
/// use mate::chess::Color;
///
/// let valid_invite = GameInvite::new(generate_game_id(), Some(Color::White));
/// assert!(validate_game_invite(&valid_invite).is_ok());
///
/// let invalid_invite = GameInvite::new("not-a-uuid".to_string(), None);
/// assert!(validate_game_invite(&invalid_invite).is_err());
/// ```
pub fn validate_game_invite(invite: &GameInvite) -> Result<(), ValidationError> {
    // Validate game ID format
    if !validate_game_id(&invite.game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Game ID '{}' is not a valid UUID format",
            invite.game_id
        )));
    }

    // Check for empty game ID
    if invite.game_id.trim().is_empty() {
        return Err(ValidationError::InvalidGameId(
            "Game ID cannot be empty".to_string(),
        ));
    }

    // Validate suggested color is a reasonable value (Color enum is already validated by type system)
    // Additional business logic validation could go here if needed

    Ok(())
}

/// Validate a chess move message
///
/// Performs comprehensive validation of a Move message including:
/// - Game ID format validation
/// - Chess move format validation  
/// - Board state hash format validation
/// - Ensures all required fields are present and properly formatted
///
/// # Arguments
///
/// * `msg` - The chess move message to validate
///
/// # Returns
///
/// * `Ok(())` - If the move message is valid
/// * `Err(ValidationError)` - If validation fails, with details about the error
///
/// # Examples
///
/// ```
/// use mate::messages::chess::{Move, generate_game_id, validate_move_message};
/// use mate::chess::Board;
/// use mate::messages::chess::hash_board_state;
///
/// let board = Board::new();
/// let valid_move = Move::new(
///     generate_game_id(),
///     "e2e4".to_string(),
///     hash_board_state(&board)
/// );
/// assert!(validate_move_message(&valid_move).is_ok());
/// ```
pub fn validate_move_message(msg: &Move) -> Result<(), ValidationError> {
    // Validate game ID format
    if !validate_game_id(&msg.game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Game ID '{}' is not a valid UUID format",
            msg.game_id
        )));
    }

    // Validate chess move format
    validate_chess_move_format(&msg.chess_move)?;

    // Validate board state hash format
    validate_board_hash_format(&msg.board_state_hash)?;

    Ok(())
}

/// Validate chess move format
///
/// Validates that a chess move string conforms to expected formats:
/// - Standard algebraic notation: "e2e4", "d7d8q" (with promotion)
/// - Castling notation: "O-O" (kingside), "O-O-O" (queenside)
/// - Ensures the move string is properly formatted and could represent a valid chess move
///
/// # Arguments
///
/// * `chess_move` - The chess move string to validate
///
/// # Returns
///
/// * `Ok(())` - If the move format is valid
/// * `Err(ValidationError)` - If the move format is invalid
///
/// # Examples
///
/// ```
/// use mate::messages::chess::validate_chess_move_format;
///
/// // Valid move formats
/// assert!(validate_chess_move_format("e2e4").is_ok());
/// assert!(validate_chess_move_format("d7d8q").is_ok());
/// assert!(validate_chess_move_format("O-O").is_ok());
/// assert!(validate_chess_move_format("O-O-O").is_ok());
///
/// // Invalid move formats
/// assert!(validate_chess_move_format("invalid").is_err());
/// assert!(validate_chess_move_format("").is_err());
/// ```
pub fn validate_chess_move_format(chess_move: &str) -> Result<(), ValidationError> {
    // Check for empty or whitespace-only moves
    if chess_move.trim().is_empty() {
        return Err(ValidationError::InvalidMove(
            "Chess move cannot be empty".to_string(),
        ));
    }

    let trimmed_move = chess_move.trim();

    // Check for castling moves first
    if trimmed_move == "O-O" || trimmed_move == "O-O-O" {
        return Ok(());
    }

    // Check for standard algebraic notation
    if validate_standard_algebraic_notation(trimmed_move) {
        return Ok(());
    }

    // If we get here, the move format is invalid
    Err(ValidationError::InvalidMove(format!(
        "Invalid chess move format '{}'. Expected formats: 'e2e4', 'd7d8q' (with promotion), 'O-O' (kingside castling), or 'O-O-O' (queenside castling)",
        chess_move
    )))
}

/// Helper function to validate standard algebraic notation
///
/// Validates moves in the format: [file][rank][file][rank][promotion?]
/// Examples: "e2e4", "a7a8q", "h1g1"
fn validate_standard_algebraic_notation(chess_move: &str) -> bool {
    // Standard move: 4 characters (e2e4) or 5 characters with promotion (e7e8q)
    if chess_move.len() != 4 && chess_move.len() != 5 {
        return false;
    }

    let chars: Vec<char> = chess_move.chars().collect();

    // Validate source square (first two characters)
    if !validate_square_notation(&chars[0..2]) {
        return false;
    }

    // Validate destination square (characters 2-3)
    if !validate_square_notation(&chars[2..4]) {
        return false;
    }

    // If 5 characters, validate promotion piece
    if chess_move.len() == 5 {
        let promotion_char = chars[4];
        if !matches!(
            promotion_char,
            'q' | 'r' | 'b' | 'n' | 'Q' | 'R' | 'B' | 'N'
        ) {
            return false;
        }
    }

    true
}

/// Helper function to validate square notation (e.g., "e2", "a8")
fn validate_square_notation(square: &[char]) -> bool {
    if square.len() != 2 {
        return false;
    }

    let file = square[0];
    let rank = square[1];

    // Validate file (a-h)
    if !matches!(file, 'a'..='h') {
        return false;
    }

    // Validate rank (1-8)
    if !matches!(rank, '1'..='8') {
        return false;
    }

    true
}

/// Validate board state hash format
///
/// Ensures that a board state hash string is properly formatted as a
/// SHA-256 hash (64-character lowercase hexadecimal string).
///
/// # Arguments
///
/// * `hash` - The hash string to validate
///
/// # Returns
///
/// * `Ok(())` - If the hash format is valid
/// * `Err(ValidationError)` - If the hash format is invalid
fn validate_board_hash_format(hash: &str) -> Result<(), ValidationError> {
    // Check for empty hash
    if hash.trim().is_empty() {
        return Err(ValidationError::InvalidBoardHash(
            "Board state hash cannot be empty".to_string(),
        ));
    }

    let trimmed_hash = hash.trim();

    // SHA-256 hash should be exactly 64 characters
    if trimmed_hash.len() != 64 {
        return Err(ValidationError::InvalidBoardHash(format!(
            "Board state hash must be exactly 64 characters (SHA-256), got {} characters",
            trimmed_hash.len()
        )));
    }

    // Hash should contain only hexadecimal characters
    if !trimmed_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ValidationError::InvalidBoardHash(format!(
            "Board state hash '{}' contains invalid characters (must be hexadecimal)",
            trimmed_hash
        )));
    }

    Ok(())
}

/// Validate a game accept message
///
/// Validates that a GameAccept message has a properly formatted game ID.
/// The accepted color is validated by the type system (Color enum).
///
/// # Arguments
///
/// * `accept` - The game accept message to validate
///
/// # Returns
///
/// * `Ok(())` - If the accept message is valid
/// * `Err(ValidationError)` - If validation fails
pub fn validate_game_accept(accept: &GameAccept) -> Result<(), ValidationError> {
    // Validate game ID format
    if !validate_game_id(&accept.game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Game ID '{}' is not a valid UUID format",
            accept.game_id
        )));
    }

    // Check for empty game ID
    if accept.game_id.trim().is_empty() {
        return Err(ValidationError::InvalidGameId(
            "Game ID cannot be empty".to_string(),
        ));
    }

    Ok(())
}

/// Validate a game decline message
///
/// Validates that a GameDecline message has a properly formatted game ID
/// and that the optional reason field is reasonable if provided.
///
/// # Arguments
///
/// * `decline` - The game decline message to validate
///
/// # Returns
///
/// * `Ok(())` - If the decline message is valid
/// * `Err(ValidationError)` - If validation fails
pub fn validate_game_decline(decline: &GameDecline) -> Result<(), ValidationError> {
    // Validate game ID format
    if !validate_game_id(&decline.game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Game ID '{}' is not a valid UUID format",
            decline.game_id
        )));
    }

    // Check for empty game ID
    if decline.game_id.trim().is_empty() {
        return Err(ValidationError::InvalidGameId(
            "Game ID cannot be empty".to_string(),
        ));
    }

    // Validate reason field if provided
    if let Some(reason) = &decline.reason {
        // Check for excessively long reasons (reasonable limit: 1000 characters)
        if reason.len() > 1000 {
            return Err(ValidationError::InvalidMessageFormat(format!(
                "Decline reason is too long ({} characters, maximum 1000)",
                reason.len()
            )));
        }

        // Check for empty string (should be None instead)
        if reason.trim().is_empty() {
            return Err(ValidationError::InvalidMessageFormat(
                "Decline reason should be None instead of empty string".to_string(),
            ));
        }
    }

    Ok(())
}

/// Validate a sync request message
///
/// Validates that a SyncRequest message has a properly formatted game ID.
///
/// # Arguments
///
/// * `request` - The sync request message to validate
///
/// # Returns
///
/// * `Ok(())` - If the request is valid
/// * `Err(ValidationError)` - If validation fails
pub fn validate_sync_request(request: &SyncRequest) -> Result<(), ValidationError> {
    // Validate game ID format
    if !validate_game_id(&request.game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Game ID '{}' is not a valid UUID format",
            request.game_id
        )));
    }

    // Check for empty game ID
    if request.game_id.trim().is_empty() {
        return Err(ValidationError::InvalidGameId(
            "Game ID cannot be empty".to_string(),
        ));
    }

    Ok(())
}

/// Validate a sync response message
///
/// Validates that a SyncResponse message has properly formatted fields including:
/// - Valid game ID format
/// - Valid FEN notation for board state
/// - Valid move history (each move follows chess move format)
/// - Valid board state hash format
///
/// # Arguments
///
/// * `response` - The sync response message to validate
///
/// # Returns
///
/// * `Ok(())` - If the response is valid
/// * `Err(ValidationError)` - If validation fails
pub fn validate_sync_response(response: &SyncResponse) -> Result<(), ValidationError> {
    // Validate game ID format
    if !validate_game_id(&response.game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Game ID '{}' is not a valid UUID format",
            response.game_id
        )));
    }

    // Check for empty game ID
    if response.game_id.trim().is_empty() {
        return Err(ValidationError::InvalidGameId(
            "Game ID cannot be empty".to_string(),
        ));
    }

    // Validate FEN notation for board state
    if response.board_state.trim().is_empty() {
        return Err(ValidationError::InvalidFen(
            "Board state FEN cannot be empty".to_string(),
        ));
    }

    // Try to parse the FEN to validate its format
    use crate::chess::Board;
    if let Err(_) = Board::from_fen(&response.board_state) {
        return Err(ValidationError::InvalidFen(format!(
            "Invalid FEN notation: '{}'",
            response.board_state
        )));
    }

    // Validate each move in the move history
    for (index, chess_move) in response.move_history.iter().enumerate() {
        if let Err(e) = validate_chess_move_format(chess_move) {
            return Err(ValidationError::InvalidMove(format!(
                "Invalid move at position {}: {}",
                index, e
            )));
        }
    }

    // Validate board state hash format
    validate_board_hash_format(&response.board_state_hash)?;

    // Validate that the board state hash matches the provided board state
    let board = Board::from_fen(&response.board_state).map_err(|_| {
        ValidationError::InvalidFen(format!(
            "Could not parse board state for hash verification: '{}'",
            response.board_state
        ))
    })?;

    if !verify_board_hash(&board, &response.board_state_hash) {
        let computed_hash = hash_board_state(&board);
        return Err(ValidationError::BoardHashMismatch {
            expected: response.board_state_hash.clone(),
            actual: computed_hash,
        });
    }

    Ok(())
}

/// Validate a move acknowledgment message
///
/// Validates that a MoveAck message has a properly formatted game ID.
/// The optional move_id field is checked for reasonable length if provided.
///
/// # Arguments
///
/// * `ack` - The move acknowledgment message to validate
///
/// # Returns
///
/// * `Ok(())` - If the acknowledgment is valid
/// * `Err(ValidationError)` - If validation fails
pub fn validate_move_ack(ack: &MoveAck) -> Result<(), ValidationError> {
    // Validate game ID
    if !validate_game_id(&ack.game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Invalid game ID format: '{}'",
            ack.game_id
        )));
    }

    // Validate move ID format if present
    if let Some(move_id) = &ack.move_id {
        if move_id.is_empty() {
            return Err(ValidationError::InvalidMessageFormat(
                "Move ID cannot be empty when present".to_string(),
            ));
        }

        // Basic format validation for move ID
        if move_id.len() > 64 {
            return Err(ValidationError::InvalidMessageFormat(
                "Move ID is too long (max 64 characters)".to_string(),
            ));
        }

        // Check for valid characters (alphanumeric, hyphens, underscores)
        if !move_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(ValidationError::InvalidMessageFormat(
                "Move ID can only contain alphanumeric characters, hyphens, and underscores"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

// =============================================================================
// Integration Functions for Chess Module
// =============================================================================

/// Create a chess move message from a game ID, chess move, and board state
///
/// This function integrates the chess module's Move type with the message protocol,
/// creating a properly formatted Move message with board state verification.
///
/// # Arguments
///
/// * `game_id` - Unique identifier for the chess game
/// * `chess_move` - The chess move from the chess module
/// * `board` - Current board state after the move for hash generation
///
/// # Returns
///
/// A `Message::Move` variant containing the move information and board state hash
///
/// # Examples
///
/// ```
/// use mate::chess::{Board, Move as ChessMove, Position};
/// use mate::messages::chess::{create_move_message, generate_game_id};
///
/// let mut board = Board::new();
/// let chess_move = ChessMove::simple(
///     Position::from_algebraic("e2").unwrap(),
///     Position::from_algebraic("e4").unwrap()
/// ).unwrap();
///
/// let game_id = generate_game_id();
/// let message = create_move_message(&game_id, &chess_move, &board);
/// ```
pub fn create_move_message(
    game_id: &str,
    chess_move: &crate::chess::Move,
    board: &Board,
) -> crate::messages::types::Message {
    let move_string = chess_move.to_string();
    let board_hash = hash_board_state(board);

    crate::messages::types::Message::new_move(game_id.to_string(), move_string, board_hash)
}

/// Create a synchronization response message from game state
///
/// This function creates a comprehensive sync response containing the current
/// board state, move history, and verification hash for game synchronization.
///
/// # Arguments
///
/// * `game_id` - Unique identifier for the chess game
/// * `board` - Current board state to be synchronized
/// * `history` - Complete move history from the chess module
///
/// # Returns
///
/// A `Message::SyncResponse` variant containing the complete game state
///
/// # Examples
///
/// ```
/// use mate::chess::{Board, Move as ChessMove, Position};
/// use mate::messages::chess::{create_sync_response, generate_game_id};
///
/// let board = Board::new();
/// let history = vec![
///     ChessMove::simple(
///         Position::from_algebraic("e2").unwrap(),
///         Position::from_algebraic("e4").unwrap()
///     ).unwrap(),
/// ];
///
/// let game_id = generate_game_id();
/// let message = create_sync_response(&game_id, &board, &history);
/// ```
pub fn create_sync_response(
    game_id: &str,
    board: &Board,
    history: &[crate::chess::Move],
) -> crate::messages::types::Message {
    let board_state = board.to_fen();
    let move_history: Vec<String> = history.iter().map(|mv| mv.to_string()).collect();
    let board_hash = hash_board_state(board);

    crate::messages::types::Message::new_sync_response(
        game_id.to_string(),
        board_state,
        move_history,
        board_hash,
    )
}

/// Apply a chess move from a message to a board
///
/// This function bridges the message protocol and chess module by parsing
/// a move message and applying it to the board with proper error handling.
///
/// # Arguments
///
/// * `board` - Mutable reference to the board to apply the move to
/// * `move_msg` - Move message containing the move string and verification hash
///
/// # Returns
///
/// * `Ok(())` if the move was successfully applied and verified
/// * `Err(ChessError)` if the move parsing, application, or verification failed
///
/// # Errors
///
/// This function can return various `ChessError` variants:
/// - `InvalidMove` - If the move string cannot be parsed
/// - `BoardStateError` - If board state hash verification fails
/// - Any chess module errors from move application
///
/// # Examples
///
/// ```
/// use mate::chess::Board;
/// use mate::messages::chess::{Move, apply_move_from_message};
///
/// let mut board = Board::new();
/// let move_msg = Move::new(
///     "game-123".to_string(),
///     "e2e4".to_string(),
///     "expected_hash".to_string(),
/// );
///
/// match apply_move_from_message(&mut board, &move_msg) {
///     Ok(()) => println!("Move applied successfully"),
///     Err(e) => eprintln!("Failed to apply move: {}", e),
/// }
/// ```
pub fn apply_move_from_message(
    board: &mut Board,
    move_msg: &Move,
) -> Result<(), crate::chess::ChessError> {
    // Parse the move string to a chess module Move
    let chess_move =
        match crate::chess::Move::from_str_with_color(&move_msg.chess_move, board.active_color()) {
            Ok(mv) => mv,
            Err(e) => {
                return Err(crate::chess::ChessError::InvalidMove(format!(
                    "Failed to parse move '{}': {}",
                    move_msg.chess_move, e
                )))
            }
        };

    // Apply the move to the board
    board.make_move(chess_move)?;

    // Verify the board state hash matches the expected hash
    let actual_hash = hash_board_state(board);
    if actual_hash != move_msg.board_state_hash {
        return Err(crate::chess::ChessError::BoardStateError(format!(
            "Board state hash mismatch after move '{}'. Expected: {}, Actual: {}",
            move_msg.chess_move, move_msg.board_state_hash, actual_hash
        )));
    }

    Ok(())
}
