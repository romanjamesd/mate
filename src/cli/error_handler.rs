use crate::chess::ChessError;
use crate::cli::validation::ValidationError;
use crate::game::{GameOpsError, MoveProcessingError};
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
    /// A correlated remote refusal of a game acceptance.
    GameAcceptanceRejected { reason: Option<String> },
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
            CliError::GameAcceptanceRejected { reason } => write!(
                f,
                "❌ Game acceptance rejected: {}",
                reason
                    .as_deref()
                    .unwrap_or("the opponent did not provide a reason")
            ),
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

impl From<MoveProcessingError> for CliError {
    fn from(err: MoveProcessingError) -> Self {
        match err {
            MoveProcessingError::GameOps(e) => CliError::GameOps(e),
            MoveProcessingError::Chess(e) => CliError::Chess(e),
            MoveProcessingError::InvalidMove(msg) => {
                CliError::Chess(ChessError::InvalidMove(msg))
            }
            MoveProcessingError::InvalidGameState(msg) => {
                CliError::GameOps(GameOpsError::InvalidGameState(msg))
            }
            MoveProcessingError::TransactionError(msg) => CliError::UserError {
                message: format!("Database transaction failed: {msg}"),
                suggestion: Some(
                    "Try again. If the problem persists, check database integrity.".to_string(),
                ),
            },
            MoveProcessingError::BoardStateError(msg) => {
                CliError::Chess(ChessError::BoardStateError(msg))
            }
            MoveProcessingError::HistoryError(msg) => CliError::UserError {
                message: format!("Move history error: {msg}"),
                suggestion: Some(
                    "The game state may be corrupted. Try 'mate board' to see the current position."
                        .to_string(),
                ),
            },
        }
    }
}

impl From<ValidationError> for CliError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::InvalidMove(msg) => CliError::Chess(ChessError::InvalidMove(msg)),
            ValidationError::InvalidGameId(msg) => {
                create_input_validation_error("game_id", "", &msg)
            }
            ValidationError::InvalidPeerAddress(msg) => {
                create_input_validation_error("address", "", &msg)
            }
            ValidationError::InvalidColor(msg) => create_input_validation_error("color", "", &msg),
            ValidationError::GameNotFound(id) => CliError::GameOps(GameOpsError::GameNotFound(id)),
            ValidationError::AmbiguousGameId(prefix, matches) => CliError::UserError {
                message: format!("Multiple games match '{prefix}': {matches}"),
                suggestion: Some(
                    "Use a more specific game ID or 'mate games' to list all games.".to_string(),
                ),
            },
            ValidationError::NoActiveGames => CliError::GameOps(GameOpsError::NoCurrentGame),
            ValidationError::UserCancelled => CliError::UserError {
                message: "Operation cancelled".to_string(),
                suggestion: None,
            },
            ValidationError::Database(e) => CliError::Storage(e),
            ValidationError::GameOps(e) => CliError::GameOps(e),
            ValidationError::Io(e) => CliError::UserError {
                message: format!("I/O error: {e}"),
                suggestion: Some("Check file permissions and try again.".to_string()),
            },
        }
    }
}

/// Extract typed errors from an anyhow chain before heuristic fallback.
pub fn cli_error_from_anyhow(err: anyhow::Error) -> CliError {
    if let Some(cli) = typed_cli_error_from_source(err.as_ref()) {
        return cli;
    }
    match err.downcast::<StorageError>() {
        Ok(storage) => CliError::from(storage),
        Err(err) => match err.downcast::<ConnectionError>() {
            Ok(conn) => CliError::from(conn),
            Err(err) => CliError::from(err),
        },
    }
}

fn typed_cli_error_from_source(err: &(dyn std::error::Error + 'static)) -> Option<CliError> {
    if let Some(e) = err.downcast_ref::<ChessProtocolError>() {
        return Some(CliError::Protocol(e.clone()));
    }
    if let Some(e) = err.downcast_ref::<ConnectionError>() {
        return Some(connection_error_from_ref(e));
    }
    err.source().and_then(typed_cli_error_from_source)
}

fn connection_error_from_ref(error: &ConnectionError) -> CliError {
    CliError::Connection(match error {
        ConnectionError::WireProtocol(wire_err) => ConnectionError::HandshakeFailed {
            reason: wire_err.to_string(),
        },
        ConnectionError::HandshakeFailed { reason } => ConnectionError::HandshakeFailed {
            reason: reason.clone(),
        },
        ConnectionError::AuthenticationFailed { peer_id } => {
            ConnectionError::AuthenticationFailed {
                peer_id: peer_id.clone(),
            }
        }
        ConnectionError::ConnectionClosed => ConnectionError::ConnectionClosed,
        ConnectionError::InvalidSignature => ConnectionError::InvalidSignature,
        ConnectionError::InvalidTimestamp => ConnectionError::InvalidTimestamp,
        ConnectionError::Io(io_err) => {
            ConnectionError::Io(std::io::Error::new(io_err.kind(), io_err.to_string()))
        }
    })
}

fn is_network_failure(error: &CliError) -> bool {
    match error {
        CliError::Connection(_) => true,
        CliError::NetworkTimeout { .. } => true,
        CliError::UserError { message, .. } => {
            let lower = message.to_lowercase();
            lower.contains("connect")
                || lower.contains("network")
                || lower.contains("timed out")
                || lower.contains("peer")
        }
        _ => false,
    }
}

impl From<anyhow::Error> for CliError {
    fn from(err: anyhow::Error) -> Self {
        // For anyhow errors, create a generic user error with the error chain
        let root_cause = err.root_cause();
        let error_string = err.to_string().to_lowercase();
        let root_cause_string = root_cause.to_string().to_lowercase();

        // Check for specific network errors first
        if error_string.contains("failed to connect")
            || root_cause_string.contains("connection refused")
        {
            return CliError::UserError {
                message: "Failed to connect to server".to_string(),
                suggestion: Some("Check that the address is correct and the peer is online. Verify network connectivity.".to_string()),
            };
        }

        if error_string.contains("address too long")
            || root_cause_string.contains("address too long")
        {
            return CliError::UserError {
                message: "Network address is too long".to_string(),
                suggestion: Some(
                    "Use a shorter address format like 'host:port' (e.g., '127.0.0.1:8080')."
                        .to_string(),
                ),
            };
        }

        if error_string.contains("timeout") || root_cause_string.contains("timeout") {
            return CliError::UserError {
                message: "Network operation timed out".to_string(),
                suggestion: Some("The peer may be slow to respond or unreachable. Check connectivity and try again.".to_string()),
            };
        }

        if error_string.contains("invalid address") || root_cause_string.contains("invalid address")
        {
            return CliError::UserError {
                message: "Invalid network address format".to_string(),
                suggestion: Some(
                    "Use format 'host:port' (e.g., '192.168.1.100:8080' or 'example.com:8080')."
                        .to_string(),
                ),
            };
        }

        // Check for more specific network errors before generic database heuristics
        if error_string.contains("connection")
            || error_string.contains("network")
            || root_cause_string.contains("connection")
        {
            return CliError::UserError {
                message: "Network operation failed".to_string(),
                suggestion: Some(
                    "Check network connectivity and peer availability. Try reconnecting."
                        .to_string(),
                ),
            };
        }

        // Check for database-related errors
        if error_string.contains("database") || root_cause_string.contains("database") {
            return CliError::UserError {
                message: "Database operation failed".to_string(),
                suggestion: Some("Check file permissions and database integrity. Try restarting the application.".to_string()),
            };
        }

        // For other anyhow errors, create a generic user error but avoid exposing raw technical details
        let user_message = if error_string.contains("anyhow") || error_string.contains("error:") {
            "An unexpected error occurred".to_string()
        } else {
            // Use the error message but clean it up
            err.to_string()
        };

        CliError::UserError {
            message: user_message,
            suggestion: Some("Check the error details above and try again. If the problem persists, this may be a bug.".to_string()),
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
            format!("♟️  Invalid move: {msg}\n   💡 Suggestion: Use coordinate notation (e.g., 'e2e4', 'g1f3', 'O-O'). Use 'mate board' to see the current position.")
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
        ConnectionError::WireProtocol(_wire_err) => {
            "🌐 Communication protocol error\n   💡 Suggestion: Check network connection and ensure both players use compatible versions.".to_string()
        }
        ConnectionError::HandshakeFailed { reason: _ } => {
            // Don't expose technical handshake details
            "🤝 Failed to connect to peer\n   💡 Suggestion: Verify the peer address is correct and the peer is online. Check for network connectivity issues.".to_string()
        }
        ConnectionError::AuthenticationFailed { peer_id: _ } => {
            "🔐 Authentication failed with peer\n   💡 Suggestion: The peer may be using different credentials. Ensure both players have compatible identities.".to_string()
        }
        ConnectionError::ConnectionClosed => {
            "🌐 Connection closed unexpectedly\n   💡 Suggestion: The peer may have disconnected. Try reconnecting to continue the game.".to_string()
        }
        ConnectionError::InvalidSignature => {
            "🔒 Message verification failed\n   💡 Suggestion: This may indicate a security issue or incompatible software versions. Try reconnecting.".to_string()
        }
        ConnectionError::InvalidTimestamp => {
            "🕐 Message timing validation failed\n   💡 Suggestion: Check that your system clock is synchronized. Try reconnecting.".to_string()
        }
        ConnectionError::Io(_) => {
            // Don't expose raw I/O error details
            "🌐 Failed to connect to server\n   💡 Suggestion: Check that the address is correct and the peer is reachable. Verify network connectivity.".to_string()
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
        "invite" => {
            if is_network_failure(&error) {
                CliError::UserError {
                    message: "Failed to send game invitation".to_string(),
                    suggestion: Some("Check that the peer address is correct and reachable. The peer may be offline or behind a firewall.".to_string()),
                }
            } else {
                error
            }
        }
        "accept" => match error {
            CliError::GameOps(GameOpsError::GameNotFound(_))
            | CliError::Storage(StorageError::GameNotFound { .. }) => CliError::UserError {
                message: "Game invitation not found".to_string(),
                suggestion: Some("Use 'mate games' to see pending invitations. The invitation may have expired or been withdrawn.".to_string()),
            },
            e if is_network_failure(&e) => CliError::UserError {
                message: "Failed to send game acceptance".to_string(),
                suggestion: Some("Check network connectivity and that the opponent is reachable. Try again.".to_string()),
            },
            _ => error,
        },
        "move" => match error {
            CliError::Chess(ChessError::InvalidMove(_)) => CliError::UserError {
                message: "Invalid chess move".to_string(),
                suggestion: Some("Use coordinate notation (e.g., 'e2e4', 'g1f3', 'O-O'). Use 'mate board' to see the current position.".to_string()),
            },
            e if is_network_failure(&e) => CliError::UserError {
                message: "Could not send move to opponent".to_string(),
                suggestion: Some("Check network connectivity and that the opponent is reachable.".to_string()),
            },
            _ => error,
        },
        "history" => match error {
            CliError::GameOps(GameOpsError::NoCurrentGame) => CliError::UserError {
                message: "No game specified and no active games found".to_string(),
                suggestion: Some("Use 'mate games' to see available games, then 'mate history --game-id <id>' to view move history.".to_string()),
            },
            CliError::GameOps(GameOpsError::GameNotFound(_)) => CliError::UserError {
                message: "Game not found for history display".to_string(),
                suggestion: Some("Use 'mate games' to see available games, then 'mate history --game-id <id>' to view move history.".to_string()),
            },
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
        "chess_move" => "Use coordinate notation (e.g., 'e2e4', 'g1f3', 'O-O'). Use 'mate board' to see the current position.".to_string(),
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
