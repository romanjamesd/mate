use crate::chess::ChessError;
use crate::cli::GameOpsError;
use crate::messages::chess::ChessProtocolError;
use crate::messages::wire::WireProtocolError;
use crate::network::ConnectionError;
use crate::storage::errors::StorageError;
use std::fmt;

/// Unified error type for CLI operations with user-friendly messages
#[derive(Debug)]
pub enum CliError {
    /// Game operations error
    GameOps(GameOpsError),
    /// Chess engine error
    Chess(ChessError),
    /// Database/storage error
    Storage(StorageError),
    /// Network communication error
    Connection(ConnectionError),
    /// Chess protocol error
    Protocol(ChessProtocolError),
    /// Wire protocol error
    Wire(WireProtocolError),
    /// Input validation error
    InvalidInput {
        field: String,
        value: String,
        reason: String,
        suggestion: String,
    },
    /// Configuration error
    Configuration {
        setting: String,
        issue: String,
        suggestion: String,
    },
    /// Network timeout error
    NetworkTimeout {
        operation: String,
        timeout_seconds: u64,
        suggestion: String,
    },
    /// User-friendly error with custom message
    UserError {
        message: String,
        suggestion: Option<String>,
    },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::GameOps(e) => write!(f, "{}", format_game_ops_error(e)),
            CliError::Chess(e) => write!(f, "{}", format_chess_error(e)),
            CliError::Storage(e) => write!(f, "{}", format_storage_error(e)),
            CliError::Connection(e) => write!(f, "{}", format_connection_error(e)),
            CliError::Protocol(e) => write!(f, "{}", format_protocol_error(e)),
            CliError::Wire(e) => write!(f, "{}", format_wire_error(e)),
            CliError::InvalidInput {
                field,
                value,
                reason,
                suggestion,
            } => {
                write!(
                    f,
                    "❌ Invalid {}: '{}'\n   Reason: {}\n   💡 Suggestion: {}",
                    field, value, reason, suggestion
                )
            }
            CliError::Configuration {
                setting,
                issue,
                suggestion,
            } => {
                write!(
                    f,
                    "⚙️  Configuration Error: {}\n   Issue: {}\n   💡 Suggestion: {}",
                    setting, issue, suggestion
                )
            }
            CliError::NetworkTimeout {
                operation,
                timeout_seconds,
                suggestion,
            } => {
                write!(
                    f,
                    "⏱️  Network timeout during {}\n   Timeout: {} seconds\n   💡 Suggestion: {}",
                    operation, timeout_seconds, suggestion
                )
            }
            CliError::UserError {
                message,
                suggestion,
            } => {
                if let Some(suggestion) = suggestion {
                    write!(f, "❌ {}\n   💡 Suggestion: {}", message, suggestion)
                } else {
                    write!(f, "❌ {}", message)
                }
            }
        }
    }
}

impl std::error::Error for CliError {}

// Conversion implementations
impl From<GameOpsError> for CliError {
    fn from(err: GameOpsError) -> Self {
        CliError::GameOps(err)
    }
}

impl From<ChessError> for CliError {
    fn from(err: ChessError) -> Self {
        CliError::Chess(err)
    }
}

impl From<StorageError> for CliError {
    fn from(err: StorageError) -> Self {
        CliError::Storage(err)
    }
}

impl From<ChessProtocolError> for CliError {
    fn from(err: ChessProtocolError) -> Self {
        CliError::Protocol(err)
    }
}

impl From<WireProtocolError> for CliError {
    fn from(err: WireProtocolError) -> Self {
        CliError::Wire(err)
    }
}

impl From<ConnectionError> for CliError {
    fn from(err: ConnectionError) -> Self {
        CliError::Connection(err)
    }
}

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        // For anyhow errors, create a generic user error with the error chain
        let root_cause = err.root_cause();

        // Check if it's a specific error type we can format better
        if root_cause.to_string().contains("Database") {
            return CliError::UserError {
                message: "Database operation failed".to_string(),
                suggestion: Some("Check file permissions and database integrity. Try restarting the application.".to_string()),
            };
        }

        if root_cause.to_string().contains("Connection")
            || root_cause.to_string().contains("network")
        {
            return CliError::UserError {
                message: "Network operation failed".to_string(),
                suggestion: Some(
                    "Check network connectivity and peer availability. Try reconnecting."
                        .to_string(),
                ),
            };
        }

        // For other anyhow errors, create a generic user error
        CliError::UserError {
            message: format!("{err}"),
            suggestion: Some("Check the error details above and try again.".to_string()),
        }
    }
}

/// Format game operations errors with user-friendly messages
fn format_game_ops_error(error: &GameOpsError) -> String {
    match error {
        GameOpsError::NoCurrentGame => {
            "🎮 No active games found.\n   💡 Suggestion: Start a new game with 'mate invite <address>' or use --game-id to specify a game.".to_string()
        }
        GameOpsError::GameNotFound(id) => {
            format!("🎮 Game '{id}' not found.\n   💡 Suggestion: Use 'mate games' to see available games, or check the game ID.")
        }
        GameOpsError::InvalidGameState(msg) => {
            format!("🎮 Invalid game state: {msg}\n   💡 Suggestion: Check the game status with 'mate games' and ensure the game is active.")
        }
        GameOpsError::Database(e) => format_storage_error(e),
        GameOpsError::Chess(e) => format_chess_error(e),
        GameOpsError::Serialization(msg) => {
            format!("🔧 Data format error: {msg}\n   💡 Suggestion: This may be a bug. Please report this issue.")
        }
    }
}

/// Format chess engine errors with user-friendly messages
fn format_chess_error(error: &ChessError) -> String {
    match error {
        ChessError::InvalidMove(msg) => {
            format!("♟️  Invalid move: {msg}\n   💡 Suggestion: Use standard algebraic notation (e.g., 'e4', 'Nf3', 'O-O'). Use 'mate board' to see the current position.")
        }
        ChessError::InvalidPosition(msg) => {
            format!("♟️  Invalid position: {msg}\n   💡 Suggestion: Check the board position with 'mate board' command.")
        }
        ChessError::InvalidFen(msg) => {
            format!(
                "♟️  Invalid board notation: {}\n   💡 Suggestion: Check the FEN string format.",
                msg
            )
        }
        ChessError::InvalidColor(msg) => {
            format!("♟️  Invalid color: {msg}\n   💡 Suggestion: Use 'white' or 'black' for color selection.")
        }
        ChessError::InvalidPieceType(msg) => {
            format!("♟️  Invalid piece: {msg}\n   💡 Suggestion: Use standard piece letters (K, Q, R, B, N, P).")
        }
        ChessError::BoardStateError(msg) => {
            format!("♟️  Board state error: {msg}\n   💡 Suggestion: The game state may be corrupted. Try 'mate board' to see the current position.")
        }
    }
}

/// Format storage errors with user-friendly messages
fn format_storage_error(error: &StorageError) -> String {
    match error {
        StorageError::GameNotFound { id } => {
            format!("🗃️  Game '{id}' not found in database.\n   💡 Suggestion: Use 'mate games' to see available games.")
        }
        StorageError::MessageNotFound { id } => {
            format!("🗃️  Message '{id}' not found.\n   💡 Suggestion: Check the message ID or game history.")
        }
        StorageError::ConnectionFailed(_) => {
            "🗃️  Database connection failed.\n   💡 Suggestion: Check file permissions and disk space. Try restarting the application.".to_string()
        }
        StorageError::DatabaseLocked { operation, timeout_ms } => {
            format!("🗃️  Database is locked during {operation}.\n   Timeout: {timeout_ms}ms\n   💡 Suggestion: Another process may be using the database. Wait a moment and try again.")
        }
        StorageError::InvalidData { field, reason } => {
            format!("🗃️  Invalid data in {field}: {reason}\n   💡 Suggestion: Check the data format and try again.")
        }
        _ => {
            let recovery = error.recovery_suggestion();
            format!("🗃️  Database error: {error}\n   💡 Suggestion: {recovery}")
        }
    }
}

/// Format connection errors with user-friendly messages
fn format_connection_error(error: &ConnectionError) -> String {
    match error {
        ConnectionError::WireProtocol(wire_err) => {
            format!("🌐 Communication protocol error: {wire_err}\n   💡 Suggestion: Check network connection and ensure both players use compatible versions.")
        }
        ConnectionError::HandshakeFailed { reason } => {
            format!("🤝 Connection handshake failed: {reason}\n   💡 Suggestion: Verify the peer address is correct and the peer is online. Check for network connectivity issues.")
        }
        ConnectionError::AuthenticationFailed { peer_id } => {
            format!("🔐 Authentication failed with peer {peer_id}\n   💡 Suggestion: The peer may be using different credentials. Ensure both players have compatible identities.")
        }
        ConnectionError::ConnectionClosed => {
            "🌐 Connection closed unexpectedly\n   💡 Suggestion: The peer may have disconnected. Try reconnecting to continue the game.".to_string()
        }
        ConnectionError::InvalidSignature => {
            "🔒 Message signature verification failed\n   💡 Suggestion: This may indicate a security issue or incompatible software versions. Try reconnecting.".to_string()
        }
        ConnectionError::InvalidTimestamp => {
            "🕐 Message timestamp validation failed\n   💡 Suggestion: Check that your system clock is synchronized. The message may be too old or from the future.".to_string()
        }
        ConnectionError::Io(io_err) => {
            format!("🌐 Network I/O error: {io_err}\n   💡 Suggestion: Check network connection and try again. The peer may be unreachable.")
        }
    }
}

/// Format protocol errors with user-friendly messages
fn format_protocol_error(error: &ChessProtocolError) -> String {
    match error {
        ChessProtocolError::Validation(msg) => {
            format!("🔒 Message validation failed: {msg}\n   💡 Suggestion: This may indicate a communication issue. Try reconnecting.")
        }
        ChessProtocolError::Timeout {
            operation,
            duration_ms,
        } => {
            format!("⏱️  Operation '{operation}' timed out after {duration_ms}ms\n   💡 Suggestion: The peer may be slow to respond. Try again or check network connection.")
        }
        ChessProtocolError::GameStateError { game_id, error } => {
            format!("🎮 Game state error in {game_id}: {error}\n   💡 Suggestion: The game state may be corrupted. Try 'mate board' to see current state.")
        }
        ChessProtocolError::SecurityViolation { game_id, violation } => {
            format!("🔒 Security violation in game {game_id}: {violation}\n   💡 Suggestion: This may indicate a malicious peer. Consider ending the game.")
        }
        _ => {
            format!("🔒 Protocol error: {error}\n   💡 Suggestion: This may be a communication issue. Try reconnecting to the peer.")
        }
    }
}

/// Format wire protocol errors with user-friendly messages
fn format_wire_error(error: &WireProtocolError) -> String {
    match error {
        WireProtocolError::InvalidMessageFormat { .. } => {
            "📡 Invalid message format received\n   💡 Suggestion: This may indicate incompatible versions. Ensure both players are using the same version.".to_string()
        }
        WireProtocolError::MessageTooLarge { size, max_size } => {
            format!("📡 Message too large: {size} bytes (max: {max_size} bytes)\n   💡 Suggestion: The message is too big to send. This may be a bug.")
        }
        WireProtocolError::Io(_) => {
            "📡 Network I/O error\n   💡 Suggestion: Check network connection and try again.".to_string()
        }
        WireProtocolError::ProtocolViolation { description } => {
            format!("📡 Protocol violation: {description}\n   💡 Suggestion: This may indicate incompatible clients. Ensure both players use the same version.")
        }
        _ => {
            format!("📡 Communication error: {error}\n   💡 Suggestion: Check network connection and try reconnecting.")
        }
    }
}

/// Handle specific error scenarios for chess commands
pub fn handle_chess_command_error(error: CliError, command: &str) -> CliError {
    match command {
        "games" => match error {
            CliError::Storage(StorageError::ConnectionFailed(_)) => {
                CliError::UserError {
                    message: "Cannot access game database".to_string(),
                    suggestion: Some("Check file permissions and disk space. The database may be corrupted or locked by another process.".to_string()),
                }
            }
            _ => error,
        },
        "board" => match error {
            CliError::GameOps(GameOpsError::NoCurrentGame) => {
                CliError::UserError {
                    message: "No game specified and no active games found".to_string(),
                    suggestion: Some("Use 'mate games' to see available games, then 'mate board --game-id <id>' to view a specific game.".to_string()),
                }
            }
            _ => error,
        },
        "invite" => match error {
            CliError::Connection(_) => {
                CliError::UserError {
                    message: "Failed to send game invitation".to_string(),
                    suggestion: Some("Check that the peer address is correct and reachable. The peer may be offline or behind a firewall.".to_string()),
                }
            }
            _ => error,
        },
        "accept" => match error {
            CliError::GameOps(GameOpsError::GameNotFound(_)) => {
                CliError::UserError {
                    message: "Game invitation not found".to_string(),
                    suggestion: Some("Use 'mate games' to see pending invitations. The invitation may have expired or been withdrawn.".to_string()),
                }
            }
            _ => error,
        },
        "move" => match error {
            CliError::Chess(ChessError::InvalidMove(_)) => {
                CliError::UserError {
                    message: "Invalid chess move".to_string(),
                    suggestion: Some("Use standard algebraic notation (e.g., 'e4', 'Nf3', 'O-O', 'Qxe7+'). Use 'mate board' to see the current position and legal moves.".to_string()),
                }
            }
            _ => error,
        },
        "history" => match error {
            CliError::GameOps(GameOpsError::GameNotFound(_)) => {
                CliError::UserError {
                    message: "Game not found for history display".to_string(),
                    suggestion: Some("Use 'mate games' to see available games, then 'mate history --game-id <id>' to view move history.".to_string()),
                }
            }
            _ => error,
        },
        _ => error,
    }
}

/// Create a network timeout error with helpful suggestions
pub fn create_network_timeout_error(operation: &str, timeout_seconds: u64) -> CliError {
    let suggestion = match operation {
        "connect" => "The peer may be offline or unreachable. Verify the address and try again.".to_string(),
        "send_invitation" => "The peer may be slow to respond. Try again or check if the peer is online.".to_string(),
        "send_move" => "Move could not be sent. The peer may have disconnected. Check connection and try again.".to_string(),
        "handshake" => "Initial connection handshake failed. The peer may be using incompatible software.".to_string(),
        _ => "Network operation timed out. Check connection and try again.".to_string(),
    };

    CliError::NetworkTimeout {
        operation: operation.to_string(),
        timeout_seconds,
        suggestion,
    }
}

/// Create an input validation error with helpful suggestions
pub fn create_input_validation_error(field: &str, value: &str, reason: &str) -> CliError {
    let suggestion = match field {
        "game_id" => "Game IDs should be in UUID format. Use 'mate games' to see valid game IDs.".to_string(),
        "chess_move" => "Use standard algebraic notation (e.g., 'e4', 'Nf3', 'O-O'). Use 'mate board' to see the current position.".to_string(),
        "color" => "Use 'white' or 'black' to specify player color.".to_string(),
        "address" => "Use format 'host:port' (e.g., '192.168.1.100:8080' or 'example.com:8080').".to_string(),
        _ => "Check the input format and try again.".to_string(),
    };

    CliError::InvalidInput {
        field: field.to_string(),
        value: value.to_string(),
        reason: reason.to_string(),
        suggestion,
    }
}

/// Result type for CLI operations
pub type CliResult<T> = Result<T, CliError>;

/// Display an error with proper formatting and exit codes
pub fn display_error_and_exit(error: CliError, exit_code: i32) -> ! {
    eprintln!("\n{}", error);
    std::process::exit(exit_code);
}

/// Display an error without exiting (for recoverable errors)
pub fn display_error(error: &CliError) {
    eprintln!("\n{}", error);
}

/// Check if an error is recoverable (user can retry)
pub fn is_recoverable_error(error: &CliError) -> bool {
    matches!(
        error,
        CliError::NetworkTimeout { .. }
            | CliError::Connection(_)
            | CliError::InvalidInput { .. }
            | CliError::GameOps(GameOpsError::NoCurrentGame)
            | CliError::GameOps(GameOpsError::GameNotFound(_))
            | CliError::Storage(StorageError::DatabaseLocked { .. })
            | CliError::Chess(ChessError::InvalidMove(_))
    )
}
