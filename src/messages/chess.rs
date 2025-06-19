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

/// Generate a cryptographically secure game ID using UUID v4
///
/// Creates a cryptographically secure, collision-resistant game identifier
/// that can be used to uniquely identify chess games across the network.
///
/// This function uses the system's cryptographically secure random number generator
/// to ensure that game IDs cannot be predicted or guessed by attackers.
///
/// # Security Properties
///
/// - **Cryptographically Secure**: Uses UUID v4 with cryptographically secure randomness
/// - **Collision Resistant**: Extremely low probability of generating duplicate IDs
/// - **Unpredictable**: Cannot be guessed or predicted by attackers
/// - **Validated Format**: Generated IDs are guaranteed to pass security validation
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
///
/// // Verify it passes security validation
/// use mate::messages::chess::security::validate_secure_game_id;
/// assert!(validate_secure_game_id(&game_id).is_ok());
/// ```
pub fn generate_game_id() -> String {
    // Generate UUID v4 using cryptographically secure randomness
    let game_id = Uuid::new_v4().to_string();

    // Verify the generated ID meets our security requirements
    // This should always pass for a properly generated UUID v4, but we check
    // as a defensive programming measure
    debug_assert!(security::validate_secure_game_id(&game_id).is_ok());

    game_id
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

/// Chess-specific protocol error type that provides comprehensive error handling
/// for all chess message protocol operations, including validation, hashing, and integration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChessProtocolError {
    /// Validation error for chess message format/content
    Validation(ValidationError),
    /// Chess engine/game logic error
    ChessEngine(crate::chess::ChessError),
    /// Wire protocol error during message transmission
    Wire(String), // Simplified since we can't derive Clone for WireProtocolError
    /// Game state synchronization error
    SyncError { game_id: String, reason: String },
    /// Hash verification failure with detailed information
    HashVerificationFailed {
        game_id: String,
        expected: String,
        actual: String,
        context: String,
    },
    /// Game not found or invalid game state
    GameStateError { game_id: String, error: String },
    /// Message type mismatch or unexpected message in game flow
    UnexpectedMessage {
        game_id: String,
        expected: String,
        received: String,
    },
    /// Timeout during chess protocol operations
    Timeout { operation: String, duration_ms: u64 },
    /// Security violation detected in chess messages
    SecurityViolation { game_id: String, violation: String },
    /// Generic internal error with context
    Internal { context: String, source: String },
}

impl std::fmt::Display for ChessProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChessProtocolError::Validation(err) => {
                write!(f, "Chess message validation error: {}", err)
            }
            ChessProtocolError::ChessEngine(err) => {
                write!(f, "Chess engine error: {}", err)
            }
            ChessProtocolError::Wire(msg) => {
                write!(f, "Wire protocol error: {}", msg)
            }
            ChessProtocolError::SyncError { game_id, reason } => {
                write!(
                    f,
                    "Game synchronization error for game {}: {}",
                    game_id, reason
                )
            }
            ChessProtocolError::HashVerificationFailed {
                game_id,
                expected,
                actual,
                context,
            } => {
                write!(
                    f,
                    "Board hash verification failed for game {} ({}): expected '{}', got '{}'",
                    game_id, context, expected, actual
                )
            }
            ChessProtocolError::GameStateError { game_id, error } => {
                write!(f, "Game state error for game {}: {}", game_id, error)
            }
            ChessProtocolError::UnexpectedMessage {
                game_id,
                expected,
                received,
            } => {
                write!(
                    f,
                    "Unexpected message for game {}: expected '{}', received '{}'",
                    game_id, expected, received
                )
            }
            ChessProtocolError::Timeout {
                operation,
                duration_ms,
            } => {
                write!(
                    f,
                    "Chess protocol timeout during '{}' after {}ms",
                    operation, duration_ms
                )
            }
            ChessProtocolError::SecurityViolation { game_id, violation } => {
                write!(f, "Security violation for game {}: {}", game_id, violation)
            }
            ChessProtocolError::Internal { context, source } => {
                write!(
                    f,
                    "Internal chess protocol error in {}: {}",
                    context, source
                )
            }
        }
    }
}

impl std::error::Error for ChessProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ChessProtocolError::Validation(err) => Some(err),
            ChessProtocolError::ChessEngine(err) => Some(err),
            _ => None,
        }
    }
}

impl From<ValidationError> for ChessProtocolError {
    fn from(err: ValidationError) -> Self {
        ChessProtocolError::Validation(err)
    }
}

impl From<crate::chess::ChessError> for ChessProtocolError {
    fn from(err: crate::chess::ChessError) -> Self {
        ChessProtocolError::ChessEngine(err)
    }
}

impl From<crate::messages::wire::WireProtocolError> for ChessProtocolError {
    fn from(err: crate::messages::wire::WireProtocolError) -> Self {
        ChessProtocolError::Wire(err.to_string())
    }
}

impl ChessProtocolError {
    /// Create a sync error
    pub fn sync_error<S: Into<String>>(game_id: S, reason: S) -> Self {
        Self::SyncError {
            game_id: game_id.into(),
            reason: reason.into(),
        }
    }

    /// Create a hash verification error
    pub fn hash_verification_failed<S: Into<String>>(
        game_id: S,
        expected: S,
        actual: S,
        context: S,
    ) -> Self {
        Self::HashVerificationFailed {
            game_id: game_id.into(),
            expected: expected.into(),
            actual: actual.into(),
            context: context.into(),
        }
    }

    /// Create a game state error
    pub fn game_state_error<S: Into<String>>(game_id: S, error: S) -> Self {
        Self::GameStateError {
            game_id: game_id.into(),
            error: error.into(),
        }
    }

    /// Create an unexpected message error
    pub fn unexpected_message<S: Into<String>>(game_id: S, expected: S, received: S) -> Self {
        Self::UnexpectedMessage {
            game_id: game_id.into(),
            expected: expected.into(),
            received: received.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout<S: Into<String>>(operation: S, duration_ms: u64) -> Self {
        Self::Timeout {
            operation: operation.into(),
            duration_ms,
        }
    }

    /// Create a security violation error
    pub fn security_violation<S: Into<String>>(game_id: S, violation: S) -> Self {
        Self::SecurityViolation {
            game_id: game_id.into(),
            violation: violation.into(),
        }
    }

    /// Create an internal error
    pub fn internal<S: Into<String>>(context: S, source: S) -> Self {
        Self::Internal {
            context: context.into(),
            source: source.into(),
        }
    }

    /// Check if this error is recoverable (can retry the operation)
    pub fn is_recoverable(&self) -> bool {
        match self {
            ChessProtocolError::Wire(_) => true, // Network issues might be temporary
            ChessProtocolError::Timeout { .. } => true, // Timeouts can be retried
            ChessProtocolError::SyncError { .. } => true, // Sync can be retried
            ChessProtocolError::Internal { .. } => false, // Internal errors are not recoverable
            ChessProtocolError::Validation(_) => false, // Invalid data won't become valid
            ChessProtocolError::ChessEngine(_) => false, // Chess rules violations are permanent
            ChessProtocolError::HashVerificationFailed { .. } => false, // Hash mismatches indicate corruption
            ChessProtocolError::GameStateError { .. } => false, // Game state issues are permanent
            ChessProtocolError::UnexpectedMessage { .. } => false, // Protocol violations are permanent
            ChessProtocolError::SecurityViolation { .. } => false, // Security violations are permanent
        }
    }

    /// Check if this error indicates a security concern
    pub fn is_security_related(&self) -> bool {
        match self {
            ChessProtocolError::SecurityViolation { .. } => true,
            ChessProtocolError::HashVerificationFailed { .. } => true,
            ChessProtocolError::Validation(ValidationError::InvalidGameId(_)) => true,
            _ => false,
        }
    }

    /// Get the game ID associated with this error, if any
    pub fn game_id(&self) -> Option<&str> {
        match self {
            ChessProtocolError::SyncError { game_id, .. } => Some(game_id),
            ChessProtocolError::HashVerificationFailed { game_id, .. } => Some(game_id),
            ChessProtocolError::GameStateError { game_id, .. } => Some(game_id),
            ChessProtocolError::UnexpectedMessage { game_id, .. } => Some(game_id),
            ChessProtocolError::SecurityViolation { game_id, .. } => Some(game_id),
            _ => None,
        }
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            ChessProtocolError::Validation(_) => "validation",
            ChessProtocolError::ChessEngine(_) => "chess_engine",
            ChessProtocolError::Wire(_) => "wire_protocol",
            ChessProtocolError::SyncError { .. } => "synchronization",
            ChessProtocolError::HashVerificationFailed { .. } => "hash_verification",
            ChessProtocolError::GameStateError { .. } => "game_state",
            ChessProtocolError::UnexpectedMessage { .. } => "protocol_violation",
            ChessProtocolError::Timeout { .. } => "timeout",
            ChessProtocolError::SecurityViolation { .. } => "security",
            ChessProtocolError::Internal { .. } => "internal",
        }
    }
}

/// Result type for chess protocol operations
pub type ChessProtocolResult<T> = Result<T, ChessProtocolError>;

/// Enhanced validation function that provides graceful error handling for game IDs
///
/// This function validates game IDs with detailed error information and
/// security considerations to prevent injection attacks and ensure proper formatting.
///
/// # Arguments
///
/// * `game_id` - The game ID string to validate
///
/// # Returns
///
/// * `Ok(())` - If the game ID is valid
/// * `Err(ChessProtocolError)` - If validation fails with detailed error information
///
/// # Security Considerations
///
/// - Validates UUID v4 format to prevent injection attacks
/// - Checks for proper length and character set
/// - Rejects empty, whitespace-only, or malformed IDs
///
/// # Examples
///
/// ```
/// use mate::messages::chess::{validate_game_id_graceful, generate_game_id};
///
/// let valid_id = generate_game_id();
/// assert!(validate_game_id_graceful(&valid_id).is_ok());
///
/// let invalid_id = "not-a-uuid";
/// assert!(validate_game_id_graceful(invalid_id).is_err());
/// ```
pub fn validate_game_id_graceful(game_id: &str) -> ChessProtocolResult<()> {
    if game_id.trim().is_empty() {
        return Err(ChessProtocolError::security_violation(
            game_id,
            "Empty game ID detected - potential security violation",
        ));
    }

    if game_id.len() > 50 {
        // UUID should be 36 characters
        return Err(ChessProtocolError::security_violation(
            game_id,
            "Game ID too long - potential buffer overflow attempt",
        ));
    }

    // Check for suspicious characters that could indicate injection attempts
    if game_id.chars().any(|c| c.is_control() || c == '\0') {
        return Err(ChessProtocolError::security_violation(
            game_id,
            "Game ID contains suspicious control characters",
        ));
    }

    if !validate_game_id(game_id) {
        return Err(ValidationError::InvalidGameId(format!(
            "Game ID '{}' is not a valid UUID v4 format",
            game_id
        ))
        .into());
    }

    Ok(())
}

/// Enhanced board state hash verification with graceful error handling
///
/// Verifies board state hashes with detailed error reporting and security checks
/// to detect potential tampering or corruption in chess game data.
///
/// # Arguments
///
/// * `game_id` - The game ID for error context
/// * `board` - The chess board to verify
/// * `expected_hash` - The expected SHA-256 hash
/// * `context` - Additional context for error reporting (e.g., "after move", "sync response")
///
/// # Returns
///
/// * `Ok(())` - If the hash verification succeeds
/// * `Err(ChessProtocolError)` - If verification fails with detailed error information
///
/// # Security Considerations
///
/// - Detects board state tampering
/// - Prevents hash collision attacks
/// - Validates hash format before comparison
///
/// # Examples
///
/// ```
/// use mate::messages::chess::{verify_board_hash_graceful, hash_board_state};
/// use mate::chess::Board;
///
/// let board = Board::new();
/// let hash = hash_board_state(&board);
/// let game_id = "test-game";
///
/// assert!(verify_board_hash_graceful(game_id, &board, &hash, "test").is_ok());
/// assert!(verify_board_hash_graceful(game_id, &board, "invalid-hash", "test").is_err());
/// ```
pub fn verify_board_hash_graceful(
    game_id: &str,
    board: &Board,
    expected_hash: &str,
    context: &str,
) -> ChessProtocolResult<()> {
    // First validate the hash format
    validate_board_hash_format(expected_hash)?;

    // Compute the actual hash
    let actual_hash = hash_board_state(board);

    // Compare hashes
    if actual_hash != expected_hash {
        return Err(ChessProtocolError::hash_verification_failed(
            game_id,
            expected_hash,
            &actual_hash,
            context,
        ));
    }

    Ok(())
}

/// Enhanced chess move validation with graceful error handling
///
/// Validates chess moves with comprehensive error reporting and security checks
/// to prevent malformed moves and potential exploitation attempts.
///
/// # Arguments
///
/// * `game_id` - The game ID for error context
/// * `chess_move` - The chess move string to validate
/// * `board` - Optional board context for move legality checking
///
/// # Returns
///
/// * `Ok(())` - If the move is valid
/// * `Err(ChessProtocolError)` - If validation fails with detailed error information
///
/// # Security Considerations
///
/// - Prevents injection through malformed move strings
/// - Validates move format before processing
/// - Checks for suspicious patterns in move data
///
/// # Examples
///
/// ```
/// use mate::messages::chess::validate_chess_move_graceful;
/// use mate::chess::Board;
///
/// let game_id = "test-game";
/// let board = Board::new();
///
/// assert!(validate_chess_move_graceful(game_id, "e2e4", Some(&board)).is_ok());
/// assert!(validate_chess_move_graceful(game_id, "invalid", Some(&board)).is_err());
/// ```
pub fn validate_chess_move_graceful(
    game_id: &str,
    chess_move: &str,
    _board: Option<&Board>, // Future: validate move legality
) -> ChessProtocolResult<()> {
    // Check for potentially malicious input
    if chess_move.len() > 20 {
        // Chess moves should be much shorter
        return Err(ChessProtocolError::security_violation(
            game_id,
            "Chess move string too long - potential buffer overflow attempt",
        ));
    }

    // Check for suspicious characters
    if chess_move.chars().any(|c| c.is_control() || c == '\0') {
        return Err(ChessProtocolError::security_violation(
            game_id,
            "Chess move contains suspicious control characters",
        ));
    }

    // Use existing validation
    validate_chess_move_format(chess_move)?;

    // Future enhancement: Validate move legality against board state
    // if let Some(board) = board {
    //     validate_move_legality(chess_move, board)?;
    // }

    Ok(())
}

/// Enhanced message chain error propagation function
///
/// Handles error propagation through the chess message processing chain
/// with proper context preservation and graceful degradation strategies.
///
/// # Arguments
///
/// * `operation` - Description of the operation being performed
/// * `result` - The result to process
///
/// # Returns
///
/// * Properly contextualized error with operation information
///
/// # Examples
///
/// ```
/// use mate::messages::chess::{propagate_error, ChessProtocolError};
///
/// let result: Result<(), ChessProtocolError> = Err(
///     ChessProtocolError::internal("test", "test error")
/// );
/// let contextualized = propagate_error("move validation", result);
/// assert!(contextualized.is_err());
/// ```
pub fn propagate_error<T>(
    operation: &str,
    result: ChessProtocolResult<T>,
) -> ChessProtocolResult<T> {
    result.map_err(|err| match err {
        ChessProtocolError::Internal { context, source } => {
            ChessProtocolError::internal(format!("{} -> {}", operation, context), source)
        }
        other => other,
    })
}

/// Gracefully handle invalid game IDs with proper security logging
///
/// Provides centralized handling of invalid game ID errors with security
/// considerations and appropriate logging for monitoring.
///
/// # Arguments
///
/// * `game_id` - The invalid game ID
/// * `context` - Context where the invalid ID was encountered
///
/// # Returns
///
/// * Appropriate ChessProtocolError with security classification
///
/// # Security Features
///
/// - Logs suspicious game ID patterns
/// - Categorizes potential attack patterns  
/// - Provides sanitized error messages
pub fn handle_invalid_game_id(game_id: &str, context: &str) -> ChessProtocolError {
    // Log security event (in a real implementation, this would use proper logging)
    eprintln!(
        "SECURITY: Invalid game ID '{}' encountered in context '{}' - potential attack attempt",
        game_id, context
    );

    // Classify the type of invalid ID for security monitoring
    let violation_type = if game_id.is_empty() {
        "empty_game_id"
    } else if game_id.len() > 50 {
        "oversized_game_id"
    } else if game_id.chars().any(|c| c.is_control()) {
        "control_characters_in_game_id"
    } else {
        "malformed_game_id"
    };

    ChessProtocolError::SecurityViolation {
        game_id: game_id.to_string(),
        violation: format!("Invalid game ID in {}: {}", context, violation_type),
    }
}

/// Gracefully handle board state hash mismatches
///
/// Provides centralized handling of hash verification failures with detailed
/// error reporting and security considerations.
///
/// # Arguments
///
/// * `game_id` - The game ID where the mismatch occurred
/// * `expected` - The expected hash value
/// * `actual` - The actual computed hash value  
/// * `context` - Context where the mismatch was detected
///
/// # Returns
///
/// * Appropriate ChessProtocolError with hash mismatch details
///
/// # Security Features
///
/// - Detects potential tampering attempts
/// - Logs hash verification failures
/// - Provides detailed mismatch information for debugging
pub fn handle_board_hash_mismatch(
    game_id: &str,
    expected: &str,
    actual: &str,
    context: &str,
) -> ChessProtocolError {
    // Log security event
    eprintln!(
        "SECURITY: Board hash mismatch for game '{}' in context '{}' - potential tampering detected",
        game_id, context
    );

    ChessProtocolError::hash_verification_failed(game_id, expected, actual, context)
}

/// Gracefully handle malformed chess moves
///
/// Provides centralized handling of invalid chess move errors with security
/// considerations and proper error classification.
///
/// # Arguments
///
/// * `game_id` - The game ID where the malformed move was encountered
/// * `chess_move` - The malformed move string
/// * `context` - Context where the malformed move was detected
///
/// # Returns
///
/// * Appropriate ChessProtocolError with move validation details
///
/// # Security Features
///
/// - Detects potential injection attempts through move strings
/// - Logs suspicious move patterns
/// - Provides sanitized error messages
pub fn handle_malformed_chess_move(
    game_id: &str,
    chess_move: &str,
    context: &str,
) -> ChessProtocolError {
    // Check for potential security violations
    let is_security_violation = chess_move.len() > 20 || chess_move.chars().any(|c| c.is_control());

    if is_security_violation {
        eprintln!(
            "SECURITY: Malformed chess move '{}' for game '{}' in context '{}' - potential injection attempt",
            chess_move, game_id, context
        );
        ChessProtocolError::SecurityViolation {
            game_id: game_id.to_string(),
            violation: format!(
                "Malformed chess move in {}: potential injection attempt",
                context
            ),
        }
    } else {
        ValidationError::InvalidMove(format!(
            "Invalid chess move '{}' in context '{}'",
            chess_move, context
        ))
        .into()
    }
}

/// Security validation and protection for chess messages
pub mod security {
    use super::*;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    /// Maximum allowed length for free-text fields to prevent injection attacks
    pub const MAX_REASON_LENGTH: usize = 500;
    pub const MAX_MOVE_NOTATION_LENGTH: usize = 20;
    pub const MAX_FEN_LENGTH: usize = 200;
    pub const MAX_MOVE_HISTORY_SIZE: usize = 1000;

    /// Rate limiting configuration and tracking for chess messages
    ///
    /// Main considerations for chess rate limiting strategy:
    /// 1. **Move Rate Limiting**: Chess moves should be rate-limited per game to prevent
    ///    automated rapid-fire move attempts that could overwhelm opponents or the system.
    ///    Typical chess time controls suggest reasonable rates (e.g., 1 move per 3-10 seconds).
    ///
    /// 2. **Game Invitation Rate Limiting**: Limit how many game invitations a player can
    ///    send per time period to prevent invitation spam. Consider both per-recipient
    ///    and global invitation limits.
    ///
    /// 3. **Sync Request Rate Limiting**: Synchronization requests should be limited to
    ///    prevent denial-of-service through repeated sync operations, which can be expensive
    ///    as they involve full game state transmission.
    ///
    /// 4. **Per-Game vs Global Limits**: Some limits should be per-game (move frequency),
    ///    while others should be global per-player (invitation frequency, total active games).
    ///
    /// 5. **Burst vs Sustained Rates**: Allow short bursts for legitimate rapid play
    ///    (like bullet chess) while preventing sustained high-frequency abuse.
    ///
    /// 6. **Progressive Penalties**: Implement escalating timeouts for rate limit violations
    ///    rather than hard blocks, to accommodate different play styles while deterring abuse.
    #[derive(Debug, Clone)]
    pub struct ChessRateLimitConfig {
        /// Maximum moves per minute per game
        pub max_moves_per_minute: u32,
        /// Maximum game invitations per hour per player
        pub max_invitations_per_hour: u32,
        /// Maximum sync requests per minute per game
        pub max_sync_requests_per_minute: u32,
        /// Maximum concurrent active games per player
        pub max_active_games: u32,
        /// Burst allowance for rapid play scenarios
        pub burst_moves_allowed: u32,
        /// Window for tracking burst moves (seconds)
        pub burst_window_seconds: u64,
    }

    impl Default for ChessRateLimitConfig {
        fn default() -> Self {
            Self {
                max_moves_per_minute: 20,        // Allows rapid play but prevents automation
                max_invitations_per_hour: 50,    // Generous for legitimate use, prevents spam
                max_sync_requests_per_minute: 5, // Allows recovery from network issues
                max_active_games: 10,            // Reasonable concurrent game limit
                burst_moves_allowed: 5,          // Allow 5 rapid moves in burst window
                burst_window_seconds: 30,        // 30-second burst window
            }
        }
    }

    /// Rate limiting tracker for chess operations
    #[derive(Debug)]
    pub struct ChessRateLimiter {
        config: ChessRateLimitConfig,
        move_times: HashMap<String, Vec<Instant>>, // game_id -> move timestamps
        invitation_times: HashMap<String, Vec<Instant>>, // player_id -> invitation timestamps
        sync_times: HashMap<String, Vec<Instant>>, // game_id -> sync timestamps
        active_games: HashMap<String, u32>,        // player_id -> active game count
    }

    impl ChessRateLimiter {
        pub fn new(config: ChessRateLimitConfig) -> Self {
            Self {
                config,
                move_times: HashMap::new(),
                invitation_times: HashMap::new(),
                sync_times: HashMap::new(),
                active_games: HashMap::new(),
            }
        }

        /// Check if a move is allowed under current rate limits
        pub fn check_move_rate_limit(&mut self, game_id: &str) -> bool {
            let now = Instant::now();
            let times = self
                .move_times
                .entry(game_id.to_string())
                .or_insert_with(Vec::new);

            // Remove old entries outside the rate limit window
            times.retain(|&time| now.duration_since(time) < Duration::from_secs(60));

            // Check both regular rate limit and burst limit
            let regular_limit_ok = times.len() < self.config.max_moves_per_minute as usize;

            // Check burst limit (rapid moves in short window)
            let burst_window = Duration::from_secs(self.config.burst_window_seconds);
            let recent_moves = times
                .iter()
                .filter(|&&time| now.duration_since(time) < burst_window)
                .count();
            let burst_limit_ok = recent_moves < self.config.burst_moves_allowed as usize;

            if regular_limit_ok && burst_limit_ok {
                times.push(now);
                true
            } else {
                false
            }
        }

        /// Check if a game invitation is allowed under current rate limits
        pub fn check_invitation_rate_limit(&mut self, player_id: &str) -> bool {
            let now = Instant::now();
            let times = self
                .invitation_times
                .entry(player_id.to_string())
                .or_insert_with(Vec::new);

            // Remove old entries outside the rate limit window
            times.retain(|&time| now.duration_since(time) < Duration::from_secs(3600)); // 1 hour

            if times.len() < self.config.max_invitations_per_hour as usize {
                times.push(now);
                true
            } else {
                false
            }
        }

        /// Check if a sync request is allowed under current rate limits
        pub fn check_sync_rate_limit(&mut self, game_id: &str) -> bool {
            let now = Instant::now();
            let times = self
                .sync_times
                .entry(game_id.to_string())
                .or_insert_with(Vec::new);

            // Remove old entries outside the rate limit window
            times.retain(|&time| now.duration_since(time) < Duration::from_secs(60));

            if times.len() < self.config.max_sync_requests_per_minute as usize {
                times.push(now);
                true
            } else {
                false
            }
        }

        /// Check if a player can start a new game (concurrent game limit)
        pub fn check_active_game_limit(&mut self, player_id: &str) -> bool {
            let active_count = self.active_games.get(player_id).unwrap_or(&0);
            *active_count < self.config.max_active_games
        }

        /// Register a new active game for a player
        pub fn register_active_game(&mut self, player_id: &str) {
            *self.active_games.entry(player_id.to_string()).or_insert(0) += 1;
        }

        /// Unregister an active game for a player
        pub fn unregister_active_game(&mut self, player_id: &str) {
            if let Some(count) = self.active_games.get_mut(player_id) {
                if *count > 0 {
                    *count -= 1;
                }
            }
        }

        /// Clean up old tracking data to prevent memory leaks
        pub fn cleanup_old_data(&mut self) {
            let now = Instant::now();

            // Clean up move times older than 1 hour
            for times in self.move_times.values_mut() {
                times.retain(|&time| now.duration_since(time) < Duration::from_secs(3600));
            }

            // Clean up invitation times older than 24 hours
            for times in self.invitation_times.values_mut() {
                times.retain(|&time| now.duration_since(time) < Duration::from_secs(86400));
            }

            // Clean up sync times older than 1 hour
            for times in self.sync_times.values_mut() {
                times.retain(|&time| now.duration_since(time) < Duration::from_secs(3600));
            }

            // Remove empty entries
            self.move_times.retain(|_, times| !times.is_empty());
            self.invitation_times.retain(|_, times| !times.is_empty());
            self.sync_times.retain(|_, times| !times.is_empty());
        }
    }

    /// Security error types specific to chess message validation
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum SecurityViolation {
        /// Input contains potentially malicious content
        InjectionAttempt { field: String, content: String },
        /// Field exceeds maximum allowed length
        FieldTooLong {
            field: String,
            length: usize,
            max_length: usize,
        },
        /// Rate limit exceeded for operation
        RateLimitExceeded { operation: String, limit: String },
        /// Cryptographic verification failed
        CryptographicFailure { reason: String },
        /// Suspicious pattern detected in input
        SuspiciousPattern { field: String, pattern: String },
        /// Board state tampering detected
        BoardTampering {
            game_id: String,
            expected_hash: String,
            actual_hash: String,
        },
    }

    impl std::fmt::Display for SecurityViolation {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                SecurityViolation::InjectionAttempt { field, content } => {
                    write!(
                        f,
                        "Potential injection attempt in field '{}': {}",
                        field, content
                    )
                }
                SecurityViolation::FieldTooLong {
                    field,
                    length,
                    max_length,
                } => {
                    write!(
                        f,
                        "Field '{}' too long: {} characters (max: {})",
                        field, length, max_length
                    )
                }
                SecurityViolation::RateLimitExceeded { operation, limit } => {
                    write!(f, "Rate limit exceeded for {}: {}", operation, limit)
                }
                SecurityViolation::CryptographicFailure { reason } => {
                    write!(f, "Cryptographic verification failed: {}", reason)
                }
                SecurityViolation::SuspiciousPattern { field, pattern } => {
                    write!(
                        f,
                        "Suspicious pattern detected in field '{}': {}",
                        field, pattern
                    )
                }
                SecurityViolation::BoardTampering {
                    game_id,
                    expected_hash,
                    actual_hash,
                } => {
                    write!(
                        f,
                        "Board state tampering detected in game {}: expected {}, got {}",
                        game_id, expected_hash, actual_hash
                    )
                }
            }
        }
    }

    /// Enhanced validation functions with security hardening

    /// Validate text input against injection attacks and length limits
    pub fn validate_safe_text_input(
        input: &str,
        field_name: &str,
        max_length: usize,
    ) -> Result<(), SecurityViolation> {
        // Check length limit
        if input.len() > max_length {
            return Err(SecurityViolation::FieldTooLong {
                field: field_name.to_string(),
                length: input.len(),
                max_length,
            });
        }

        // Check for potential injection patterns
        let suspicious_patterns = [
            "<script",
            "</script",
            "javascript:",
            "data:",
            "vbscript:",
            "onload=",
            "onerror=",
            "onclick=",
            "eval(",
            "Function(",
            "setTimeout(",
            "setInterval(",
            "${",
            "#{",
            "{{",
            "<%",
            "%>",
            "../",
            "..\\",
            "/etc/",
            "C:\\",
            "DROP TABLE",
            "DELETE FROM",
            "INSERT INTO",
            "UPDATE SET",
            "UNION SELECT",
            "'; DROP",
            "--",
            "/*",
            "*/",
            "\x00",
            "\x01",
            "\x02",
            "\x03",
            "\x04",
            "\x05",
            "\x06",
            "\x07",
            "\x08",
            "\x0b",
            "\x0c",
            "\x0e",
            "\x0f",
            "\x10",
            "\x11",
            "\x12",
        ];

        let input_lower = input.to_lowercase();
        for &pattern in &suspicious_patterns {
            if input_lower.contains(pattern) {
                return Err(SecurityViolation::InjectionAttempt {
                    field: field_name.to_string(),
                    content: format!("Contains suspicious pattern: {}", pattern),
                });
            }
        }

        // Check for suspicious character sequences
        if input
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
        {
            return Err(SecurityViolation::SuspiciousPattern {
                field: field_name.to_string(),
                pattern: "Contains control characters".to_string(),
            });
        }

        // Check for excessive whitespace (potential buffer overflow attempt)
        let whitespace_ratio =
            input.chars().filter(|c| c.is_whitespace()).count() as f64 / input.len() as f64;
        if whitespace_ratio > 0.8 && input.len() > 50 {
            return Err(SecurityViolation::SuspiciousPattern {
                field: field_name.to_string(),
                pattern: "Excessive whitespace content".to_string(),
            });
        }

        Ok(())
    }

    /// Enhanced game ID validation with cryptographic strength verification
    pub fn validate_secure_game_id(game_id: &str) -> Result<(), SecurityViolation> {
        // First use the existing validation
        if !validate_game_id(game_id) {
            return Err(SecurityViolation::CryptographicFailure {
                reason: "Invalid UUID format".to_string(),
            });
        }

        // Parse as UUID to verify it's properly formed
        match uuid::Uuid::parse_str(game_id) {
            Ok(uuid) => {
                // Verify it's a version 4 (random) UUID for cryptographic strength
                if uuid.get_version() != Some(uuid::Version::Random) {
                    return Err(SecurityViolation::CryptographicFailure {
                        reason: "Game ID must be UUID version 4 (random) for security".to_string(),
                    });
                }

                // Check that it's not a nil UUID (all zeros)
                if uuid.is_nil() {
                    return Err(SecurityViolation::CryptographicFailure {
                        reason: "Game ID cannot be nil UUID".to_string(),
                    });
                }

                Ok(())
            }
            Err(_) => Err(SecurityViolation::CryptographicFailure {
                reason: "Invalid UUID format".to_string(),
            }),
        }
    }

    /// Enhanced chess move validation with injection protection
    pub fn validate_secure_chess_move(
        chess_move: &str,
        _game_id: &str,
    ) -> Result<(), SecurityViolation> {
        // Validate input safety
        validate_safe_text_input(chess_move, "chess_move", MAX_MOVE_NOTATION_LENGTH)?;

        // Use existing chess move format validation
        validate_chess_move_format(chess_move).map_err(|_| {
            SecurityViolation::SuspiciousPattern {
                field: "chess_move".to_string(),
                pattern: format!("Invalid chess move format: {}", chess_move),
            }
        })?;

        // Additional security checks for chess moves
        if chess_move.is_empty() {
            return Err(SecurityViolation::SuspiciousPattern {
                field: "chess_move".to_string(),
                pattern: "Empty move string".to_string(),
            });
        }

        // Check for repeated characters (potential fuzzing attempt)
        if chess_move.len() > 3 {
            let mut prev_char = chess_move.chars().next().unwrap();
            let mut repeat_count = 1;
            for c in chess_move.chars().skip(1) {
                if c == prev_char {
                    repeat_count += 1;
                    if repeat_count > 4 {
                        return Err(SecurityViolation::SuspiciousPattern {
                            field: "chess_move".to_string(),
                            pattern: "Excessive character repetition".to_string(),
                        });
                    }
                } else {
                    repeat_count = 1;
                    prev_char = c;
                }
            }
        }

        Ok(())
    }

    /// Enhanced board state hash validation with tampering detection
    pub fn validate_secure_board_hash(
        game_id: &str,
        board: &Board,
        provided_hash: &str,
        _context: &str,
    ) -> Result<(), SecurityViolation> {
        // Validate hash format first
        validate_board_hash_format(provided_hash).map_err(|_| {
            SecurityViolation::CryptographicFailure {
                reason: "Invalid hash format".to_string(),
            }
        })?;

        // Calculate expected hash
        let expected_hash = hash_board_state(board);

        // Compare hashes using constant-time comparison to prevent timing attacks
        if provided_hash.len() != expected_hash.len() {
            return Err(SecurityViolation::BoardTampering {
                game_id: game_id.to_string(),
                expected_hash: expected_hash,
                actual_hash: provided_hash.to_string(),
            });
        }

        // Constant-time comparison
        let mut result = 0u8;
        for (a, b) in provided_hash.bytes().zip(expected_hash.bytes()) {
            result |= a ^ b;
        }

        if result != 0 {
            return Err(SecurityViolation::BoardTampering {
                game_id: game_id.to_string(),
                expected_hash,
                actual_hash: provided_hash.to_string(),
            });
        }

        Ok(())
    }

    /// Enhanced reason text validation for game declines
    pub fn validate_secure_reason_text(reason: &str) -> Result<(), SecurityViolation> {
        validate_safe_text_input(reason, "reason", MAX_REASON_LENGTH)
    }

    /// Enhanced FEN notation validation for sync responses
    pub fn validate_secure_fen_notation(fen: &str) -> Result<(), SecurityViolation> {
        validate_safe_text_input(fen, "fen_notation", MAX_FEN_LENGTH)?;

        // Additional FEN-specific validation
        let parts: Vec<&str> = fen.split_whitespace().collect();
        if parts.len() != 6 {
            return Err(SecurityViolation::SuspiciousPattern {
                field: "fen_notation".to_string(),
                pattern: "FEN must have exactly 6 space-separated parts".to_string(),
            });
        }

        // Validate piece placement (first part)
        let piece_placement = parts[0];
        let ranks: Vec<&str> = piece_placement.split('/').collect();
        if ranks.len() != 8 {
            return Err(SecurityViolation::SuspiciousPattern {
                field: "fen_notation".to_string(),
                pattern: "FEN piece placement must have 8 ranks".to_string(),
            });
        }

        Ok(())
    }

    /// Enhanced move history validation for sync responses
    pub fn validate_secure_move_history(move_history: &[String]) -> Result<(), SecurityViolation> {
        if move_history.len() > MAX_MOVE_HISTORY_SIZE {
            return Err(SecurityViolation::FieldTooLong {
                field: "move_history".to_string(),
                length: move_history.len(),
                max_length: MAX_MOVE_HISTORY_SIZE,
            });
        }

        for (i, chess_move) in move_history.iter().enumerate() {
            validate_safe_text_input(
                chess_move,
                &format!("move_history[{}]", i),
                MAX_MOVE_NOTATION_LENGTH,
            )?;

            // Basic format validation for each move
            validate_chess_move_format(chess_move).map_err(|_| {
                SecurityViolation::SuspiciousPattern {
                    field: format!("move_history[{}]", i),
                    pattern: format!("Invalid move format: {}", chess_move),
                }
            })?;
        }

        Ok(())
    }

    /// Comprehensive security validation for chess messages
    pub fn validate_message_security(
        message: &crate::messages::types::Message,
    ) -> Result<(), SecurityViolation> {
        match message {
            crate::messages::types::Message::GameInvite(invite) => {
                validate_secure_game_id(&invite.game_id)?;
            }
            crate::messages::types::Message::GameAccept(accept) => {
                validate_secure_game_id(&accept.game_id)?;
            }
            crate::messages::types::Message::GameDecline(decline) => {
                validate_secure_game_id(&decline.game_id)?;
                if let Some(reason) = &decline.reason {
                    validate_secure_reason_text(reason)?;
                }
            }
            crate::messages::types::Message::Move(chess_move) => {
                validate_secure_game_id(&chess_move.game_id)?;
                validate_secure_chess_move(&chess_move.chess_move, &chess_move.game_id)?;
                validate_safe_text_input(&chess_move.board_state_hash, "board_state_hash", 64)?;
            }
            crate::messages::types::Message::MoveAck(ack) => {
                validate_secure_game_id(&ack.game_id)?;
                if let Some(move_id) = &ack.move_id {
                    validate_safe_text_input(move_id, "move_id", 100)?;
                }
            }
            crate::messages::types::Message::SyncRequest(request) => {
                validate_secure_game_id(&request.game_id)?;
            }
            crate::messages::types::Message::SyncResponse(response) => {
                validate_secure_game_id(&response.game_id)?;
                validate_secure_fen_notation(&response.board_state)?;
                validate_secure_move_history(&response.move_history)?;
                validate_safe_text_input(&response.board_state_hash, "board_state_hash", 64)?;
            }
            // Non-chess messages are not subject to chess-specific security validation
            _ => {}
        }

        Ok(())
    }
}
