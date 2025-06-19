pub mod chess;
pub mod types;
pub mod wire;

pub use chess::{
    generate_game_id, hash_board_state, validate_chess_move_format, validate_game_accept,
    validate_game_decline, validate_game_id, validate_game_invite, validate_move_ack,
    validate_move_message, validate_sync_request, validate_sync_response, verify_board_hash,
    GameAccept, GameDecline, GameInvite, Move as ChessMove, MoveAck, SyncRequest, SyncResponse,
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
