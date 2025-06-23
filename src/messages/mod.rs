pub mod chess;
pub mod types;
pub mod wire;

pub use chess::{
    apply_move_from_message,
    // Integration functions
    create_move_message,
    create_sync_response,
    generate_game_id,
    // Enhanced error handling functions
    handle_board_hash_mismatch,
    handle_invalid_game_id,
    handle_malformed_chess_move,
    hash_board_state,
    propagate_error,
    // Security module re-exports
    security,
    validate_chess_move_format,
    validate_chess_move_graceful,
    validate_game_accept,
    validate_game_decline,
    validate_game_id,
    validate_game_id_graceful,
    validate_game_invite,
    validate_move_ack,
    validate_move_message,
    validate_sync_request,
    validate_sync_response,
    verify_board_hash,
    verify_board_hash_graceful,
    // Chess protocol types
    ChessProtocolError,
    ChessProtocolResult,
    GameAccept,
    GameDecline,
    GameInvite,
    Move as ChessMove,
    MoveAck,
    SyncRequest,
    SyncResponse,
    ValidationError,
};
pub use types::{Message, SignedEnvelope};
pub use wire::{
    ConnectionState,
    DosProtectionConfig,

    // Core wire protocol types
    FramedMessage,
    ResilientSession,
    // Graceful degradation types (Step 4.3)
    RetryConfig,
    SessionSummary,

    WireConfig,
    WireProtocolError,
    CLIENT_RETRY_BASE_DELAY,
    // Client retry constants
    CLIENT_RETRY_MAX_ATTEMPTS,
    DEFAULT_READ_TIMEOUT,
    DEFAULT_WRITE_TIMEOUT,

    LENGTH_PREFIX_SIZE,
    MAX_ALLOCATION_SIZE,
    MAX_CONCURRENT_CONNECTIONS,
    // Wire protocol constants
    MAX_MESSAGE_SIZE,
    MAX_REASONABLE_MESSAGE_SIZE,
    MIN_MESSAGE_SIZE,
    SUSPICIOUS_MESSAGE_THRESHOLD,
};
