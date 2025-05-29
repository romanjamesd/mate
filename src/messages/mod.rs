pub mod types;
pub mod wire;

pub use types::{Message, SignedEnvelope};
pub use wire::{
    // Core wire protocol types
    FramedMessage, 
    WireConfig,
    WireProtocolError,
    DosProtectionConfig,
    
    // Graceful degradation types (Step 4.3)
    RetryConfig,
    ConnectionState,
    ResilientSession,
    SessionSummary,
    
    // Wire protocol constants
    MAX_MESSAGE_SIZE,
    MIN_MESSAGE_SIZE,
    MAX_REASONABLE_MESSAGE_SIZE,
    SUSPICIOUS_MESSAGE_THRESHOLD,
    MAX_CONCURRENT_CONNECTIONS,
    MAX_ALLOCATION_SIZE,
    LENGTH_PREFIX_SIZE,
    DEFAULT_READ_TIMEOUT,
    DEFAULT_WRITE_TIMEOUT,
    
    // Client retry constants
    CLIENT_RETRY_MAX_ATTEMPTS,
    CLIENT_RETRY_BASE_DELAY,
};
