use crate::crypto::Identity;
use crate::messages::chess::{GameAccept, GameInvite, Move as ChessMove};
use crate::messages::types::Message;
use crate::messages::{FailureClass, RetryStrategy};
use crate::network::{Client, Connection};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Configuration for network operations
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Default retry strategy for network operations
    pub default_retry_strategy: RetryStrategy,
    /// Connection timeout duration
    pub connection_timeout: Duration,
    /// Maximum number of persistent connections to maintain
    pub max_persistent_connections: usize,
    /// How long to keep connections alive when idle
    pub connection_keepalive: Duration,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            default_retry_strategy: RetryStrategy::NoRetry,
            connection_timeout: Duration::from_secs(10),
            max_persistent_connections: 10,
            connection_keepalive: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Connection state tracking
#[derive(Debug, Clone)]
struct ConnectionInfo {
    /// When this connection was last used
    last_used: Instant,
    /// Whether the connection is currently healthy
    is_healthy: bool,
    /// Number of consecutive failures
    failure_count: u32,
}

/// Network manager for chess game operations
pub struct NetworkManager {
    /// Client for creating new connections
    client: Client,
    /// Configuration for network operations
    config: NetworkConfig,
    /// Active persistent connections
    connections: Arc<Mutex<HashMap<String, (Connection, ConnectionInfo)>>>,
    /// Pending messages for offline peers
    pending_messages: Arc<Mutex<HashMap<String, Vec<PendingMessage>>>>,
}

/// A message waiting to be sent when peer comes online
#[derive(Debug, Clone)]
struct PendingMessage {
    /// The message to send
    message: Message,
    /// When this message was created
    created_at: Instant,
    /// Number of attempts made to send
    attempts: u32,
    /// Game ID this message belongs to
    game_id: String,
}

impl NetworkManager {
    /// Create a new network manager
    pub fn new(identity: Arc<Identity>) -> Self {
        let client = Client::new(identity);
        Self {
            client,
            config: NetworkConfig::default(),
            connections: Arc::new(Mutex::new(HashMap::new())),
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a network manager with custom configuration
    pub fn with_config(identity: Arc<Identity>, config: NetworkConfig) -> Self {
        let client = Client::new(identity);
        Self {
            client,
            config,
            connections: Arc::new(Mutex::new(HashMap::new())),
            pending_messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Send a game invitation with retry logic
    pub async fn send_game_invite(
        &self,
        peer_address: &str,
        game_id: String,
        invite: GameInvite,
    ) -> Result<Message> {
        let message = Message::new_game_invite(game_id.clone(), invite.suggested_color);

        match self
            .send_message_with_retry(peer_address, message.clone(), &game_id)
            .await
        {
            Ok(response) => {
                info!("Game invitation sent successfully to {}", peer_address);
                Ok(response)
            }
            Err(e) => {
                warn!("Failed to send game invitation to {}: {}", peer_address, e);
                // Store as pending message for when peer comes online
                if let Err(store_err) = self
                    .store_pending_message(peer_address, message, game_id)
                    .await
                {
                    warn!("Failed to store pending message: {}", store_err);
                }
                Err(e)
            }
        }
    }

    /// Send a game acceptance with retry logic
    pub async fn send_game_accept(
        &self,
        peer_address: &str,
        game_id: String,
        accept: GameAccept,
    ) -> Result<Message> {
        let message = Message::new_game_accept(game_id.clone(), accept.accepted_color);

        match self
            .send_message_with_retry(peer_address, message.clone(), &game_id)
            .await
        {
            Ok(response) => {
                info!("Game acceptance sent successfully to {}", peer_address);
                Ok(response)
            }
            Err(e) => {
                warn!("Failed to send game acceptance to {}: {}", peer_address, e);
                // Store as pending message for when peer comes online
                self.store_pending_message(peer_address, message, game_id)
                    .await?;
                Err(e)
            }
        }
    }

    /// Send a chess move with retry logic
    pub async fn send_chess_move(
        &self,
        peer_address: &str,
        game_id: String,
        chess_move: ChessMove,
    ) -> Result<Message> {
        let message = Message::new_move(
            game_id.clone(),
            chess_move.chess_move.clone(),
            chess_move.board_state_hash.clone(),
        );

        match self
            .send_message_with_retry(peer_address, message.clone(), &game_id)
            .await
        {
            Ok(response) => {
                info!("Chess move sent successfully to {}", peer_address);
                Ok(response)
            }
            Err(e) => {
                warn!("Failed to send chess move to {}: {}", peer_address, e);
                // Store as pending message for when peer comes online
                self.store_pending_message(peer_address, message, game_id)
                    .await?;
                Err(e)
            }
        }
    }

    /// Send a message with retry logic and connection management
    async fn send_message_with_retry(
        &self,
        peer_address: &str,
        message: Message,
        game_id: &str,
    ) -> Result<Message> {
        // Determine retry strategy based on operation type
        let operation = self.classify_operation(&message);
        let retry_strategy = RetryStrategy::for_cli_operation(&operation);

        self.send_message_with_strategy(peer_address, message, game_id, retry_strategy)
            .await
    }

    /// Send a message with a specific retry strategy
    async fn send_message_with_strategy(
        &self,
        peer_address: &str,
        message: Message,
        _game_id: &str,
        strategy: RetryStrategy,
    ) -> Result<Message> {
        let max_attempts = strategy.max_attempts();
        let base_delay = strategy.base_delay();
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            debug!(
                "Attempting to send message to {} (attempt {}/{}, strategy: {:?})",
                peer_address, attempt, max_attempts, strategy
            );

            // Try to get or create a connection
            match self
                .get_or_create_connection_with_strategy(peer_address, strategy)
                .await
            {
                Ok(mut connection) => {
                    // Send the message
                    match connection.send_message(message.clone()).await {
                        Ok(()) => {
                            // Now receive the response
                            match connection.receive_message().await {
                                Ok((response, _sender)) => {
                                    // Update connection as healthy
                                    self.update_connection_health(peer_address, true).await;
                                    return Ok(response);
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to receive response from {} (attempt {}): {}",
                                        peer_address, attempt, e
                                    );
                                    let failure_class =
                                        FailureClass::classify_error(&anyhow::anyhow!("{}", e));
                                    if failure_class == FailureClass::NoRetry {
                                        return Err(anyhow::anyhow!("Receive failed: {}", e));
                                    }
                                    last_error = Some(anyhow::anyhow!("Receive failed: {}", e));
                                    // Mark connection as unhealthy
                                    self.update_connection_health(peer_address, false).await;
                                    // Remove the failed connection
                                    self.remove_connection(peer_address).await;
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to send message to {} (attempt {}): {}",
                                peer_address, attempt, e
                            );
                            let failure_class =
                                FailureClass::classify_error(&anyhow::anyhow!("{}", e));
                            if failure_class == FailureClass::NoRetry {
                                return Err(anyhow::anyhow!("Send failed: {}", e));
                            }
                            last_error = Some(anyhow::anyhow!("Send failed: {}", e));
                            // Mark connection as unhealthy
                            self.update_connection_health(peer_address, false).await;
                            // Remove the failed connection
                            self.remove_connection(peer_address).await;
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Failed to establish connection to {} (attempt {}): {}",
                        peer_address, attempt, e
                    );
                    let failure_class = FailureClass::classify_error(&e);
                    if failure_class == FailureClass::NoRetry {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }

            // Wait before retrying (except on last attempt)
            if attempt < max_attempts && base_delay > Duration::from_millis(0) {
                let delay = self.calculate_retry_delay_for_strategy(attempt, strategy);
                debug!("Waiting {}ms before retry", delay.as_millis());
                tokio::time::sleep(delay).await;
            }
        }

        // All attempts failed
        let final_error = last_error.unwrap_or_else(|| {
            anyhow::anyhow!("Failed to send message after {} attempts", max_attempts)
        });

        error!(
            "All retry attempts failed for {} (strategy: {:?}): {}",
            peer_address, strategy, final_error
        );
        Err(final_error)
    }

    /// Get an existing healthy connection or create a new one
    async fn get_or_create_connection(&self, peer_address: &str) -> Result<Connection> {
        self.get_or_create_connection_with_strategy(
            peer_address,
            self.config.default_retry_strategy,
        )
        .await
    }

    /// Get an existing healthy connection or create a new one with a specific retry strategy
    async fn get_or_create_connection_with_strategy(
        &self,
        peer_address: &str,
        strategy: RetryStrategy,
    ) -> Result<Connection> {
        // Create a new connection each time since we can't clone connections
        debug!(
            "Creating new connection to {} (strategy: {:?})",
            peer_address, strategy
        );
        let connection = tokio::time::timeout(
            self.config.connection_timeout,
            self.client.connect_with_strategy(peer_address, strategy),
        )
        .await
        .context("Connection timeout")?
        .with_context(|| format!("Failed to connect to {}", peer_address))?;

        Ok(connection)
    }

    /// Update the health status of a connection
    async fn update_connection_health(&self, peer_address: &str, is_healthy: bool) {
        let mut connections = self.connections.lock().await;
        if let Some((_, info)) = connections.get_mut(peer_address) {
            info.is_healthy = is_healthy;
            if is_healthy {
                info.failure_count = 0;
            } else {
                info.failure_count += 1;
            }
            info.last_used = Instant::now();
        }
    }

    /// Remove a connection from the pool
    async fn remove_connection(&self, peer_address: &str) {
        let mut connections = self.connections.lock().await;
        if let Some((mut connection, _)) = connections.remove(peer_address) {
            debug!("Removing connection to {}", peer_address);
            if let Err(e) = connection.close().await {
                warn!("Error closing connection to {}: {}", peer_address, e);
            }
        }
    }

    /// Store a message for sending when peer comes online
    async fn store_pending_message(
        &self,
        peer_address: &str,
        message: Message,
        game_id: String,
    ) -> Result<()> {
        let game_id_for_log = game_id.clone(); // Keep a copy for logging
        let pending_message = PendingMessage {
            message,
            created_at: Instant::now(),
            attempts: 0,
            game_id,
        };

        let mut pending = self.pending_messages.lock().await;
        pending
            .entry(peer_address.to_string())
            .or_insert_with(Vec::new)
            .push(pending_message);

        info!(
            "Stored pending message for {} (game: {})",
            peer_address, game_id_for_log
        );
        Ok(())
    }

    /// Attempt to send all pending messages for a peer
    pub async fn send_pending_messages(&self, peer_address: &str) -> Result<u32> {
        let mut sent_count = 0;

        // Get pending messages for this peer
        let messages = {
            let mut pending = self.pending_messages.lock().await;
            pending.remove(peer_address).unwrap_or_default()
        };

        if messages.is_empty() {
            return Ok(0);
        }

        let messages_count = messages.len();
        info!(
            "Attempting to send {} pending messages to {}",
            messages_count, peer_address
        );

        for mut pending_msg in messages {
            pending_msg.attempts += 1;

            match self
                .send_message_with_retry(
                    peer_address,
                    pending_msg.message.clone(),
                    &pending_msg.game_id,
                )
                .await
            {
                Ok(_) => {
                    sent_count += 1;
                    info!(
                        "Successfully sent pending message for game {}",
                        pending_msg.game_id
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to send pending message for game {} (attempt {}): {}",
                        pending_msg.game_id, pending_msg.attempts, e
                    );

                    // If we haven't exceeded max attempts, put it back in pending
                    let max_attempts = self.config.default_retry_strategy.max_attempts();
                    if pending_msg.attempts < max_attempts {
                        let mut pending = self.pending_messages.lock().await;
                        pending
                            .entry(peer_address.to_string())
                            .or_insert_with(Vec::new)
                            .push(pending_msg);
                    } else {
                        error!(
                            "Dropping pending message for game {} after {} attempts",
                            pending_msg.game_id, pending_msg.attempts
                        );
                    }
                }
            }
        }

        info!(
            "Sent {}/{} pending messages to {}",
            sent_count, messages_count, peer_address
        );
        Ok(sent_count)
    }

    /// Check if a peer is currently reachable
    pub async fn is_peer_online(&self, peer_address: &str) -> bool {
        match tokio::time::timeout(
            Duration::from_secs(5),
            self.client.ping(peer_address, "ping".to_string()),
        )
        .await
        {
            Ok(Ok(_)) => {
                debug!("Peer {} is online", peer_address);
                true
            }
            Ok(Err(e)) => {
                debug!("Peer {} is offline: {}", peer_address, e);
                false
            }
            Err(_) => {
                debug!("Peer {} ping timeout", peer_address);
                false
            }
        }
    }

    /// Clean up old connections and pending messages
    pub async fn cleanup_connections(&self) {
        // Clean up old pending messages (older than 1 hour)
        {
            let mut pending = self.pending_messages.lock().await;
            let mut peers_to_clean: Vec<String> = Vec::new();
            let now = Instant::now();

            for (peer, messages) in pending.iter_mut() {
                messages
                    .retain(|msg| now.duration_since(msg.created_at) < Duration::from_secs(3600));
                if messages.is_empty() {
                    peers_to_clean.push(peer.clone());
                }
            }

            for peer in peers_to_clean {
                pending.remove(&peer);
            }
        }

        debug!("Connection cleanup completed");
    }

    /// Get statistics about network operations
    pub async fn get_network_stats(&self) -> NetworkStats {
        let connections = self.connections.lock().await;
        let pending = self.pending_messages.lock().await;

        let active_connections = connections.len();
        let healthy_connections = connections
            .values()
            .filter(|(_, info)| info.is_healthy)
            .count();
        let total_pending_messages = pending.values().map(|msgs| msgs.len()).sum();

        NetworkStats {
            active_connections,
            healthy_connections,
            total_pending_messages,
        }
    }

    /// Classify a message to determine the appropriate operation type
    fn classify_operation(&self, message: &Message) -> String {
        match message {
            Message::GameInvite(_) => "invite".to_string(),
            Message::GameAccept(_) => "accept".to_string(),
            Message::GameDecline(_) => "decline".to_string(),
            Message::Move(_) => "move".to_string(),
            Message::MoveAck(_) => "move_ack".to_string(),
            Message::SyncRequest(_) => "sync".to_string(),
            Message::SyncResponse(_) => "sync".to_string(),
            Message::Ping { .. } => "ping".to_string(),
            Message::Pong { .. } => "pong".to_string(),
        }
    }

    /// Calculate retry delay for a specific strategy with exponential backoff
    fn calculate_retry_delay_for_strategy(
        &self,
        attempt: u32,
        strategy: RetryStrategy,
    ) -> Duration {
        let base_delay = strategy.base_delay();
        if base_delay == Duration::from_millis(0) {
            return Duration::from_millis(0);
        }

        let delay = base_delay.as_millis() as u64 * (2_u64.pow(attempt - 1));
        let max_delay = Duration::from_secs(30); // Cap at 30 seconds
        std::cmp::min(Duration::from_millis(delay), max_delay)
    }
}

/// Network operation statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub active_connections: usize,
    pub healthy_connections: usize,
    pub total_pending_messages: usize,
}

impl std::fmt::Display for NetworkStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Active connections: {}, Healthy: {}, Pending messages: {}",
            self.active_connections, self.healthy_connections, self.total_pending_messages
        )
    }
}
