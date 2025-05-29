pub mod types;
pub mod wire;

pub use types::{Message, SignedEnvelope};
pub use wire::{
    // Core wire protocol types
    FramedMessage, 
    WireConfig,
    WireProtocolError,
    DosProtectionConfig,
    
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
};
