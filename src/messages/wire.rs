use crate::messages::SignedEnvelope;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use anyhow::{Result, Context};
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, error, warn, trace, instrument};

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
/// ```rust,ignore
/// pub struct RateLimitConfig {
///     pub max_messages_per_second: u32,
///     pub max_bytes_per_second: u64,
///     pub max_connections_per_ip: u32,
///     pub connection_rate_limit: u32,
///     pub burst_allowance: u32,
/// }
/// ```
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
    
    #[error("Invalid length prefix: {length}")]
    InvalidLength { length: u32 },
    
    #[error("Suspicious message size: {size} bytes exceeds threshold of {threshold} bytes")]
    SuspiciousMessageSize { size: usize, threshold: usize },
    
    #[error("Message too small: {size} bytes is below minimum of {min_size} bytes")]
    MessageTooSmall { size: usize, min_size: usize },
    
    #[error("Memory allocation denied: {size} bytes exceeds safe allocation limit of {limit} bytes")]
    AllocationDenied { size: usize, limit: usize },
    
    #[error("Read operation timed out after {timeout:?}")]
    ReadTimeout { timeout: Duration },
    
    #[error("Write operation timed out after {timeout:?}")]
    WriteTimeout { timeout: Duration },
    
    #[error("Corrupted data: {reason}")]
    CorruptedData { reason: String },
    
    #[error("Unexpected end of file while reading {operation}")]
    UnexpectedEof { operation: String },
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct FramedMessage {
    wire_config: WireConfig,
    dos_config: DosProtectionConfig,
}

impl Default for FramedMessage {
    fn default() -> Self {
        Self {
            wire_config: WireConfig::default(),
            dos_config: DosProtectionConfig::default(),
        }
    }
}

impl FramedMessage {
    /// Create a new FramedMessage with custom wire protocol configuration
    pub fn new(wire_config: WireConfig) -> Self {
        let mut dos_config = DosProtectionConfig::default();
        // Sync the max_message_size between configs
        dos_config.max_message_size = wire_config.max_message_size;
        
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
        let mut dos_config = DosProtectionConfig::default();
        dos_config.max_message_size = max_message_size;
        
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

    /// Validate message size against DoS protection thresholds
    #[instrument(level = "trace", skip(self), fields(size, max_size = self.dos_config.max_message_size))]
    fn validate_message_size(&self, size: usize) -> Result<(), WireProtocolError> {
        tracing::Span::current().record("size", size);
        
        // Check minimum size to prevent zero-length or malformed messages
        if size < self.dos_config.min_message_size {
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
            error!(
                size = size,
                max_size = self.dos_config.max_message_size,
                "Message size exceeds maximum allowed size"
            );
            return Err(WireProtocolError::MessageTooLarge {
                size,
                max_size: self.dos_config.max_message_size,
            });
        }
        
        // Log suspicious but allowed message sizes for monitoring
        if size > self.dos_config.suspicious_threshold {
            warn!(
                size = size,
                threshold = self.dos_config.suspicious_threshold,
                max_size = self.dos_config.max_message_size,
                "Message size exceeds suspicious threshold but is still allowed"
            );
        }
        
        debug!("Message size validation passed for {} bytes", size);
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
            error!(
                size = size,
                "Allocation request exceeds safe memory bounds"
            );
            return Err(WireProtocolError::AllocationDenied {
                size,
                limit: isize::MAX as usize / 2,
            });
        }
        
        trace!("Allocating {} bytes for message buffer", size);
        
        // Attempt allocation with error handling
        match Vec::with_capacity(size) {
            mut vec => {
                // Initialize the vector to the required size
                vec.resize(size, 0);
                debug!("Successfully allocated {} byte buffer", size);
                Ok(vec)
            }
        }
    }

    /// Serialize a SignedEnvelope to bytes with enhanced DoS protection
    #[instrument(level = "trace", skip(self, envelope), fields(envelope_size))]
    fn serialize_envelope(&self, envelope: &SignedEnvelope) -> Result<Vec<u8>, WireProtocolError> {
        trace!("Starting envelope serialization with DoS protection");
        
        // Serialize the envelope using bincode
        let serialized = bincode::serialize(envelope)
            .map_err(|e| {
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
        let envelope = bincode::deserialize(data)
            .map_err(|e| {
                error!(
                    error = %e,
                    data_size = data.len(),
                    "Failed to deserialize data to SignedEnvelope"
                );
                WireProtocolError::CorruptedData {
                    reason: format!("Failed to deserialize SignedEnvelope: {}", e),
                }
            })?;
            
        debug!("Successfully deserialized envelope from {} bytes", data.len());
        trace!("Envelope deserialization completed successfully with DoS validation");
        Ok(envelope)
    }
    
    /// Enhanced length validation with comprehensive DoS protection
    #[instrument(level = "trace", skip(self))]
    fn validate_length(&self, length: u32) -> Result<usize, WireProtocolError> {
        trace!("Validating message length: {} with enhanced DoS protection", length);
        
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
            return Err(WireProtocolError::InvalidLength { length });
        }
        
        // Check for zero length (should not happen for valid messages)
        if length == 0 {
            warn!("Received zero-length message prefix");
            return Err(WireProtocolError::InvalidLength { length });
        }
        
        debug!("Enhanced length validation passed for {} bytes", length_usize);
        Ok(length_usize)
    }

    /// Robust write operation with recovery logic for partial writes
    #[instrument(level = "debug", skip(writer, data), fields(data_size = data.len()))]
    async fn write_all_with_recovery(
        writer: &mut (impl AsyncWrite + Unpin),
        data: &[u8]
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
        
        debug!("Write operation completed successfully, {} bytes written", total_written);
        Ok(())
    }

    /// Robust read operation with recovery logic for partial reads
    #[instrument(level = "debug", skip(reader, buffer), fields(buffer_size = buffer.len()))]
    async fn read_exact_with_recovery(
        reader: &mut (impl AsyncRead + Unpin),
        buffer: &mut [u8]
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
        
        debug!("Read operation completed successfully, {} bytes read", total_read);
        Ok(())
    }

    #[instrument(level = "debug", skip(self, writer, envelope))]
    pub async fn write_message(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope
    ) -> Result<()> {
        debug!("Starting message write operation with DoS protection");
        
        // Serialize the envelope to bytes with enhanced validation
        let message_bytes = self.serialize_envelope(envelope)
            .with_context(|| "Failed to serialize envelope for writing")?;
        
        // Get the message length as u32 for the length prefix
        let message_length = message_bytes.len() as u32;
        debug!("Prepared message: {} bytes with length prefix", message_length);
        
        // Create the 4-byte length prefix (big-endian)
        let length_prefix = message_length.to_be_bytes();
        
        // Write the length prefix first with recovery logic
        Self::write_all_with_recovery(writer, &length_prefix).await
            .with_context(|| format!("Failed to write 4-byte length prefix ({})", message_length))?;
        
        // Write the message bytes with recovery logic
        Self::write_all_with_recovery(writer, &message_bytes).await
            .with_context(|| format!("Failed to write message data ({} bytes)", message_bytes.len()))?;
        
        // Ensure all data is flushed to the underlying writer
        writer.flush().await
            .with_context(|| "Failed to flush writer after message write")?;
        
        debug!("Message write operation completed successfully with DoS protection");
        Ok(())
    }
    
    #[instrument(level = "debug", skip(self, reader))]
    pub async fn read_message(
        &self,
        reader: &mut (impl AsyncRead + Unpin)
    ) -> Result<SignedEnvelope> {
        debug!("Starting message read operation with DoS protection");
        
        // Read the 4-byte length prefix with recovery logic
        let mut length_buffer = [0u8; LENGTH_PREFIX_SIZE];
        Self::read_exact_with_recovery(reader, &mut length_buffer).await
            .with_context(|| "Failed to read 4-byte length prefix")?;
        
        // Parse the length as big-endian u32
        let message_length = u32::from_be_bytes(length_buffer);
        debug!("Read length prefix: {} bytes expected", message_length);
        
        // Validate the length with enhanced DoS protection
        let validated_length = self.validate_length(message_length)
            .with_context(|| format!("Invalid message length received: {}", message_length))?;
        
        // Safe allocation with DoS protection
        let mut message_buffer = self.safe_allocate(validated_length)
            .with_context(|| format!("Failed to allocate {} byte buffer", validated_length))?;
        debug!("Safely allocated buffer for {} byte message", validated_length);
        
        // Read exactly the specified number of bytes for the message with recovery logic
        Self::read_exact_with_recovery(reader, &mut message_buffer).await
            .with_context(|| {
                format!("Failed to read message data: expected {} bytes", validated_length)
            })?;
        
        // Deserialize the message bytes back to SignedEnvelope with enhanced validation
        let envelope = self.deserialize_envelope(&message_buffer)
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
    /// # Examples
    /// ```rust,ignore
    /// use std::time::Duration;
    /// 
    /// let framed = FramedMessage::default();
    /// let timeout = Duration::from_secs(10);
    /// let envelope = framed.read_message_with_timeout(&mut reader, timeout).await?;
    /// ```
    #[instrument(level = "debug", skip(self, reader), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn read_message_with_timeout(
        &self,
        reader: &mut (impl AsyncRead + Unpin),
        timeout_duration: Duration
    ) -> Result<SignedEnvelope> {
        debug!("Starting timed message read operation with {:?} timeout and DoS protection", timeout_duration);
        
        let start_time = std::time::Instant::now();
        
        match tokio::time::timeout(timeout_duration, self.read_message(reader)).await {
            Ok(result) => {
                let elapsed = start_time.elapsed();
                debug!("Timed read operation with DoS protection completed in {:?}", elapsed);
                result
            }
            Err(_elapsed) => {
                let elapsed = start_time.elapsed();
                error!(
                    timeout = ?timeout_duration,
                    elapsed = ?elapsed,
                    "Read operation timed out"
                );
                Err(anyhow::anyhow!("{}", WireProtocolError::ReadTimeout { 
                    timeout: timeout_duration 
                }))
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
    /// # Examples
    /// ```rust,ignore
    /// use std::time::Duration;
    /// 
    /// let framed = FramedMessage::default();
    /// let timeout = Duration::from_secs(10);
    /// framed.write_message_with_timeout(&mut writer, &envelope, timeout).await?;
    /// ```
    #[instrument(level = "debug", skip(self, writer, envelope), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn write_message_with_timeout(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
        timeout_duration: Duration
    ) -> Result<()> {
        debug!("Starting timed message write operation with {:?} timeout and DoS protection", timeout_duration);
        
        let start_time = std::time::Instant::now();
        
        match tokio::time::timeout(timeout_duration, self.write_message(writer, envelope)).await {
            Ok(result) => {
                let elapsed = start_time.elapsed();
                debug!("Timed write operation with DoS protection completed in {:?}", elapsed);
                result
            }
            Err(_elapsed) => {
                let elapsed = start_time.elapsed();
                error!(
                    timeout = ?timeout_duration,
                    elapsed = ?elapsed,
                    "Write operation timed out"
                );
                Err(anyhow::anyhow!("{}", WireProtocolError::WriteTimeout { 
                    timeout: timeout_duration 
                }))
            }
        }
    }

    /// Read a message using the configured default timeout and DoS protection
    /// 
    /// Convenience method that uses the read timeout from the wire configuration.
    #[instrument(level = "debug", skip(self, reader), fields(timeout_secs = self.wire_config.read_timeout.as_secs()))]
    pub async fn read_message_with_default_timeout(
        &self,
        reader: &mut (impl AsyncRead + Unpin)
    ) -> Result<SignedEnvelope> {
        let timeout = self.wire_config.read_timeout;
        debug!("Starting read operation with configured default timeout ({:?}) and DoS protection", timeout);
        self.read_message_with_timeout(reader, timeout).await
    }

    /// Write a message using the configured default timeout and DoS protection
    /// 
    /// Convenience method that uses the write timeout from the wire configuration.
    #[instrument(level = "debug", skip(self, writer, envelope), fields(timeout_secs = self.wire_config.write_timeout.as_secs()))]
    pub async fn write_message_with_default_timeout(
        &self,
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope
    ) -> Result<()> {
        let timeout = self.wire_config.write_timeout;
        debug!("Starting write operation with configured default timeout ({:?}) and DoS protection", timeout);
        self.write_message_with_timeout(writer, envelope, timeout).await
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
        envelope: &SignedEnvelope
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
        reader: &mut (impl AsyncRead + Unpin)
    ) -> Result<SignedEnvelope> {
        let framed = FramedMessage::default();
        framed.read_message(reader).await
    }

    /// Static convenience method for reading a message with timeout and default DoS protection
    #[instrument(level = "debug", skip(reader), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn read_message_with_timeout_static(
        reader: &mut (impl AsyncRead + Unpin),
        timeout_duration: Duration
    ) -> Result<SignedEnvelope> {
        let framed = FramedMessage::default();
        framed.read_message_with_timeout(reader, timeout_duration).await
    }

    /// Static convenience method for writing a message with timeout and default DoS protection
    #[instrument(level = "debug", skip(writer, envelope), fields(timeout_secs = timeout_duration.as_secs()))]
    pub async fn write_message_with_timeout_static(
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope,
        timeout_duration: Duration
    ) -> Result<()> {
        let framed = FramedMessage::default();
        framed.write_message_with_timeout(writer, envelope, timeout_duration).await
    }

    /// Static convenience method for reading a message with default timeout and DoS protection
    #[instrument(level = "debug", skip(reader))]
    pub async fn read_message_with_default_timeout_static(
        reader: &mut (impl AsyncRead + Unpin)
    ) -> Result<SignedEnvelope> {
        let framed = FramedMessage::default();
        framed.read_message_with_default_timeout(reader).await
    }

    /// Static convenience method for writing a message with default timeout and DoS protection
    #[instrument(level = "debug", skip(writer, envelope))]
    pub async fn write_message_with_default_timeout_static(
        writer: &mut (impl AsyncWrite + Unpin),
        envelope: &SignedEnvelope
    ) -> Result<()> {
        let framed = FramedMessage::default();
        framed.write_message_with_default_timeout(writer, envelope).await
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
}
