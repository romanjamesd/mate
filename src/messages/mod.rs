pub mod types;
pub mod wire;

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
