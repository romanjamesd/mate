use crate::messages::SignedEnvelope;
use anyhow::{Context, Result};
use std::time::Duration;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::{debug, error, instrument, trace, warn};

// Wire protocol constants
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024; // 16MB
pub const LENGTH_PREFIX_SIZE: usize = 4; // 4 bytes for u32 length prefix
pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);

// Network-specific default configurations for Step 5.1
// These provide appropriate defaults optimized for network operations
pub const NETWORK_DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);
pub const NETWORK_DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
pub const NETWORK_DEFAULT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
pub const NETWORK_DEFAULT_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB for typical network messages
pub const NETWORK_LARGE_MESSAGE_SIZE: usize = 8 * 1024 * 1024; // 8MB for large transfers
pub const NETWORK_SMALL_MESSAGE_SIZE: usize = 64 * 1024; // 64KB for small/control messages

// Connection-specific timeouts
pub const CONNECTION_KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
pub const CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(600); // 10 minutes

// Server-specific configuration
pub const SERVER_ACCEPT_TIMEOUT: Duration = Duration::from_millis(100);
pub const SERVER_MAX_CONCURRENT_CONNECTIONS: usize = 1000;
pub const SERVER_CONNECTION_BACKLOG: usize = 128;

// Client-specific configuration
pub const CLIENT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub const CLIENT_RETRY_MAX_ATTEMPTS: u32 = 3;
pub const CLIENT_RETRY_BASE_DELAY: Duration = Duration::from_millis(1000);

// DoS Protection Constants
pub const MIN_MESSAGE_SIZE: usize = 1; // Minimum message size (1 byte)
pub const MAX_REASONABLE_MESSAGE_SIZE: usize = 1024 * 1024; // 1MB for reasonable messages
pub const SUSPICIOUS_MESSAGE_THRESHOLD: usize = 8 * 1024 * 1024; // 8MB threshold for logging
pub const MAX_CONCURRENT_CONNECTIONS: usize = 1000; // TODO: Implement connection limiting
pub const MAX_ALLOCATION_SIZE: usize = MAX_MESSAGE_SIZE; // Maximum single allocation

/// Configuration for wire protocol operations including timeouts and message size limits
#[derive(Debug, Clone)]
pub struct WireConfig {
    pub max_message_size: usize,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for WireConfig {
    fn default() -> Self {
        Self {
            max_message_size: MAX_MESSAGE_SIZE,
            read_timeout: DEFAULT_READ_TIMEOUT,
            write_timeout: DEFAULT_WRITE_TIMEOUT,
        }
    }
}

impl WireConfig {
    /// Create a new WireConfig with custom parameters
    pub fn new(max_message_size: usize, read_timeout: Duration, write_timeout: Duration) -> Self {
        Self {
            max_message_size,
            read_timeout,
            write_timeout,
        }
    }

    /// Create a WireConfig with custom message size and default timeouts
    pub fn with_max_message_size(max_message_size: usize) -> Self {
        Self {
            max_message_size,
            read_timeout: DEFAULT_READ_TIMEOUT,
            write_timeout: DEFAULT_WRITE_TIMEOUT,
        }
    }

    /// Create a WireConfig with custom timeouts and default message size
    pub fn with_timeouts(read_timeout: Duration, write_timeout: Duration) -> Self {
        Self {
            max_message_size: MAX_MESSAGE_SIZE,
            read_timeout,
            write_timeout,
        }
    }

    /// Create a WireConfig with a single timeout for both read and write operations
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            max_message_size: MAX_MESSAGE_SIZE,
            read_timeout: timeout,
            write_timeout: timeout,
        }
    }

    // Step 5.1: Network-specific configuration presets for appropriate defaults

    /// Create a WireConfig optimized for network operations with standard timeouts
    /// Uses 1MB message size limit and 30-second timeouts
    pub fn for_network() -> Self {
        Self {
            max_message_size: NETWORK_DEFAULT_MESSAGE_SIZE,
            read_timeout: NETWORK_DEFAULT_READ_TIMEOUT,
            write_timeout: NETWORK_DEFAULT_WRITE_TIMEOUT,
        }
    }

    /// Create a WireConfig optimized for small/control messages with faster timeouts
    /// Uses 64KB message size limit and 10-second timeouts for responsive communication
    pub fn for_control_messages() -> Self {
        Self {
            max_message_size: NETWORK_SMALL_MESSAGE_SIZE,
            read_timeout: NETWORK_DEFAULT_HANDSHAKE_TIMEOUT,
            write_timeout: NETWORK_DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }

    /// Create a WireConfig optimized for large file transfers with extended timeouts
    /// Uses 8MB message size limit and extended timeouts for bulk operations
    pub fn for_large_transfers() -> Self {
        Self {
            max_message_size: NETWORK_LARGE_MESSAGE_SIZE,
            read_timeout: Duration::from_secs(120), // 2 minutes for large messages
            write_timeout: Duration::from_secs(120),
        }
    }

    /// Create a WireConfig optimized for server operations
    /// Balanced configuration for handling multiple concurrent connections
    pub fn for_server() -> Self {
        Self {
            max_message_size: NETWORK_DEFAULT_MESSAGE_SIZE,
            read_timeout: NETWORK_DEFAULT_READ_TIMEOUT,
            write_timeout: NETWORK_DEFAULT_WRITE_TIMEOUT,
        }
    }

    /// Create a WireConfig optimized for client operations
    /// Slightly more aggressive timeouts for responsive client behavior
    pub fn for_client() -> Self {
        Self {
            max_message_size: NETWORK_DEFAULT_MESSAGE_SIZE,
            read_timeout: Duration::from_secs(20), // Slightly shorter for clients
            write_timeout: Duration::from_secs(20),
        }
    }

    /// Create a WireConfig for handshake operations with quick timeouts
    /// Optimized for connection establishment with fast feedback
    pub fn for_handshake() -> Self {
        Self {
            max_message_size: NETWORK_SMALL_MESSAGE_SIZE, // Handshakes should be small
            read_timeout: NETWORK_DEFAULT_HANDSHAKE_TIMEOUT,
            write_timeout: NETWORK_DEFAULT_HANDSHAKE_TIMEOUT,
        }
    }

    /// Create a WireConfig for testing with very permissive settings
    /// Allows large messages and long timeouts for development/testing
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            max_message_size: MAX_MESSAGE_SIZE, // Allow maximum size for tests
            read_timeout: Duration::from_secs(60), // Long timeouts for debugging
            write_timeout: Duration::from_secs(60),
        }
    }

    /// Create a WireConfig for production with conservative, secure settings
    /// Optimized for security and resource management in production environments
    pub fn for_production() -> Self {
        Self {
            max_message_size: NETWORK_DEFAULT_MESSAGE_SIZE, // Conservative 1MB limit
            read_timeout: NETWORK_DEFAULT_READ_TIMEOUT,
            write_timeout: NETWORK_DEFAULT_WRITE_TIMEOUT,
        }
    }

    // Chess-specific configurations for Step 8

    /// Create a WireConfig optimized for standard chess messages (invites, moves, acknowledgments)
    /// Uses smaller size limits and faster timeouts for responsive gameplay
    /// Suitable for: GameInvite, GameAccept, GameDecline, Move, MoveAck, SyncRequest
    pub fn for_chess_standard() -> Self {
        Self {
            max_message_size: NETWORK_SMALL_MESSAGE_SIZE, // 64KB - more than enough for standard chess messages
            read_timeout: Duration::from_secs(15),        // Faster timeout for responsive gameplay
            write_timeout: Duration::from_secs(15),
        }
    }

    /// Create a WireConfig optimized for chess synchronization messages
    /// Uses larger size limits and extended timeouts to handle full game state transfers
    /// Suitable for: SyncResponse messages with complete move history and board state
    pub fn for_chess_sync() -> Self {
        Self {
            max_message_size: NETWORK_LARGE_MESSAGE_SIZE, // 8MB - handles games with extensive move history
            read_timeout: Duration::from_secs(60),        // Extended timeout for large sync data
            write_timeout: Duration::from_secs(60),
        }
    }

    /// Create a WireConfig optimized for chess tournaments or bulk operations
    /// Uses maximum size limits and very extended timeouts for handling multiple games
    /// Suitable for: Bulk game synchronization, tournament data, multiple concurrent games
    pub fn for_chess_bulk() -> Self {
        Self {
            max_message_size: MAX_MESSAGE_SIZE, // Full 16MB - for tournament or bulk operations
            read_timeout: Duration::from_secs(120), // Very extended timeout for bulk operations
            write_timeout: Duration::from_secs(120),
        }
    }

    /// Create a WireConfig optimized for real-time chess gameplay
    /// Uses balanced settings optimized for low-latency interactive play
    /// Suitable for: Real-time games where responsiveness is critical
    pub fn for_chess_realtime() -> Self {
        Self {
            max_message_size: NETWORK_DEFAULT_MESSAGE_SIZE, // 1MB - balanced for real-time use
            read_timeout: Duration::from_secs(10), // Very fast timeout for real-time responsiveness
            write_timeout: Duration::from_secs(10),
        }
    }
}

/// DoS Protection configuration for future rate limiting implementation
///
/// TODO: Future implementation should include:
/// - Per-connection message rate limiting (messages per second)
/// - Per-IP address rate limiting
/// - Bandwidth limiting (bytes per second)
/// - Connection attempt rate limiting
/// - Backpressure mechanisms for high load
///
/// Example future configuration:
/// ```rust
/// pub struct RateLimitConfig {
///     pub max_messages_per_second: u32,
///     pub max_bytes_per_second: u64,
///     pub max_connections_per_ip: u32,
///     pub connection_rate_limit: u32,
///     pub burst_allowance: u32,
/// }
/// ```
#[derive(Debug, Clone)]
pub struct DosProtectionConfig {
    pub max_message_size: usize,
    pub min_message_size: usize,
    pub suspicious_threshold: usize,
    pub max_allocation_size: usize,
    // Rate limiting fields will be added in future implementation
}

impl Default for DosProtectionConfig {
    fn default() -> Self {
        Self {
            max_message_size: MAX_MESSAGE_SIZE,
            min_message_size: MIN_MESSAGE_SIZE,
            suspicious_threshold: SUSPICIOUS_MESSAGE_THRESHOLD,
            max_allocation_size: MAX_ALLOCATION_SIZE,
        }
    }
}

/// Custom error types for wire protocol operations
#[derive(Error, Debug)]
pub enum WireProtocolError {
    #[error("Message too large: {size} bytes exceeds maximum of {max_size} bytes")]
    MessageTooLarge { size: usize, max_size: usize },

    #[error("Invalid length prefix: {length} (valid range: {min}-{max})")]
    InvalidLength { length: u32, min: u32, max: u32 },

    #[error("Suspicious message size: {size} bytes exceeds threshold of {threshold} bytes")]
    SuspiciousMessageSize { size: usize, threshold: usize },

    #[error("Message too small: {size} bytes is below minimum of {min_size} bytes")]
    MessageTooSmall { size: usize, min_size: usize },

    #[error(
        "Memory allocation denied: {size} bytes exceeds safe allocation limit of {limit} bytes"
    )]
    AllocationDenied { size: usize, limit: usize },

    #[error("Read operation timed out after {timeout:?}")]
    ReadTimeout { timeout: Duration },

    #[error("Write operation timed out after {timeout:?}")]
    WriteTimeout { timeout: Duration },

    #[error("Operation timed out after {timeout:?}: {operation}")]
    OperationTimeout {
        timeout: Duration,
        operation: String,
    },

    #[error("Corrupted data: {reason}")]
    CorruptedData { reason: String },

    #[error("Unexpected end of file while reading {operation}")]
    UnexpectedEof { operation: String },

    #[error("Connection closed unexpectedly during {operation}")]
    ConnectionClosed { operation: String },

    #[error("Protocol violation: {description}")]
    ProtocolViolation { description: String },

    #[error("Buffer overflow: attempted to write {attempted} bytes to buffer of size {capacity}")]
    BufferOverflow { attempted: usize, capacity: usize },

    #[error("Invalid message format: {details}")]
    InvalidMessageFormat { details: String },

    #[error("Length mismatch: expected {expected} bytes, got {actual} bytes")]
    LengthMismatch { expected: usize, actual: usize },

    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Timeout error: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),
}

impl WireProtocolError {
    /// Create a new MessageTooLarge error with context
    pub fn message_too_large(size: usize, max_size: usize) -> Self {
        Self::MessageTooLarge { size, max_size }
    }

    /// Create a new InvalidLength error with valid range context
    pub fn invalid_length(length: u32) -> Self {
        Self::InvalidLength {
            length,
            min: 1,
            max: MAX_MESSAGE_SIZE as u32,
        }
    }

    /// Create a new InvalidLength error with custom range
    pub fn invalid_length_with_range(length: u32, min: u32, max: u32) -> Self {
        Self::InvalidLength { length, min, max }
    }

    /// Create a new CorruptedData error with reason
    pub fn corrupted_data<S: Into<String>>(reason: S) -> Self {
        Self::CorruptedData {
            reason: reason.into(),
        }
    }

    /// Create a new UnexpectedEof error with operation context
    pub fn unexpected_eof<S: Into<String>>(operation: S) -> Self {
        Self::UnexpectedEof {
            operation: operation.into(),
        }
    }

    /// Create a new ConnectionClosed error with operation context
    pub fn connection_closed<S: Into<String>>(operation: S) -> Self {
        Self::ConnectionClosed {
            operation: operation.into(),
        }
    }

    /// Create a new ProtocolViolation error with description
    pub fn protocol_violation<S: Into<String>>(description: S) -> Self {
        Self::ProtocolViolation {
            description: description.into(),
        }
    }

    /// Create a new BufferOverflow error with capacity information
    pub fn buffer_overflow(attempted: usize, capacity: usize) -> Self {
        Self::BufferOverflow {
            attempted,
            capacity,
        }
    }

    /// Create a new InvalidMessageFormat error with details
    pub fn invalid_message_format<S: Into<String>>(details: S) -> Self {
        Self::InvalidMessageFormat {
            details: details.into(),
        }
    }

    /// Create a new LengthMismatch error
    pub fn length_mismatch(expected: usize, actual: usize) -> Self {
        Self::LengthMismatch { expected, actual }
    }

    /// Create a new OperationTimeout error with context
    pub fn operation_timeout<S: Into<String>>(timeout: Duration, operation: S) -> Self {
        Self::OperationTimeout {
            timeout,
            operation: operation.into(),
        }
    }

    /// Check if this error is recoverable (can be retried)
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Timeout errors are generally recoverable
            WireProtocolError::ReadTimeout { .. }
            | WireProtocolError::WriteTimeout { .. }
            | WireProtocolError::OperationTimeout { .. }
            | WireProtocolError::Timeout(_) => true,

            // Some IO errors are recoverable (e.g., interrupted operations)
            WireProtocolError::Io(io_err) => matches!(
                io_err.kind(),
                std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::TimedOut
            ),

            // Connection closed might be recoverable if we can reconnect
            WireProtocolError::ConnectionClosed { .. } => true,

            // Protocol violations and data corruption are not recoverable
            WireProtocolError::ProtocolViolation { .. }
            | WireProtocolError::CorruptedData { .. }
            | WireProtocolError::InvalidMessageFormat { .. }
            | WireProtocolError::Serialization(_) => false,

            // Size-related errors are configuration issues, not recoverable
            WireProtocolError::MessageTooLarge { .. }
            | WireProtocolError::MessageTooSmall { .. }
            | WireProtocolError::AllocationDenied { .. }
            | WireProtocolError::InvalidLength { .. }
            | WireProtocolError::BufferOverflow { .. }
            | WireProtocolError::LengthMismatch { .. } => false,

            // Suspicious messages may be recoverable if it's a false positive
            WireProtocolError::SuspiciousMessageSize { .. } => true,

            // EOF is generally not recoverable
            WireProtocolError::UnexpectedEof { .. } => false,
        }
    }

    /// Check if this error indicates a security concern
    pub fn is_security_related(&self) -> bool {
        matches!(
            self,
            WireProtocolError::MessageTooLarge { .. }
                | WireProtocolError::SuspiciousMessageSize { .. }
                | WireProtocolError::AllocationDenied { .. }
                | WireProtocolError::ProtocolViolation { .. }
                | WireProtocolError::CorruptedData { .. }
                | WireProtocolError::InvalidMessageFormat { .. }
                | WireProtocolError::BufferOverflow { .. }
        )
    }

    /// Get a user-friendly error category
    pub fn category(&self) -> &'static str {
        match self {
            WireProtocolError::ReadTimeout { .. }
            | WireProtocolError::WriteTimeout { .. }
            | WireProtocolError::OperationTimeout { .. }
            | WireProtocolError::Timeout(_) => "Timeout",

            WireProtocolError::MessageTooLarge { .. }
            | WireProtocolError::MessageTooSmall { .. }
            | WireProtocolError::InvalidLength { .. }
            | WireProtocolError::SuspiciousMessageSize { .. } => "Message Size",

            WireProtocolError::AllocationDenied { .. }
            | WireProtocolError::BufferOverflow { .. } => "Memory",

            WireProtocolError::CorruptedData { .. }
            | WireProtocolError::InvalidMessageFormat { .. }
            | WireProtocolError::LengthMismatch { .. }
            | WireProtocolError::Serialization(_) => "Data Format",

            WireProtocolError::UnexpectedEof { .. }
            | WireProtocolError::ConnectionClosed { .. }
            | WireProtocolError::Io(_) => "Connection",

            WireProtocolError::ProtocolViolation { .. } => "Protocol",
        }
    }
}

// Implement conversion from anyhow::Error for compatibility
impl From<anyhow::Error> for WireProtocolError {
    fn from(err: anyhow::Error) -> Self {
        // Try to downcast to known error types first
        if let Some(io_err) = err.downcast_ref::<std::io::Error>() {
            return WireProtocolError::Io(std::io::Error::new(io_err.kind(), format!("{}", err)));
        }

        if let Some(_bincode_err) = err.downcast_ref::<bincode::Error>() {
            // Create a new serialization error with the context from anyhow
            return WireProtocolError::invalid_message_format(format!(
                "Serialization failed: {}",
                err
            ));
        }

        if let Some(_timeout_err) = err.downcast_ref::<tokio::time::error::Elapsed>() {
            // Create a generic timeout error since we don't have the specific timeout duration
            return WireProtocolError::ProtocolViolation {
                description: format!("Operation timed out: {}", err),
            };
        }

        // For other anyhow errors, create a generic protocol violation
        WireProtocolError::ProtocolViolation {
            description: format!("Unexpected error: {}", err),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FramedMessage {
    wire_config: WireConfig,
    dos_config: DosProtectionConfig,
}

impl FramedMessage {
    /// Create a new FramedMessage with custom wire protocol configuration
    pub fn new(wire_config: WireConfig) -> Self {
        let dos_config = DosProtectionConfig {
            max_message_size: wire_config.max_message_size,
            ..DosProtectionConfig::default()
        };

        Self {
            wire_config,
            dos_config,
        }
    }

    /// Create a new FramedMessage with both wire and DoS protection configurations
    pub fn with_configs(wire_config: WireConfig, dos_config: DosProtectionConfig) -> Self {
        Self {
            wire_config,
            dos_config,
        }
    }

    /// Create a new FramedMessage with custom DoS protection configuration (backward compatibility)
    pub fn with_config(config: DosProtectionConfig) -> Self {
        let wire_config = WireConfig {
            max_message_size: config.max_message_size,
            ..WireConfig::default()
        };

        Self {
            wire_config,
            dos_config: config,
        }
    }

    /// Create a new FramedMessage with custom timeouts and default other settings
    pub fn with_timeouts(read_timeout: Duration, write_timeout: Duration) -> Self {
        let wire_config = WireConfig::with_timeouts(read_timeout, write_timeout);
        let dos_config = DosProtectionConfig::default();

        Self {
            wire_config,
            dos_config,
        }
    }

    /// Create a new FramedMessage with a single timeout for both operations
    pub fn with_timeout(timeout: Duration) -> Self {
        let wire_config = WireConfig::with_timeout(timeout);
        let dos_config = DosProtectionConfig::default();

        Self {
            wire_config,
            dos_config,
        }
    }

    /// Create a new FramedMessage with custom max message size and default other settings
    pub fn with_max_message_size(max_message_size: usize) -> Self {
        let wire_config = WireConfig::with_max_message_size(max_message_size);
        let dos_config = DosProtectionConfig {
            max_message_size,
            ..DosProtectionConfig::default()
        };

        Self {
            wire_config,
            dos_config,
        }
    }

    /// Get the current wire protocol configuration
    pub fn wire_config(&self) -> &WireConfig {
        &self.wire_config
    }

    /// Get the current DoS protection configuration (backward compatibility)
    pub fn config(&self) -> &DosProtectionConfig {
        &self.dos_config
    }

    /// Get the current DoS protection configuration
    pub fn dos_config(&self) -> &DosProtectionConfig {
        &self.dos_config
    }

    /// Get the read timeout from wire configuration
    pub fn read_timeout(&self) -> Duration {
        self.wire_config.read_timeout
    }

    /// Get the write timeout from wire configuration
    pub fn write_timeout(&self) -> Duration {
        self.wire_config.write_timeout
    }

    /// Test helper: Safe memory allocation with DoS protection (exposed for testing)
    pub fn test_safe_allocate(&self, size: usize) -> Result<Vec<u8>, WireProtocolError> {
        self.safe_allocate(size)
    }

    /// Test helper: Enhanced length validation (exposed for testing)
    pub fn test_validate_length(&self, length: u32) -> Result<usize, WireProtocolError> {
        self.validate_length(length)
    }

    /// Validate message size against DoS protection limits with comprehensive logging
    ///
    /// This method performs multi-layered validation of message sizes to prevent
    /// denial-of-service attacks through oversized messages or memory exhaustion.
    /// All validation events are logged with structured data for security monitoring.
    #[instrument(level = "trace", skip(self), fields(
        size,
        min_size = self.dos_config.min_message_size,
        max_size = self.dos_config.max_message_size,
        suspicious_threshold = self.dos_config.suspicious_threshold,
        validation_result = tracing::field::Empty
    ))]
    fn validate_message_size(&self, size: usize) -> Result<(), WireProtocolError> {
        tracing::Span::current().record("size", size);

        // Check minimum size to prevent zero-length or malformed messages
        if size < self.dos_config.min_message_size {
            tracing::Span::current().record("validation_result", "rejected_too_small");
            warn!(
                size = size,
                min_size = self.dos_config.min_message_size,
                "Message size is below minimum threshold"
            );
            return Err(WireProtocolError::MessageTooSmall {
                size,
                min_size: self.dos_config.min_message_size,
            });
        }

        // Check maximum size to prevent memory exhaustion
        if size > self.dos_config.max_message_size {
            tracing::Span::current().record("validation_result", "rejected_too_large");
            error!(
                size = size,
                max_size = self.dos_config.max_message_size,
                security_event = "dos_protection_triggered",
                "Message size exceeds maximum allowed size"
            );
            return Err(WireProtocolError::MessageTooLarge {
                size,
                max_size: self.dos_config.max_message_size,
            });
        }

        // Log suspicious but allowed message sizes for monitoring
        if size > self.dos_config.suspicious_threshold {
            tracing::Span::current().record("validation_result", "accepted_suspicious");
            warn!(
                size = size,
                threshold = self.dos_config.suspicious_threshold,
                max_size = self.dos_config.max_message_size,
                security_event = "suspicious_message_size",
                size_ratio = (size as f64) / (self.dos_config.max_message_size as f64),
                "Message size exceeds suspicious threshold but is still allowed"
            );
        } else {
            tracing::Span::current().record("validation_result", "accepted_normal");
        }

        debug!(
            size = size,
            utilization_pct =
                ((size as f64) / (self.dos_config.max_message_size as f64) * 100.0) as u32,
            "Message size validation passed"
        );
        Ok(())
    }

    /// Safe memory allocation with DoS protection
    #[instrument(level = "trace", skip(self), fields(size, max_allocation = self.dos_config.max_allocation_size))]
    fn safe_allocate(&self, size: usize) -> Result<Vec<u8>, WireProtocolError> {
        tracing::Span::current().record("size", size);

        // Validate allocation size against configured limits
        if size > self.dos_config.max_allocation_size {
            error!(
                size = size,
                limit = self.dos_config.max_allocation_size,
                "Allocation request exceeds safe allocation limit"
            );
            return Err(WireProtocolError::AllocationDenied {
                size,
                limit: self.dos_config.max_allocation_size,
            });
        }

        // Additional check: ensure we don't allocate more than available memory
        // This is a conservative check - in production, you might want to check actual available memory
        if size > isize::MAX as usize / 2 {
            error!(size = size, "Allocation request exceeds safe memory bounds");
            return Err(WireProtocolError::AllocationDenied {
                size,
                limit: isize::MAX as usize / 2,
            });
        }

        trace!("Allocating {} bytes for message buffer", size);

        // Attempt allocation with error handling
        // Initialize the vector to the required size
        let vec = vec![0; size];
        debug!("Successfully allocated {} byte buffer", size);
        Ok(vec)
    }

    /// Serialize a SignedEnvelope to bytes with enhanced DoS protection
    #[instrument(level = "trace", skip(self, envelope), fields(envelope_size))]
    fn serialize_envelope(&self, envelope: &SignedEnvelope) -> Result<Vec<u8>, WireProtocolError> {
        trace!("Starting envelope serialization with DoS protection");

        // Serialize the envelope using bincode
        let serialized = bincode::serialize(envelope).map_err(|e| {
            error!(error = %e, "Failed to serialize SignedEnvelope with bincode");
            WireProtocolError::Serialization(e)
        })?;

        tracing::Span::current().record("envelope_size", serialized.len());

        // Validate serialized message size against DoS protection
        self.validate_message_size(serialized.len())?;

        debug!("Serialized envelope to {} bytes", serialized.len());
        trace!("Envelope serialization completed successfully with DoS validation");
        Ok(serialized)
    }

    /// Deserialize bytes back to SignedEnvelope with enhanced DoS protection
    #[instrument(level = "trace", skip(self, data), fields(data_size = data.len()))]
    fn deserialize_envelope(&self, data: &[u8]) -> Result<SignedEnvelope, WireProtocolError> {
        trace!("Starting envelope deserialization with DoS protection");

        // Validate data size against DoS protection
        self.validate_message_size(data.len())?;

        // Deserialize the data using bincode
        let envelope = bincode::deserialize(data).map_err(|e| {
            error!(
                error = %e,
                data_size = data.len(),
                "Failed to deserialize data to SignedEnvelope"
            );
            WireProtocolError::CorruptedData {
                reason: format!("Failed to deserialize SignedEnvelope: {}", e),
            }
        })?;

        debug!(
            "Successfully deserialized envelope from {} bytes",
            data.len()
        );
        trace!("Envelope deserialization completed successfully with DoS validation");
        Ok(envelope)
    }

    /// Enhanced length validation with comprehensive DoS protection
    #[instrument(level = "trace", skip(self))]
    fn validate_length(&self, length: u32) -> Result<usize, WireProtocolError> {
        trace!(
            "Validating message length: {} with enhanced DoS protection",
            length
        );

        let length_usize = length as usize;

        // Validate against configured message size limits
        self.validate_message_size(length_usize)?;

        // Additional sanity checks for length prefix

        // Check for unreasonably large lengths that could indicate corruption
        // Even if within max size, extremely large lengths might be suspicious
        if length > (u32::MAX / 2) {
            warn!(
                length = length,
                "Length prefix is extremely large, possible corruption or attack"
            );
            return Err(WireProtocolError::InvalidLength {
                length,
                min: 1,
                max: MAX_MESSAGE_SIZE as u32,
            });
        }

        // Check for zero length (should not happen for valid messages)
        if length == 0 {
            warn!("Received zero-length message prefix");
            return Err(WireProtocolError::InvalidLength {
                length,
                min: 1,
                max: MAX_MESSAGE_SIZE as u32,
            });
        }

        debug!(
            "Enhanced length validation passed for {} bytes",
            length_usize
        );
        Ok(length_usize)
    }

    /// Robust write operation with recovery logic for partial writes
    #[instrument(level = "debug", skip(writer, data), fields(data_size = data.len()))]
    async fn write_all_with_recovery(
        writer: &mut (impl AsyncWrite + Unpin),
        data: &[u8],
    ) -> Result<()> {
        let mut total_written = 0;
        let data_len = data.len();

        debug!("Starting write operation for {} bytes", data_len);

        while total_written < data_len {
            let remaining = &data[total_written..];

            match writer.write(remaining).await {
                Ok(0) => {
                    error!(
                        total_written = total_written,
                        remaining = remaining.len(),
                        "Write operation returned 0 bytes, indicating writer is closed"
                    );
                    return Err(anyhow::anyhow!(
                        "Write failed: writer closed after writing {} of {} bytes",
                        total_written,
                        data_len
                    ));
                }
                Ok(written) => {
                    total_written += written;
                    trace!(
                        written = written,
                        total_written = total_written,
                        remaining = data_len - total_written,
                        "Partial write completed"
                    );

                    if written < remaining.len() {
                        debug!(
                            "Partial write: wrote {} of {} remaining bytes, continuing",
                            written,
                            remaining.len()
                        );
                    }
                }
                Err(e) => {
                    error!(
                        error = %e,
                        total_written = total_written,
                        data_size = data_len,
                        "Write operation failed"
                    );
                    return Err(anyhow::anyhow!(
                        "Write failed after writing {} of {} bytes: {}",
                        total_written,
                        data_len,
                        e
                    ));
                }
            }
        }

        debug!(
            "Write operation completed successfully, {} bytes written",
            total_written
        );
        Ok(())
    }

    /// Robust read operation with recovery logic for partial reads
    #[instrument(level = "debug", skip(reader, buffer), fields(buffer_size = buffer.len()))]
    async fn read_exact_with_recovery(
        reader: &mut (impl AsyncRead + Unpin),
        buffer: &mut [u8],
    ) -> Result<()> {
        let mut total_read = 0;
        let buffer_len = buffer.len();

        debug!("Starting read operation for {} bytes", buffer_len);

        while total_read < buffer_len {
            let remaining = &mut buffer[total_read..];

            match reader.read(remaining).await {
                Ok(0) => {
                    error!(
                        total_read = total_read,
                        expected = buffer_len,
                        "Unexpected EOF: read 0 bytes when {} bytes remaining",
                        remaining.len()
                    );
                    return Err(anyhow::anyhow!(
                        "Unexpected EOF while reading: got {} of {} expected bytes",
                        total_read,
                        buffer_len
                    ));
                }
                Ok(read) => {
                    total_read += read;
                    trace!(
                        read = read,
                        total_read = total_read,
                        remaining = buffer_len - total_read,
                        "Partial read completed"
                    );

                    if read < remaining.len() {
                        debug!(
                            "Partial read: got {} of {} remaining bytes, continuing",
                            read,
                            remaining.len()
                        );
                    }
                }
                Err(e) => {
                    error!(
                        error = %e,
                        total_read = total_read,
                        expected = buffer_len,
                        "Read operation failed"
                    );
                    return Err(anyhow::anyhow!(
                        "Read failed after reading {} of {} bytes: {}",
                        total_read,
                        buffer_len,
                        e
                    ));
                }
            }
        }

        debug!(
            "Read operation completed successfully, {} bytes read",
            total_read
        );
        Ok(())
    }

    /// Write a message with enhanced DoS protection and structured logging
    ///
    /// This method serializes a `SignedEnvelope` and writes it to the provided async writer
    /// using the length-prefixed wire protocol format. The operation includes comprehensive
    /// DoS protection with configurable size limits and detailed logging for monitoring.
    ///
    /// # Arguments
    /// * `writer` - The async writer to write the message to
    /// * `envelope` - The SignedEnvelope to serialize and send
    ///
    /// # Returns
    /// * `Ok(())` - Message was successfully written
    /// * `Err(anyhow::Error)` - Serialization, validation, or IO error occurred
    ///
    /// # Wire Protocol Format
    /// ```text
    /// [4 bytes: message length (big-endian u32)][message bytes: serialized SignedEnvelope]
    /// ```
    #[instrument(level = "debug", skip(self, writer, envelope), fields(
        message_size,
        write_timeout_secs = self.wire_config.write_timeout.as_secs(),
        max_message_size = self.wire_config.max_message_size
    ))]
    pub async fn write_message(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();
        debug!("Starting message write operation with DoS protection");

        // Serialize the envelope to bytes with enhanced validation
        let message_bytes = self
            .serialize_envelope(envelope)
            .with_context(|| "Failed to serialize envelope for writing")?;

        // Record message size for metrics
        let message_length = message_bytes.len() as u32;
        tracing::Span::current().record("message_size", message_length);

        debug!(
            "Prepared message: {} bytes with length prefix",
            message_length
        );

        // Create the 4-byte length prefix (big-endian)
        let length_prefix = message_length.to_be_bytes();

        // Write the length prefix first with recovery logic
        Self::write_all_with_recovery(writer, &length_prefix)
            .await
            .with_context(|| {
                format!("Failed to write 4-byte length prefix ({})", message_length)
            })?;

        // Write the message bytes with recovery logic
        Self::write_all_with_recovery(writer, &message_bytes)
            .await
            .with_context(|| {
                format!(
                    "Failed to write message data ({} bytes)",
                    message_bytes.len()
                )
            })?;

        // Ensure all data is flushed to the underlying writer
        writer
            .flush()
            .await
            .with_context(|| "Failed to flush writer after message write")?;

        let elapsed = start_time.elapsed();
        debug!(
            message_size = message_length,
            duration_ms = elapsed.as_millis(),
            "Message write operation completed successfully with DoS protection"
        );
        Ok(())
    }

    #[instrument(level = "debug", skip(self, reader))]
    pub async fn read_message(
        &self,
        reader: &mut (impl AsyncRead + Unpin),
    ) -> Result<SignedEnvelope> {
        debug!("Starting message read operation with DoS protection");

        // Read the 4-byte length prefix with recovery logic
        let mut length_buffer = [0u8; LENGTH_PREFIX_SIZE];
        Self::read_exact_with_recovery(reader, &mut length_buffer)
            .await
            .with_context(|| "Failed to read 4-byte length prefix")?;

        // Parse the length as big-endian u32
        let message_length = u32::from_be_bytes(length_buffer);
        debug!("Read length prefix: {} bytes expected", message_length);

        // Validate the length with enhanced DoS protection
        let validated_length = self
            .validate_length(message_length)
            .with_context(|| format!("Invalid message length received: {}", message_length))?;

        // Safe allocation with DoS protection
        let mut message_buffer = self
            .safe_allocate(validated_length)
            .with_context(|| format!("Failed to allocate {} byte buffer", validated_length))?;
        debug!(
            "Safely allocated buffer for {} byte message",
            validated_length
        );

        // Read exactly the specified number of bytes for the message with recovery logic
        Self::read_exact_with_recovery(reader, &mut message_buffer)
            .await
            .with_context(|| {
                format!(
                    "Failed to read message data: expected {} bytes",
                    validated_length
                )
            })?;

        // Deserialize the message bytes back to SignedEnvelope with enhanced validation
        let envelope = self
            .deserialize_envelope(&message_buffer)
            .with_context(|| format!("Failed to deserialize {} byte message", validated_length))?;

        debug!("Message read operation completed successfully with DoS protection");
        Ok(envelope)
    }

    /// Read a message with a timeout using enhanced DoS protection
    ///
    /// This method wraps the standard `read_message` operation with a timeout to prevent
    /// hanging operations. If the timeout expires before the message is fully read,
    /// a `WireProtocolError::ReadTimeout` error is returned.
    ///
    /// # Arguments
    /// * `reader` - The async reader to read from
    /// * `timeout_duration` - Maximum time to wait for the read operation
    ///
    /// # Returns
    /// * `Ok(SignedEnvelope)` - Successfully read and deserialized message
    /// * `Err(anyhow::Error)` - Read timeout, IO error, or deserialization error
    ///

    #[instrument(level = "debug", skip(self, reader), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn read_message_with_timeout(
        &self,
        reader: &mut (impl AsyncRead + Unpin),
        timeout_duration: Duration,
    ) -> Result<SignedEnvelope> {
        debug!(
            "Starting timed message read operation with {:?} timeout and DoS protection",
            timeout_duration
        );

        let start_time = std::time::Instant::now();

        match tokio::time::timeout(timeout_duration, self.read_message(reader)).await {
            Ok(result) => {
                let elapsed = start_time.elapsed();
                debug!(
                    "Timed read operation with DoS protection completed in {:?}",
                    elapsed
                );
                result
            }
            Err(elapsed_err) => {
                let elapsed = start_time.elapsed();
                error!(
                    timeout = ?timeout_duration,
                    elapsed = ?elapsed,
                    "Read operation timed out"
                );
                // Use the new From trait for automatic conversion
                Err(WireProtocolError::from(elapsed_err).into())
            }
        }
    }

    /// Write a message with a timeout using enhanced DoS protection
    ///
    /// This method wraps the standard `write_message` operation with a timeout to prevent
    /// hanging operations. If the timeout expires before the message is fully written,
    /// a `WireProtocolError::WriteTimeout` error is returned.
    ///
    /// # Arguments
    /// * `writer` - The async writer to write to
    /// * `envelope` - The SignedEnvelope to serialize and send
    /// * `timeout_duration` - Maximum time to wait for the write operation
    ///
    /// # Returns
    /// * `Ok(())` - Successfully serialized and written message
    /// * `Err(anyhow::Error)` - Write timeout, IO error, or serialization error
    ///

    #[instrument(level = "debug", skip(self, writer, envelope), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn write_message_with_timeout(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
        timeout_duration: Duration,
    ) -> Result<()> {
        debug!(
            "Starting timed message write operation with {:?} timeout and DoS protection",
            timeout_duration
        );

        let start_time = std::time::Instant::now();

        match tokio::time::timeout(timeout_duration, self.write_message(writer, envelope)).await {
            Ok(result) => {
                let elapsed = start_time.elapsed();
                debug!(
                    "Timed write operation with DoS protection completed in {:?}",
                    elapsed
                );
                result
            }
            Err(elapsed_err) => {
                let elapsed = start_time.elapsed();
                error!(
                    timeout = ?timeout_duration,
                    elapsed = ?elapsed,
                    "Write operation timed out"
                );
                // Use the new From trait for automatic conversion
                Err(WireProtocolError::from(elapsed_err).into())
            }
        }
    }

    /// Read a message using the configured default timeout and DoS protection
    ///
    /// Convenience method that uses the read timeout from the wire configuration.
    #[instrument(level = "debug", skip(self, reader), fields(timeout_secs = self.wire_config.read_timeout.as_secs()))]
    pub async fn read_message_with_default_timeout(
        &self,
        reader: &mut (impl AsyncRead + Unpin),
    ) -> Result<SignedEnvelope> {
        let timeout = self.wire_config.read_timeout;
        debug!(
            "Starting read operation with configured default timeout ({:?}) and DoS protection",
            timeout
        );
        self.read_message_with_timeout(reader, timeout).await
    }

    /// Write a message using the configured default timeout and DoS protection
    ///
    /// Convenience method that uses the write timeout from the wire configuration.
    #[instrument(level = "debug", skip(self, writer, envelope), fields(timeout_secs = self.wire_config.write_timeout.as_secs()))]
    pub async fn write_message_with_default_timeout(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
    ) -> Result<()> {
        let timeout = self.wire_config.write_timeout;
        debug!(
            "Starting write operation with configured default timeout ({:?}) and DoS protection",
            timeout
        );
        self.write_message_with_timeout(writer, envelope, timeout)
            .await
    }

    // Static convenience methods for backward compatibility and ease of use

    /// Static convenience method for writing a message with default DoS protection
    ///
    /// This method creates a default FramedMessage instance and uses it to write the message.
    /// For performance-critical applications or custom DoS protection settings,
    /// prefer creating a FramedMessage instance and reusing it.
    #[instrument(level = "debug", skip(writer, envelope))]
    pub async fn write_message_static(
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
    ) -> Result<()> {
        let framed = FramedMessage::default();
        framed.write_message(writer, envelope).await
    }

    /// Static convenience method for reading a message with default DoS protection
    ///
    /// This method creates a default FramedMessage instance and uses it to read the message.
    /// For performance-critical applications or custom DoS protection settings,
    /// prefer creating a FramedMessage instance and reusing it.
    #[instrument(level = "debug", skip(reader))]
    pub async fn read_message_static(
        reader: &mut (impl AsyncRead + Unpin),
    ) -> Result<SignedEnvelope> {
        let framed = FramedMessage::default();
        framed.read_message(reader).await
    }

    /// Static convenience method for reading a message with timeout and default DoS protection
    #[instrument(level = "debug", skip(reader), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn read_message_with_timeout_static(
        reader: &mut (impl AsyncRead + Unpin),
        timeout_duration: Duration,
    ) -> Result<SignedEnvelope> {
        let framed = FramedMessage::default();
        framed
            .read_message_with_timeout(reader, timeout_duration)
            .await
    }

    /// Static convenience method for writing a message with timeout and default DoS protection
    #[instrument(level = "debug", skip(writer, envelope), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn write_message_with_timeout_static(
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
        timeout_duration: Duration,
    ) -> Result<()> {
        let framed = FramedMessage::default();
        framed
            .write_message_with_timeout(writer, envelope, timeout_duration)
            .await
    }

    /// Static convenience method for reading a message with default timeout and DoS protection
    #[instrument(level = "debug", skip(reader))]
    pub async fn read_message_with_default_timeout_static(
        reader: &mut (impl AsyncRead + Unpin),
    ) -> Result<SignedEnvelope> {
        let framed = FramedMessage::default();
        framed.read_message_with_default_timeout(reader).await
    }

    /// Static convenience method for writing a message with default timeout and DoS protection
    #[instrument(level = "debug", skip(writer, envelope))]
    pub async fn write_message_with_default_timeout_static(
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
    ) -> Result<()> {
        let framed = FramedMessage::default();
        framed
            .write_message_with_default_timeout(writer, envelope)
            .await
    }

    // Step 5.1: Network-specific FramedMessage constructors for appropriate defaults

    /// Create a FramedMessage optimized for general network operations
    /// Uses 1MB message size limit and 30-second timeouts
    pub fn for_network() -> Self {
        Self::new(WireConfig::for_network())
    }

    /// Create a FramedMessage optimized for small/control messages
    /// Uses 64KB message size limit and 10-second timeouts for responsive communication
    pub fn for_control_messages() -> Self {
        Self::new(WireConfig::for_control_messages())
    }

    /// Create a FramedMessage optimized for large file transfers
    /// Uses 8MB message size limit and extended timeouts for bulk operations
    pub fn for_large_transfers() -> Self {
        Self::new(WireConfig::for_large_transfers())
    }

    /// Create a FramedMessage optimized for server operations
    /// Balanced configuration for handling multiple concurrent connections
    pub fn for_server() -> Self {
        Self::new(WireConfig::for_server())
    }

    /// Create a FramedMessage optimized for client operations
    /// Slightly more aggressive timeouts for responsive client behavior
    pub fn for_client() -> Self {
        Self::new(WireConfig::for_client())
    }

    /// Create a FramedMessage for handshake operations
    /// Optimized for connection establishment with quick timeouts and small messages
    pub fn for_handshake() -> Self {
        Self::new(WireConfig::for_handshake())
    }

    /// Create a FramedMessage for testing with permissive settings
    /// Allows large messages and long timeouts for development/testing
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self::new(WireConfig::for_testing())
    }

    /// Create a FramedMessage for production with conservative, secure settings
    /// Optimized for security and resource management in production environments
    pub fn for_production() -> Self {
        Self::new(WireConfig::for_production())
    }

    // Chess-specific configurations for Step 8

    /// Create a FramedMessage optimized for standard chess messages (invites, moves, acknowledgments)
    /// Uses smaller size limits and faster timeouts for responsive gameplay
    /// Suitable for: GameInvite, GameAccept, GameDecline, Move, MoveAck, SyncRequest
    pub fn for_chess_standard() -> Self {
        Self::new(WireConfig::for_chess_standard())
    }

    /// Create a FramedMessage optimized for chess synchronization messages
    /// Uses larger size limits and extended timeouts to handle full game state transfers
    /// Suitable for: SyncResponse messages with complete move history and board state
    pub fn for_chess_sync() -> Self {
        Self::new(WireConfig::for_chess_sync())
    }

    /// Create a FramedMessage optimized for chess tournaments or bulk operations
    /// Uses maximum size limits and very extended timeouts for handling multiple games
    /// Suitable for: Bulk game synchronization, tournament data, multiple concurrent games
    pub fn for_chess_bulk() -> Self {
        Self::new(WireConfig::for_chess_bulk())
    }

    /// Create a FramedMessage optimized for real-time chess gameplay
    /// Uses balanced settings optimized for low-latency interactive play
    /// Suitable for: Real-time games where responsiveness is critical
    pub fn for_chess_realtime() -> Self {
        Self::new(WireConfig::for_chess_realtime())
    }

    /// Read a message with graceful degradation and retry logic (Step 4.3)
    ///
    /// This method provides enhanced resilience for message reading operations by:
    /// - Implementing exponential backoff retry logic for transient failures
    /// - Tracking connection state and degrading gracefully on repeated errors
    /// - Providing detailed logging and monitoring of retry attempts
    ///
    /// # Arguments
    /// * `reader` - The async reader to read from
    /// * `retry_config` - Configuration for retry behavior
    /// * `connection_state` - Mutable reference to track connection health
    ///
    /// # Returns
    /// * `Ok(SignedEnvelope)` - Successfully read and validated message
    /// * `Err(WireProtocolError)` - Operation failed after all retry attempts
    #[instrument(level = "debug", skip(self, reader, connection_state), fields(
        max_attempts = retry_config.max_attempts,
        connection_state = ?connection_state
    ))]
    pub async fn read_message_with_graceful_degradation(
        &self,
        reader: &mut (impl AsyncRead + Unpin),
        retry_config: &RetryConfig,
        connection_state: &mut ConnectionState,
    ) -> Result<SignedEnvelope, WireProtocolError> {
        let mut last_error = None;

        for attempt in 1..=retry_config.max_attempts {
            // Check if connection state allows operations
            if !connection_state.can_attempt_operation() {
                warn!(
                    operation = "read_message",
                    attempt = attempt,
                    connection_state = ?connection_state,
                    "Aborting operation due to connection state"
                );

                if let Some(err) = last_error {
                    return Err(err);
                } else {
                    return Err(WireProtocolError::connection_closed(format!(
                        "read_message - connection in unusable state: {:?}",
                        connection_state
                    )));
                }
            }

            // Log retry attempt
            if attempt > 1 {
                debug!(
                    operation = "read_message",
                    attempt = attempt,
                    max_attempts = retry_config.max_attempts,
                    connection_state = ?connection_state,
                    "Retrying operation"
                );
            }

            // Execute the operation
            match self.read_message(reader).await {
                Ok(result) => {
                    // Operation succeeded - update connection state
                    connection_state.update_on_success();

                    if attempt > 1 {
                        debug!(
                            operation = "read_message",
                            attempt = attempt,
                            "Operation succeeded after retry"
                        );
                    }

                    return Ok(result);
                }
                Err(error) => {
                    let wire_error = WireProtocolError::from(error);

                    // Update connection state based on error
                    connection_state.update_on_error(&wire_error);

                    // Check if we should retry this error
                    let should_retry = retry_config.should_retry(&wire_error);
                    let is_last_attempt = attempt >= retry_config.max_attempts;

                    if should_retry && !is_last_attempt {
                        let delay = retry_config.calculate_delay(attempt);

                        warn!(
                            operation = "read_message",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            delay_ms = delay.as_millis(),
                            connection_state = ?connection_state,
                            "Operation failed, will retry after delay"
                        );

                        // Wait before retrying
                        if delay > Duration::from_millis(0) {
                            tokio::time::sleep(delay).await;
                        }

                        last_error = Some(wire_error);
                    } else {
                        // Either not retryable or last attempt
                        error!(
                            operation = "read_message",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            should_retry = should_retry,
                            is_last_attempt = is_last_attempt,
                            connection_state = ?connection_state,
                            "Operation failed permanently"
                        );

                        return Err(wire_error);
                    }
                }
            }
        }

        // This should never be reached, but handle it gracefully
        if let Some(err) = last_error {
            Err(err)
        } else {
            Err(WireProtocolError::operation_timeout(
                Duration::from_secs(0),
                "read_message - max retry attempts reached".to_string(),
            ))
        }
    }

    /// Write a message with graceful degradation and retry logic (Step 4.3)
    ///
    /// This method provides enhanced resilience for message writing operations by:
    /// - Implementing exponential backoff retry logic for transient failures
    /// - Tracking connection state and degrading gracefully on repeated errors
    /// - Providing detailed logging and monitoring of retry attempts
    ///
    /// # Arguments
    /// * `writer` - The async writer to write to
    /// * `envelope` - The message to send
    /// * `retry_config` - Configuration for retry behavior
    /// * `connection_state` - Mutable reference to track connection health
    ///
    /// # Returns
    /// * `Ok(())` - Message was successfully written
    /// * `Err(WireProtocolError)` - Operation failed after all retry attempts
    #[instrument(level = "debug", skip(self, writer, envelope, connection_state), fields(
        max_attempts = retry_config.max_attempts,
        connection_state = ?connection_state
    ))]
    pub async fn write_message_with_graceful_degradation(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
        retry_config: &RetryConfig,
        connection_state: &mut ConnectionState,
    ) -> Result<(), WireProtocolError> {
        let mut last_error = None;

        for attempt in 1..=retry_config.max_attempts {
            // Check if connection state allows operations
            if !connection_state.can_attempt_operation() {
                warn!(
                    operation = "write_message",
                    attempt = attempt,
                    connection_state = ?connection_state,
                    "Aborting operation due to connection state"
                );

                if let Some(err) = last_error {
                    return Err(err);
                } else {
                    return Err(WireProtocolError::connection_closed(format!(
                        "write_message - connection in unusable state: {:?}",
                        connection_state
                    )));
                }
            }

            // Log retry attempt
            if attempt > 1 {
                debug!(
                    operation = "write_message",
                    attempt = attempt,
                    max_attempts = retry_config.max_attempts,
                    connection_state = ?connection_state,
                    "Retrying operation"
                );
            }

            // Execute the operation
            match self.write_message(writer, envelope).await {
                Ok(result) => {
                    // Operation succeeded - update connection state
                    connection_state.update_on_success();

                    if attempt > 1 {
                        debug!(
                            operation = "write_message",
                            attempt = attempt,
                            "Operation succeeded after retry"
                        );
                    }

                    return Ok(result);
                }
                Err(error) => {
                    let wire_error = WireProtocolError::from(error);

                    // Update connection state based on error
                    connection_state.update_on_error(&wire_error);

                    // Check if we should retry this error
                    let should_retry = retry_config.should_retry(&wire_error);
                    let is_last_attempt = attempt >= retry_config.max_attempts;

                    if should_retry && !is_last_attempt {
                        let delay = retry_config.calculate_delay(attempt);

                        warn!(
                            operation = "write_message",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            delay_ms = delay.as_millis(),
                            connection_state = ?connection_state,
                            "Operation failed, will retry after delay"
                        );

                        // Wait before retrying
                        if delay > Duration::from_millis(0) {
                            tokio::time::sleep(delay).await;
                        }

                        last_error = Some(wire_error);
                    } else {
                        // Either not retryable or last attempt
                        error!(
                            operation = "write_message",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            should_retry = should_retry,
                            is_last_attempt = is_last_attempt,
                            connection_state = ?connection_state,
                            "Operation failed permanently"
                        );

                        return Err(wire_error);
                    }
                }
            }
        }

        // This should never be reached, but handle it gracefully
        if let Some(err) = last_error {
            Err(err)
        } else {
            Err(WireProtocolError::operation_timeout(
                Duration::from_secs(0),
                "write_message - max retry attempts reached".to_string(),
            ))
        }
    }

    /// Read a message with timeout and graceful degradation (Step 4.3)
    ///
    /// Combines timeout handling with retry logic for maximum resilience.
    /// This is the recommended method for production network operations.
    #[instrument(level = "debug", skip(self, reader, connection_state), fields(
        timeout_secs = timeout_duration.as_secs(),
        max_attempts = retry_config.max_attempts,
        connection_state = ?connection_state
    ))]
    pub async fn read_message_with_timeout_and_graceful_degradation(
        &self,
        reader: &mut (impl AsyncRead + Unpin),
        timeout_duration: Duration,
        retry_config: &RetryConfig,
        connection_state: &mut ConnectionState,
    ) -> Result<SignedEnvelope, WireProtocolError> {
        let mut last_error = None;

        for attempt in 1..=retry_config.max_attempts {
            // Check if connection state allows operations
            if !connection_state.can_attempt_operation() {
                warn!(
                    operation = "read_message_with_timeout",
                    attempt = attempt,
                    connection_state = ?connection_state,
                    "Aborting operation due to connection state"
                );

                if let Some(err) = last_error {
                    return Err(err);
                } else {
                    return Err(WireProtocolError::connection_closed(format!(
                        "read_message_with_timeout - connection in unusable state: {:?}",
                        connection_state
                    )));
                }
            }

            // Log retry attempt
            if attempt > 1 {
                debug!(
                    operation = "read_message_with_timeout",
                    attempt = attempt,
                    max_attempts = retry_config.max_attempts,
                    connection_state = ?connection_state,
                    "Retrying operation"
                );
            }

            // Execute the operation with timeout
            match self
                .read_message_with_timeout(reader, timeout_duration)
                .await
            {
                Ok(result) => {
                    // Operation succeeded - update connection state
                    connection_state.update_on_success();

                    if attempt > 1 {
                        debug!(
                            operation = "read_message_with_timeout",
                            attempt = attempt,
                            "Operation succeeded after retry"
                        );
                    }

                    return Ok(result);
                }
                Err(error) => {
                    let wire_error = WireProtocolError::from(error);

                    // Update connection state based on error
                    connection_state.update_on_error(&wire_error);

                    // Check if we should retry this error
                    let should_retry = retry_config.should_retry(&wire_error);
                    let is_last_attempt = attempt >= retry_config.max_attempts;

                    if should_retry && !is_last_attempt {
                        let delay = retry_config.calculate_delay(attempt);

                        warn!(
                            operation = "read_message_with_timeout",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            delay_ms = delay.as_millis(),
                            connection_state = ?connection_state,
                            "Operation failed, will retry after delay"
                        );

                        // Wait before retrying
                        if delay > Duration::from_millis(0) {
                            tokio::time::sleep(delay).await;
                        }

                        last_error = Some(wire_error);
                    } else {
                        // Either not retryable or last attempt
                        error!(
                            operation = "read_message_with_timeout",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            should_retry = should_retry,
                            is_last_attempt = is_last_attempt,
                            connection_state = ?connection_state,
                            "Operation failed permanently"
                        );

                        return Err(wire_error);
                    }
                }
            }
        }

        // This should never be reached, but handle it gracefully
        if let Some(err) = last_error {
            Err(err)
        } else {
            Err(WireProtocolError::operation_timeout(
                Duration::from_secs(0),
                "read_message_with_timeout - max retry attempts reached".to_string(),
            ))
        }
    }

    /// Write a message with timeout and graceful degradation (Step 4.3)
    ///
    /// Combines timeout handling with retry logic for maximum resilience.
    /// This is the recommended method for production network operations.
    #[instrument(level = "debug", skip(self, writer, envelope, connection_state), fields(
        timeout_secs = timeout_duration.as_secs(),
        max_attempts = retry_config.max_attempts,
        connection_state = ?connection_state
    ))]
    pub async fn write_message_with_timeout_and_graceful_degradation(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
        timeout_duration: Duration,
        retry_config: &RetryConfig,
        connection_state: &mut ConnectionState,
    ) -> Result<(), WireProtocolError> {
        let mut last_error = None;

        for attempt in 1..=retry_config.max_attempts {
            // Check if connection state allows operations
            if !connection_state.can_attempt_operation() {
                warn!(
                    operation = "write_message_with_timeout",
                    attempt = attempt,
                    connection_state = ?connection_state,
                    "Aborting operation due to connection state"
                );

                if let Some(err) = last_error {
                    return Err(err);
                } else {
                    return Err(WireProtocolError::connection_closed(format!(
                        "write_message_with_timeout - connection in unusable state: {:?}",
                        connection_state
                    )));
                }
            }

            // Log retry attempt
            if attempt > 1 {
                debug!(
                    operation = "write_message_with_timeout",
                    attempt = attempt,
                    max_attempts = retry_config.max_attempts,
                    connection_state = ?connection_state,
                    "Retrying operation"
                );
            }

            // Execute the operation with timeout
            match self
                .write_message_with_timeout(writer, envelope, timeout_duration)
                .await
            {
                Ok(result) => {
                    // Operation succeeded - update connection state
                    connection_state.update_on_success();

                    if attempt > 1 {
                        debug!(
                            operation = "write_message_with_timeout",
                            attempt = attempt,
                            "Operation succeeded after retry"
                        );
                    }

                    return Ok(result);
                }
                Err(error) => {
                    let wire_error = WireProtocolError::from(error);

                    // Update connection state based on error
                    connection_state.update_on_error(&wire_error);

                    // Check if we should retry this error
                    let should_retry = retry_config.should_retry(&wire_error);
                    let is_last_attempt = attempt >= retry_config.max_attempts;

                    if should_retry && !is_last_attempt {
                        let delay = retry_config.calculate_delay(attempt);

                        warn!(
                            operation = "write_message_with_timeout",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            delay_ms = delay.as_millis(),
                            connection_state = ?connection_state,
                            "Operation failed, will retry after delay"
                        );

                        // Wait before retrying
                        if delay > Duration::from_millis(0) {
                            tokio::time::sleep(delay).await;
                        }

                        last_error = Some(wire_error);
                    } else {
                        // Either not retryable or last attempt
                        error!(
                            operation = "write_message_with_timeout",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            should_retry = should_retry,
                            is_last_attempt = is_last_attempt,
                            connection_state = ?connection_state,
                            "Operation failed permanently"
                        );

                        return Err(wire_error);
                    }
                }
            }
        }

        // This should never be reached, but handle it gracefully
        if let Some(err) = last_error {
            Err(err)
        } else {
            Err(WireProtocolError::operation_timeout(
                Duration::from_secs(0),
                "write_message_with_timeout - max retry attempts reached".to_string(),
            ))
        }
    }

    /// Perform a connection health check with graceful degradation (Step 4.3)
    ///
    /// This method can be used to verify connection health and update connection state
    /// without performing actual message operations. Useful for connection pooling
    /// and maintenance tasks.
    ///
    /// # Implementation Note
    /// This is a placeholder implementation that tests basic I/O capability.
    /// Real implementations should consider sending ping/pong messages or similar.
    #[instrument(level = "debug", skip(self, _reader, writer, connection_state))]
    pub async fn health_check_with_graceful_degradation(
        &self,
        _reader: &mut (impl AsyncRead + Unpin),
        writer: &mut (impl AsyncWrite + Unpin),
        retry_config: &RetryConfig,
        connection_state: &mut ConnectionState,
    ) -> Result<(), WireProtocolError> {
        let mut last_error = None;

        for attempt in 1..=retry_config.max_attempts {
            // Check if connection state allows operations
            if !connection_state.can_attempt_operation() {
                warn!(
                    operation = "health_check",
                    attempt = attempt,
                    connection_state = ?connection_state,
                    "Aborting operation due to connection state"
                );

                if let Some(err) = last_error {
                    return Err(err);
                } else {
                    return Err(WireProtocolError::connection_closed(format!(
                        "health_check - connection in unusable state: {:?}",
                        connection_state
                    )));
                }
            }

            // Log retry attempt
            if attempt > 1 {
                debug!(
                    operation = "health_check",
                    attempt = attempt,
                    max_attempts = retry_config.max_attempts,
                    connection_state = ?connection_state,
                    "Retrying operation"
                );
            }

            // Execute the health check
            match writer.flush().await {
                Ok(_) => {
                    // Operation succeeded - update connection state
                    connection_state.update_on_success();

                    if attempt > 1 {
                        debug!(
                            operation = "health_check",
                            attempt = attempt,
                            "Operation succeeded after retry"
                        );
                    }

                    debug!("Health check: connection appears healthy");
                    return Ok(());
                }
                Err(error) => {
                    let wire_error = WireProtocolError::Io(error);

                    // Update connection state based on error
                    connection_state.update_on_error(&wire_error);

                    // Check if we should retry this error
                    let should_retry = retry_config.should_retry(&wire_error);
                    let is_last_attempt = attempt >= retry_config.max_attempts;

                    if should_retry && !is_last_attempt {
                        let delay = retry_config.calculate_delay(attempt);

                        warn!(
                            operation = "health_check",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            delay_ms = delay.as_millis(),
                            connection_state = ?connection_state,
                            "Operation failed, will retry after delay"
                        );

                        // Wait before retrying
                        if delay > Duration::from_millis(0) {
                            tokio::time::sleep(delay).await;
                        }

                        last_error = Some(wire_error);
                    } else {
                        // Either not retryable or last attempt
                        error!(
                            operation = "health_check",
                            attempt = attempt,
                            max_attempts = retry_config.max_attempts,
                            error = %wire_error,
                            should_retry = should_retry,
                            is_last_attempt = is_last_attempt,
                            connection_state = ?connection_state,
                            "Operation failed permanently"
                        );

                        return Err(wire_error);
                    }
                }
            }
        }

        // This should never be reached, but handle it gracefully
        if let Some(err) = last_error {
            Err(err)
        } else {
            Err(WireProtocolError::operation_timeout(
                Duration::from_secs(0),
                "health_check - max retry attempts reached".to_string(),
            ))
        }
    }

    /// Create a resilient message exchange session (Step 4.3)
    ///
    /// This helper method creates a reusable context for message operations with
    /// consistent retry configuration and connection state tracking.
    pub fn create_resilient_session(&self, retry_config: RetryConfig) -> ResilientSession {
        ResilientSession {
            framed_message: self.clone(),
            retry_config,
            connection_state: ConnectionState::Healthy,
        }
    }
}

/// Retry configuration for graceful degradation (Step 4.3)
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub retry_on_timeout: bool,
    pub retry_on_connection_errors: bool,
    pub retry_on_transient_io_errors: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: CLIENT_RETRY_MAX_ATTEMPTS,
            base_delay: CLIENT_RETRY_BASE_DELAY,
            max_delay: Duration::from_secs(60), // Cap at 1 minute
            backoff_multiplier: 2.0,
            retry_on_timeout: true,
            retry_on_connection_errors: true,
            retry_on_transient_io_errors: true,
        }
    }
}

impl RetryConfig {
    /// Create a conservative retry configuration for production use
    pub fn conservative() -> Self {
        Self {
            max_attempts: 2,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 1.5,
            retry_on_timeout: true,
            retry_on_connection_errors: false, // Don't retry on connection errors in production
            retry_on_transient_io_errors: true,
        }
    }

    /// Create an aggressive retry configuration for development/testing
    pub fn aggressive() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            retry_on_timeout: true,
            retry_on_connection_errors: true,
            retry_on_transient_io_errors: true,
        }
    }

    /// Create a no-retry configuration for operations that must not be retried
    pub fn no_retry() -> Self {
        Self {
            max_attempts: 1,
            base_delay: Duration::from_millis(0),
            max_delay: Duration::from_millis(0),
            backoff_multiplier: 1.0,
            retry_on_timeout: false,
            retry_on_connection_errors: false,
            retry_on_transient_io_errors: false,
        }
    }

    /// Calculate the delay for a specific retry attempt with exponential backoff
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        let multiplier = self.backoff_multiplier.powi((attempt - 1) as i32);
        let delay_ms = (self.base_delay.as_millis() as f64 * multiplier) as u64;
        let delay = Duration::from_millis(delay_ms);

        // Cap at max_delay
        if delay > self.max_delay {
            self.max_delay
        } else {
            delay
        }
    }

    /// Check if an error should trigger a retry based on configuration
    pub fn should_retry(&self, error: &WireProtocolError) -> bool {
        match error {
            // Timeout errors
            WireProtocolError::ReadTimeout { .. }
            | WireProtocolError::WriteTimeout { .. }
            | WireProtocolError::OperationTimeout { .. }
            | WireProtocolError::Timeout(_) => self.retry_on_timeout,

            // Connection errors
            WireProtocolError::ConnectionClosed { .. } => self.retry_on_connection_errors,

            // Transient IO errors
            WireProtocolError::Io(io_err) => match io_err.kind() {
                std::io::ErrorKind::Interrupted
                | std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::TimedOut => self.retry_on_transient_io_errors,

                // Connection-related IO errors
                std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe => self.retry_on_connection_errors,

                _ => false,
            },

            // Suspicious messages might be retryable (could be network corruption)
            WireProtocolError::SuspiciousMessageSize { .. } => self.retry_on_transient_io_errors,

            // These errors should never be retried
            WireProtocolError::MessageTooLarge { .. }
            | WireProtocolError::MessageTooSmall { .. }
            | WireProtocolError::InvalidLength { .. }
            | WireProtocolError::AllocationDenied { .. }
            | WireProtocolError::ProtocolViolation { .. }
            | WireProtocolError::CorruptedData { .. }
            | WireProtocolError::InvalidMessageFormat { .. }
            | WireProtocolError::BufferOverflow { .. }
            | WireProtocolError::LengthMismatch { .. }
            | WireProtocolError::Serialization(_)
            | WireProtocolError::UnexpectedEof { .. } => false,
        }
    }
}

/// Connection state tracking for recovery purposes (Step 4.3)
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Connection is healthy and operational
    Healthy,
    /// Connection experienced a recoverable error but may still be usable
    Degraded {
        error_count: u32,
        last_error: String,
    },
    /// Connection is suspected to be broken and should be replaced
    Broken { reason: String },
    /// Connection is being recovered/reconnected
    Recovering,
}

impl ConnectionState {
    /// Check if the connection state allows for operation attempts
    pub fn can_attempt_operation(&self) -> bool {
        match self {
            ConnectionState::Healthy => true,
            ConnectionState::Degraded { error_count, .. } => *error_count < 3,
            ConnectionState::Broken { .. } => false,
            ConnectionState::Recovering => false,
        }
    }

    /// Update the connection state based on an operation result
    pub fn update_on_error(&mut self, error: &WireProtocolError) {
        match self {
            ConnectionState::Healthy => {
                if error.is_recoverable() {
                    *self = ConnectionState::Degraded {
                        error_count: 1,
                        last_error: error.to_string(),
                    };
                } else {
                    *self = ConnectionState::Broken {
                        reason: error.to_string(),
                    };
                }
            }
            ConnectionState::Degraded {
                error_count,
                last_error,
            } => {
                if error.is_recoverable() {
                    *error_count += 1;
                    *last_error = error.to_string();

                    if *error_count >= 3 {
                        *self = ConnectionState::Broken {
                            reason: format!("Too many recoverable errors: {}", last_error),
                        };
                    }
                } else {
                    *self = ConnectionState::Broken {
                        reason: error.to_string(),
                    };
                }
            }
            ConnectionState::Broken { .. } => {
                // Already broken, no state change needed
            }
            ConnectionState::Recovering => {
                // If we get an error while recovering, mark as broken
                *self = ConnectionState::Broken {
                    reason: format!("Error during recovery: {}", error),
                };
            }
        }
    }

    /// Update the connection state after a successful operation
    pub fn update_on_success(&mut self) {
        match self {
            ConnectionState::Healthy => {
                // Already healthy, no change needed
            }
            ConnectionState::Degraded { .. } => {
                // Successful operation recovered the connection
                *self = ConnectionState::Healthy;
            }
            ConnectionState::Broken { .. } => {
                // Unexpected success on broken connection - mark as healthy
                warn!("Unexpected successful operation on broken connection, marking as healthy");
                *self = ConnectionState::Healthy;
            }
            ConnectionState::Recovering => {
                // Recovery completed successfully
                *self = ConnectionState::Healthy;
            }
        }
    }

    /// Mark the connection as recovering
    pub fn mark_recovering(&mut self) {
        *self = ConnectionState::Recovering;
    }
}

/// A resilient session for message operations with persistent connection state (Step 4.3)
#[derive(Debug, Clone)]
pub struct ResilientSession {
    framed_message: FramedMessage,
    retry_config: RetryConfig,
    connection_state: ConnectionState,
}

impl ResilientSession {
    /// Create a new resilient session with custom configuration
    pub fn new(framed_message: FramedMessage, retry_config: RetryConfig) -> Self {
        Self {
            framed_message,
            retry_config,
            connection_state: ConnectionState::Healthy,
        }
    }

    /// Get the current connection state
    pub fn connection_state(&self) -> &ConnectionState {
        &self.connection_state
    }

    /// Reset the connection state to healthy (use after successful reconnection)
    pub fn reset_connection_state(&mut self) {
        self.connection_state = ConnectionState::Healthy;
    }

    /// Read a message with session-managed retry and connection state
    pub async fn read_message(
        &mut self,
        reader: &mut (impl AsyncRead + Unpin),
    ) -> Result<SignedEnvelope, WireProtocolError> {
        self.framed_message
            .read_message_with_graceful_degradation(
                reader,
                &self.retry_config,
                &mut self.connection_state,
            )
            .await
    }

    /// Write a message with session-managed retry and connection state
    pub async fn write_message(
        &mut self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
    ) -> Result<(), WireProtocolError> {
        self.framed_message
            .write_message_with_graceful_degradation(
                writer,
                envelope,
                &self.retry_config,
                &mut self.connection_state,
            )
            .await
    }
}

/// Summary information about a resilient session for monitoring
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub connection_state: ConnectionState,
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub can_attempt_operations: bool,
}
