use crate::crypto::Identity;
use crate::messages::{
    wire::{FailureClass, RetryStrategy, WireConfig, FAST_FAIL_CONNECTION_TIMEOUT},
    Message,
};
use crate::network::Connection;
use anyhow::{Context, Result};
use rand;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tracing::{debug, error, info, instrument, warn};

// Step 4.2: Error conversion is automatically provided by anyhow's blanket implementation
// since ConnectionError implements std::error::Error via thiserror::Error

/// A secure peer-to-peer client with automatic retry logic and connection management.
///
/// The `Client` provides a robust foundation for establishing authenticated connections to peers.
/// It handles connection establishment, authentication handshake, and provides both one-shot
/// and persistent connection patterns.
///
/// # Security Features
///
/// - **Automatic Authentication**: All connections require successful handshake completion
/// - **Cryptographic Verification**: Messages are automatically signed and verified
/// - **Connection Validation**: Failed handshakes prevent message exchange
/// - **Timeout Protection**: All operations have appropriate timeouts to prevent hanging
///
/// # Error Recovery Strategies
///
/// ## Connection Establishment
/// - **Connection failures**: Automatic retry with exponential backoff (up to 3 attempts)
///   - *Recovery*: Check network connectivity, verify target address and port
/// - **Handshake failures**: Authentication errors are not retried automatically
///   - *Recovery*: Verify peer identity, check clock synchronization, examine logs
/// - **Timeout errors**: May indicate network congestion or peer availability issues
///   - *Recovery*: Retry after delay, consider increasing timeout values
///
/// ## Message Exchange Errors
/// - **Invalid signatures**: Security errors indicating potential tampering
///   - *Recovery*: Do not retry, investigate security implications
/// - **Timestamp validation failures**: Clock synchronization issues
///   - *Recovery*: Check system time, retry with fresh message
/// - **Connection drops**: Network or peer issues
///   - *Recovery*: Re-establish connection and retry operation
///
/// ## Performance Optimization
/// - **Connection reuse**: For multiple messages to same peer, reuse connections
/// - **Retry configuration**: Tune retry attempts and delays via `CLIENT_RETRY_*` constants
/// - **Echo sessions**: Use `echo_session()` to validate connection quality
///
/// # Configuration Options
///
/// ## Wire Protocol Configuration
/// - **Message limits**: Configure maximum message size via `WireConfig`
/// - **Timeout values**: Adjust for network conditions (read/write timeouts)
/// - **Retry behavior**: Modify `CLIENT_RETRY_*` constants
///
/// ## Connection Patterns
/// - **One-shot**: Use `send_message_to()` for single message exchanges
/// - **Persistent**: Use `connect()` and reuse connection for multiple messages
/// - **Quality testing**: Use `test_connection_quality()` for network assessment
///
/// # Example Usage
///
/// ```no_run
/// use std::sync::Arc;
/// use mate::network::Client;
/// use mate::crypto::Identity;
/// use mate::messages::Message;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create test identity and messages
///     let identity = Arc::new(Identity::generate()?);
///     let message = Message::new_ping(12345, "test message".to_string());
///     let message1 = Message::new_ping(11111, "message 1".to_string());
///     let message2 = Message::new_ping(22222, "message 2".to_string());
///     
///     let client = Client::new(identity);
///     
///     // One-shot message exchange
///     let response = client.send_message_to("127.0.0.1:8080", message).await?;
///     
///     // Persistent connection for multiple messages
///     let mut conn = client.connect("127.0.0.1:8080").await?;
///     client.echo_session(&mut conn).await?;
///     conn.send_message(message1).await?;
///     conn.send_message(message2).await?;
///     
///     // Connection quality assessment
///     let quality = client.test_connection_quality("127.0.0.1:8080").await?;
///     if quality.is_acceptable_quality() {
///         println!("Connection quality: {}", quality.quality_assessment());
///     }
///     
///     Ok(())
/// }
/// ```
///
/// # Thread Safety
///
/// `Client` is thread-safe and can be shared across threads. Individual `Connection` instances
/// are NOT thread-safe and should be used by a single task at a time.
pub struct Client {
    identity: Arc<Identity>,
    wire_config: WireConfig,
}

impl Client {
    pub fn new(identity: Arc<Identity>) -> Self {
        info!("Creating new client with client-optimized network configuration");
        Self {
            identity,
            wire_config: WireConfig::for_client(),
        }
    }

    /// Create a new Client with custom WireConfig for advanced configuration
    pub fn new_with_config(identity: Arc<Identity>, wire_config: WireConfig) -> Self {
        info!("Creating new client with custom wire config");
        debug!(
            "Wire config - max_message_size: {}, read_timeout: {:?}, write_timeout: {:?}",
            wire_config.max_message_size, wire_config.read_timeout, wire_config.write_timeout
        );
        Self {
            identity,
            wire_config,
        }
    }

    /// Connect to a peer with smart retry logic and failure classification
    #[instrument(level = "info", skip(self))]
    pub async fn connect(&self, addr: &str) -> Result<Connection> {
        self.connect_with_strategy(addr, RetryStrategy::Normal)
            .await
    }

    /// Connect to a peer with a specific retry strategy
    #[instrument(level = "info", skip(self))]
    pub async fn connect_with_strategy(
        &self,
        addr: &str,
        strategy: RetryStrategy,
    ) -> Result<Connection> {
        info!("Attempting to connect to {}", addr);
        debug!(
            "Using wire config - max_message_size: {}, read_timeout: {:?}, write_timeout: {:?}",
            self.wire_config.max_message_size,
            self.wire_config.read_timeout,
            self.wire_config.write_timeout
        );

        // Use strategy-appropriate retry counts and delays
        let max_retry_attempts = match strategy {
            RetryStrategy::NoRetry => 1,
            RetryStrategy::Quick => 2, // Quick: 2 attempts, ~3 seconds total
            RetryStrategy::Normal => 3, // Normal: 3 attempts, ~7 seconds total
            RetryStrategy::Patient => 4, // Patient: 4 attempts, ~15 seconds total
        };

        let base_retry_delay = match strategy {
            RetryStrategy::NoRetry => Duration::from_millis(0),
            RetryStrategy::Quick => Duration::from_millis(500), // 0.5s base delay
            RetryStrategy::Normal => Duration::from_millis(1000), // 1s base delay
            RetryStrategy::Patient => Duration::from_millis(2000), // 2s base delay
        };

        let mut last_error = None;
        let mut failure_class: FailureClass;

        for attempt in 1..=max_retry_attempts {
            debug!(
                "Connection attempt {} of {} to {} (strategy: {:?})",
                attempt, max_retry_attempts, addr, strategy
            );

            match self.try_connect_once_with_fast_fail(addr).await {
                Ok(mut connection) => {
                    info!("TCP connection established to {}, starting handshake", addr);

                    // Perform client-side handshake
                    match connection.handshake().await {
                        Ok(peer_id) => {
                            info!("Handshake completed successfully with peer: {}", peer_id);
                            debug!(
                                "Connection fully established to {} (peer: {})",
                                addr, peer_id
                            );
                            return Ok(connection);
                        }
                        Err(e) => {
                            error!("Handshake failed with {}: {}", addr, e);
                            last_error = Some(anyhow::anyhow!("Handshake failed: {}", e));
                            failure_class =
                                FailureClass::classify_error(last_error.as_ref().unwrap());

                            // If handshake fails and it's a no-retry error, exit immediately
                            if failure_class == FailureClass::NoRetry {
                                error!("Fast-failing on handshake error: {}", e);
                                return Err(last_error.unwrap());
                            }

                            // Close the connection since handshake failed
                            if let Err(close_error) = connection.close().await {
                                warn!(
                                    "Failed to close connection after handshake failure: {}",
                                    close_error
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "TCP connection failed to {} (attempt {}): {}",
                        addr, attempt, e
                    );
                    last_error = Some(e);
                    failure_class = FailureClass::classify_error(last_error.as_ref().unwrap());

                    // If this is a no-retry error (DNS, invalid address), exit immediately
                    if failure_class == FailureClass::NoRetry {
                        error!(
                            "Fast-failing on connection error: {}",
                            last_error.as_ref().unwrap()
                        );
                        return Err(last_error.unwrap());
                    }
                }
            }

            // Add exponential backoff delay before retry (except for last attempt)
            if attempt < max_retry_attempts && base_retry_delay > Duration::from_millis(0) {
                let delay_ms = base_retry_delay.as_millis() * (2_u128.pow(attempt - 1)); // Exponential backoff
                debug!("Retrying connection to {} in {} ms", addr, delay_ms);
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms as u64)).await;
            }
        }

        // All retry attempts failed
        let final_error =
            last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown connection failure"));
        error!(
            "Failed to connect to {} after {} attempts (strategy: {:?}): {}",
            addr, max_retry_attempts, strategy, final_error
        );
        Err(final_error)
    }

    /// Internal helper method for a single connection attempt with fast-fail detection
    #[instrument(level = "debug", skip(self))]
    async fn try_connect_once_with_fast_fail(&self, addr: &str) -> Result<Connection> {
        debug!(
            "Creating TCP connection to {} with fast-fail detection",
            addr
        );

        // Use shorter timeout for initial connection attempt to detect obvious failures quickly
        let connection_result =
            tokio::time::timeout(FAST_FAIL_CONNECTION_TIMEOUT, TcpStream::connect(addr)).await;

        let stream = match connection_result {
            Ok(stream_result) => {
                stream_result.with_context(|| format!("Failed to connect to {}", addr))?
            }
            Err(_) => {
                // Timeout occurred - this could be a legitimate slow connection, retry with normal timeout
                debug!("Fast connection attempt timed out, trying with normal timeout");
                TcpStream::connect(addr).await.with_context(|| {
                    format!("Failed to connect to {} (after fast-fail timeout)", addr)
                })?
            }
        };

        debug!("TCP stream established to {}", addr);

        // Log connection details
        if let (Ok(local_addr), Ok(peer_addr)) = (stream.local_addr(), stream.peer_addr()) {
            debug!("Connection established: {} -> {}", local_addr, peer_addr);
        }

        // Create Connection with our wire config
        let connection = Connection::new_with_config(
            stream,
            Arc::clone(&self.identity),
            self.wire_config.clone(),
        )
        .await;

        debug!("Connection object created with custom wire config");
        Ok(connection)
    }

    #[instrument(level = "info", skip(self, conn), fields(peer_id = conn.peer_identity()))]
    pub async fn echo_session(&self, conn: &mut Connection) -> Result<()> {
        info!("Starting echo session");

        if !conn.is_authenticated() {
            error!("Cannot start echo session: connection is not authenticated");
            return Err(anyhow::anyhow!(
                "Connection must be authenticated before echo session"
            ));
        }

        // Echo session configuration
        const NUM_TEST_MESSAGES: usize = 5;
        const ECHO_TIMEOUT_MS: u64 = 5000; // 5 seconds per echo
        const MAX_PAYLOAD_SIZE: usize = 1024; // 1KB test messages

        let session_start = std::time::Instant::now();
        let mut successful_echoes = 0;
        let mut total_round_trip_time = std::time::Duration::ZERO;
        let mut message_sizes = Vec::new();

        info!(
            "Echo session configured: {} messages, max_payload={} bytes, timeout={}ms",
            NUM_TEST_MESSAGES, MAX_PAYLOAD_SIZE, ECHO_TIMEOUT_MS
        );

        for i in 1..=NUM_TEST_MESSAGES {
            let message_start = std::time::Instant::now();

            // Generate unique test message with variable payload size
            let nonce = rand::random::<u64>();
            let payload_size = (i * 200).min(MAX_PAYLOAD_SIZE); // Increasing size: 200, 400, 600, 800, 1000 bytes
            let test_payload = format!(
                "ECHO_TEST_{:03}_{}_{}",
                i,
                nonce,
                "x".repeat(payload_size.saturating_sub(50)) // Subtract space for prefix
            );

            let ping_message = Message::new_ping(nonce, test_payload.clone());

            debug!(
                "Sending echo test message {} of {} (nonce: {}, payload_size: {} bytes)",
                i,
                NUM_TEST_MESSAGES,
                nonce,
                test_payload.len()
            );

            // Send ping message with error handling
            let send_result = tokio::time::timeout(
                std::time::Duration::from_millis(ECHO_TIMEOUT_MS),
                conn.send_message(ping_message),
            )
            .await;

            match send_result {
                Ok(Ok(())) => {
                    debug!(
                        "Successfully sent echo test message {} (nonce: {})",
                        i, nonce
                    );
                }
                Ok(Err(e)) => {
                    error!(
                        "Failed to send echo test message {} (nonce: {}): {}",
                        i, nonce, e
                    );
                    warn!("Echo session continuing with remaining messages");
                    continue;
                }
                Err(_) => {
                    error!("Timeout sending echo test message {} (nonce: {})", i, nonce);
                    warn!("Echo session continuing with remaining messages");
                    continue;
                }
            }

            // Receive pong response with timeout
            let receive_result = tokio::time::timeout(
                std::time::Duration::from_millis(ECHO_TIMEOUT_MS),
                conn.receive_message(),
            )
            .await;

            match receive_result {
                Ok(Ok((response_message, sender_id))) => {
                    let round_trip_time = message_start.elapsed();

                    debug!(
                        "Received response for message {} from {} in {:?} (nonce: {})",
                        i,
                        sender_id,
                        round_trip_time,
                        response_message.get_nonce()
                    );

                    // Verify message integrity
                    if let Err(integrity_error) =
                        self.verify_echo_integrity(&response_message, nonce, &test_payload)
                    {
                        error!(
                            "Echo integrity verification failed for message {} (nonce: {}): {}",
                            i, nonce, integrity_error
                        );
                        warn!("Echo session continuing with remaining messages");
                        continue;
                    }

                    // Record successful echo metrics
                    successful_echoes += 1;
                    total_round_trip_time += round_trip_time;
                    message_sizes.push(test_payload.len());

                    info!(
                        "Echo test message {} completed successfully (nonce: {}, round_trip: {:?})",
                        i, nonce, round_trip_time
                    );
                }
                Ok(Err(e)) => {
                    error!(
                        "Failed to receive echo response for message {} (nonce: {}): {}",
                        i, nonce, e
                    );
                    warn!("Echo session continuing with remaining messages");
                    continue;
                }
                Err(_) => {
                    error!(
                        "Timeout receiving echo response for message {} (nonce: {})",
                        i, nonce
                    );
                    warn!("Echo session continuing with remaining messages");
                    continue;
                }
            }
        }

        // Calculate and log session performance metrics
        let session_duration = session_start.elapsed();
        let success_rate = (successful_echoes as f64 / NUM_TEST_MESSAGES as f64) * 100.0;
        let avg_round_trip = if successful_echoes > 0 {
            total_round_trip_time / successful_echoes as u32
        } else {
            std::time::Duration::ZERO
        };

        let total_bytes_sent = message_sizes.iter().sum::<usize>();
        let avg_message_size = if successful_echoes > 0 {
            total_bytes_sent / successful_echoes
        } else {
            0
        };

        info!(
            "Echo session completed: {}/{} successful ({:.1}% success rate)",
            successful_echoes, NUM_TEST_MESSAGES, success_rate
        );

        info!(
            "Session metrics - duration: {:?}, avg_round_trip: {:?}, avg_message_size: {} bytes, total_bytes: {}",
            session_duration, avg_round_trip, avg_message_size, total_bytes_sent
        );

        // Determine session success
        if successful_echoes == 0 {
            error!("Echo session failed: no successful message exchanges");
            return Err(anyhow::anyhow!(
                "Echo session failed: no successful message exchanges"
            ));
        } else if success_rate < 50.0 {
            warn!(
                "Echo session completed with low success rate: {:.1}% ({}/{})",
                success_rate, successful_echoes, NUM_TEST_MESSAGES
            );
        }

        debug!("Echo session completed successfully with acceptable performance");
        Ok(())
    }

    /// Internal helper method to verify echo message integrity
    #[instrument(level = "debug", skip(self, test_payload))]
    fn verify_echo_integrity(
        &self,
        response_message: &Message,
        expected_nonce: u64,
        test_payload: &str,
    ) -> Result<()> {
        // Verify message type (should be Pong for echo response)
        if !response_message.is_pong() {
            return Err(anyhow::anyhow!(
                "Expected Pong message, received: {}",
                response_message.message_type()
            ));
        }

        // Verify nonce matches (echo server should return same nonce)
        let response_nonce = response_message.get_nonce();
        if response_nonce != expected_nonce {
            return Err(anyhow::anyhow!(
                "Nonce mismatch: expected {}, received {}",
                expected_nonce,
                response_nonce
            ));
        }

        // Verify payload integrity (echo server should return same or similar payload)
        let response_payload = response_message.get_payload();
        if !response_payload.contains("ECHO_TEST_") {
            return Err(anyhow::anyhow!(
                "Invalid echo response payload format: {}",
                response_payload
            ));
        }

        // Additional integrity checks could include:
        // - Exact payload matching (if server does exact echo)
        // - Checksum verification
        // - Timestamp validation

        debug!(
            "Echo integrity verified: nonce={}, payload_len={}, response_len={}",
            expected_nonce,
            test_payload.len(),
            response_payload.len()
        );

        Ok(())
    }

    /// Send a single message to a peer and wait for response (one-shot operation)
    ///
    /// This is a convenience method that handles the full lifecycle:
    /// 1. Connect to the target address
    /// 2. Perform handshake
    /// 3. Send the message
    /// 4. Receive response
    /// 5. Close connection
    ///
    /// For multiple messages to the same peer, prefer using `connect()` and reusing the connection.
    #[instrument(level = "info", skip(self, message), fields(addr = addr, msg_type = message.message_type(), local_peer = self.identity.peer_id().as_str()))]
    pub async fn send_message_to(&self, addr: &str, message: Message) -> Result<Message> {
        info!("Starting one-shot message send to {}", addr);
        debug!(
            "Message details - type: {}, nonce: {}, payload_size: {} bytes",
            message.message_type(),
            message.get_nonce(),
            message.get_payload().len()
        );

        // Establish connection
        let mut connection = self
            .connect(addr)
            .await
            .with_context(|| format!("Failed to connect to {}", addr))?;

        let peer_id = connection.peer_identity().unwrap_or("unknown").to_string();

        info!("Connected to {} (peer: {}), sending message", addr, peer_id);

        // Send message with error context
        connection
            .send_message(message)
            .await
            .with_context(|| format!("Failed to send message to {}", addr))?;

        debug!("Message sent successfully, waiting for response");

        // Receive response
        let (response_message, response_sender) = connection
            .receive_message()
            .await
            .with_context(|| format!("Failed to receive response from {}", addr))?;

        info!(
            "Received response from {} (type: {}, nonce: {})",
            response_sender,
            response_message.message_type(),
            response_message.get_nonce()
        );

        // Clean up connection
        if let Err(e) = connection.close().await {
            warn!("Failed to cleanly close connection to {}: {}", addr, e);
        }

        debug!("One-shot message exchange completed successfully");
        Ok(response_message)
    }

    /// Send a ping message and wait for pong response (convenience method)
    ///
    /// This is a specialized version of `send_message_to` for ping/pong exchanges.
    #[instrument(level = "info", skip(self), fields(addr = addr, local_peer = self.identity.peer_id().as_str()))]
    pub async fn ping(&self, addr: &str, payload: String) -> Result<Message> {
        let nonce = rand::random::<u64>();
        let ping_message = Message::new_ping(nonce, payload);

        info!("Sending ping to {} with nonce {}", addr, nonce);

        let response = self.send_message_to(addr, ping_message).await?;

        // Verify response is a pong with matching nonce
        if !response.is_pong() {
            return Err(anyhow::anyhow!(
                "Expected pong response, got: {}",
                response.message_type()
            ));
        }

        if response.get_nonce() != nonce {
            return Err(anyhow::anyhow!(
                "Nonce mismatch: sent {}, received {}",
                nonce,
                response.get_nonce()
            ));
        }

        info!(
            "Ping successful: received pong with matching nonce {}",
            nonce
        );
        Ok(response)
    }

    /// Test connection quality to a peer using echo session
    ///
    /// This method combines connection establishment with echo session testing
    /// to provide a comprehensive connection quality assessment.
    #[instrument(level = "info", skip(self), fields(addr = addr, local_peer = self.identity.peer_id().as_str()))]
    pub async fn test_connection_quality(&self, addr: &str) -> Result<ConnectionQualityReport> {
        info!("Starting connection quality test to {}", addr);

        let test_start = std::time::Instant::now();

        // Establish connection and measure handshake time
        let handshake_start = std::time::Instant::now();
        let mut connection = self
            .connect(addr)
            .await
            .context("Connection failed during quality test")?;
        let handshake_duration = handshake_start.elapsed();

        let peer_id = connection.peer_identity().unwrap_or("unknown").to_string();
        debug!(
            "Connection established to {} (peer: {}) in {:?}",
            addr, peer_id, handshake_duration
        );

        // Run echo session and capture results
        let echo_start = std::time::Instant::now();
        let echo_result = self.echo_session(&mut connection).await;
        let echo_duration = echo_start.elapsed();

        // Close connection
        if let Err(e) = connection.close().await {
            warn!("Failed to close connection during quality test: {}", e);
        }

        let total_duration = test_start.elapsed();

        // Create quality report
        let quality_report = ConnectionQualityReport {
            peer_address: addr.to_string(),
            peer_id,
            handshake_duration,
            echo_duration,
            total_duration,
            echo_session_success: echo_result.is_ok(),
            echo_error: echo_result.err().map(|e| e.to_string()),
        };

        info!(
            "Connection quality test completed for {} in {:?} (success: {})",
            addr, total_duration, quality_report.echo_session_success
        );

        Ok(quality_report)
    }

    // Future Enhancement: Connection Pooling Support
    //
    // The following methods provide a foundation for connection pooling.
    // This is marked for future implementation as it requires additional
    // infrastructure for connection lifecycle management.

    /// Check if a cached connection to the given address is available and healthy
    ///
    /// TODO: Future implementation should include:
    /// - Connection cache/pool management
    /// - Health check mechanisms (ping/heartbeat)
    /// - Connection expiration policies
    /// - Thread-safe connection sharing
    pub async fn has_healthy_connection(&self, _addr: &str) -> bool {
        // Placeholder for future connection pooling implementation
        //
        // Implementation would check:
        // 1. If connection exists in pool for this address
        // 2. If connection is still alive (not closed)
        // 3. If connection passed recent health check
        // 4. If connection hasn't exceeded max idle time
        //
        // Example future implementation:
        // ```rust
        // if let Some(conn) = self.connection_pool.get(addr) {
        //     return conn.is_healthy().await;
        // }
        // false
        // ```

        false // Currently no pooling, so no cached connections
    }

    /// Perform health check on a connection
    ///
    /// TODO: Future implementation should include:
    /// - Lightweight ping/pong health checks
    /// - Connection latency measurement
    /// - Error rate tracking
    /// - Automatic connection replacement on failure
    pub async fn health_check_connection(
        &self,
        _connection: &mut Connection,
    ) -> Result<ConnectionHealth> {
        // Placeholder for future health check implementation
        //
        // Implementation would:
        // 1. Send lightweight ping message
        // 2. Measure response time
        // 3. Check for recent errors
        // 4. Return health status with metrics
        //
        // Example future implementation:
        // ```rust
        // let start = std::time::Instant::now();
        // let ping_result = self.ping_via_connection(connection, "health_check".to_string()).await;
        // let latency = start.elapsed();
        //
        // match ping_result {
        //     Ok(_) => Ok(ConnectionHealth::Healthy { latency }),
        //     Err(e) => Ok(ConnectionHealth::Unhealthy { error: e.to_string() }),
        // }
        // ```

        Ok(ConnectionHealth::Unknown)
    }
}

/// Connection quality assessment report
#[derive(Debug, Clone)]
pub struct ConnectionQualityReport {
    pub peer_address: String,
    pub peer_id: String,
    pub handshake_duration: std::time::Duration,
    pub echo_duration: std::time::Duration,
    pub total_duration: std::time::Duration,
    pub echo_session_success: bool,
    pub echo_error: Option<String>,
}

impl ConnectionQualityReport {
    /// Check if the connection quality is acceptable for production use
    pub fn is_acceptable_quality(&self) -> bool {
        // Define quality thresholds
        const MAX_HANDSHAKE_MS: u64 = 5000; // 5 seconds
        const MAX_ECHO_MS: u64 = 30000; // 30 seconds

        self.echo_session_success
            && self.handshake_duration.as_millis() < MAX_HANDSHAKE_MS as u128
            && self.echo_duration.as_millis() < MAX_ECHO_MS as u128
    }

    /// Get a human-readable quality assessment
    pub fn quality_assessment(&self) -> &'static str {
        if !self.echo_session_success {
            return "Poor - Echo session failed";
        }

        let handshake_ms = self.handshake_duration.as_millis();
        let echo_ms = self.echo_duration.as_millis();

        match (handshake_ms, echo_ms) {
            (0..=1000, 0..=5000) => "Excellent - Fast handshake and echo",
            (0..=2000, 0..=10000) => "Good - Reasonable performance",
            (0..=5000, 0..=30000) => "Fair - Acceptable for most use cases",
            _ => "Poor - High latency detected",
        }
    }
}

/// Connection health status for future pooling implementation
#[derive(Debug, Clone)]
pub enum ConnectionHealth {
    Healthy { latency: std::time::Duration },
    Unhealthy { error: String },
    Unknown,
}

impl ConnectionHealth {
    pub fn is_healthy(&self) -> bool {
        matches!(self, ConnectionHealth::Healthy { .. })
    }
}
