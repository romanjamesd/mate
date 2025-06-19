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
