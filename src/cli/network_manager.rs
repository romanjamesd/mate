use crate::crypto::Identity;
use crate::messages::chess::{
    ChessProtocolError, GameAccept, GameDecline, GameInvite, Move as ChessMove, MoveAck,
};
use crate::messages::types::Message;
use crate::messages::wire::WireProtocolError;
use crate::messages::{FailureClass, RetryStrategy};
use crate::network::{Client, Connection, ConnectionError};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Signer mismatches stay terminal even through contextual error wrappers.
pub(super) fn is_peer_identity_mismatch(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<ConnectionError>(),
            Some(ConnectionError::PeerIdentityMismatch { .. })
        )
    })
}

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

/// Result of a successful chess send: wire response plus remote peer identity.
#[derive(Debug, Clone)]
pub struct SendOutcome {
    /// Response message from the remote peer
    pub response: Message,
    /// Authenticated peer id from the connection handshake
    pub peer_id: String,
}

/// Application-level response to an acceptance, separate from transport success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameAcceptOutcome {
    Accepted {
        acknowledgement: GameAccept,
        peer_id: String,
    },
    Rejected {
        decline: GameDecline,
        peer_id: String,
    },
}

impl GameAcceptOutcome {
    pub fn peer_id(&self) -> &str {
        match self {
            Self::Accepted { peer_id, .. } | Self::Rejected { peer_id, .. } => peer_id,
        }
    }
}

/// Correlate a received response with the acceptance actually sent.
pub fn classify_accept_response(
    request: &GameAccept,
    outcome: SendOutcome,
) -> Result<GameAcceptOutcome, ChessProtocolError> {
    let SendOutcome { response, peer_id } = outcome;
    match response {
        Message::GameAccept(acknowledgement) if acknowledgement == *request => {
            Ok(GameAcceptOutcome::Accepted {
                acknowledgement,
                peer_id,
            })
        }
        Message::GameDecline(decline) if decline.game_id == request.game_id => {
            Ok(GameAcceptOutcome::Rejected { decline, peer_id })
        }
        response => Err(ChessProtocolError::UnexpectedMessage {
            game_id: request.game_id.clone(),
            expected: format!(
                "GameAccept {request:?} or GameDecline for {}",
                request.game_id
            ),
            received: format!("{response:?}"),
        }),
    }
}

/// A transport success only acknowledges an invite when the full payload matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InviteOutcome {
    Acknowledged {
        acknowledgement: GameInvite,
        peer_id: String,
    },
    Declined {
        decline: GameDecline,
        peer_id: String,
    },
}

pub fn classify_invite_response(
    request: &GameInvite,
    outcome: SendOutcome,
) -> Result<InviteOutcome, ChessProtocolError> {
    let SendOutcome { response, peer_id } = outcome;
    match response {
        Message::GameInvite(acknowledgement) if acknowledgement == *request => {
            Ok(InviteOutcome::Acknowledged {
                acknowledgement,
                peer_id,
            })
        }
        Message::GameDecline(decline) if decline.game_id == request.game_id => {
            Ok(InviteOutcome::Declined { decline, peer_id })
        }
        response => Err(ChessProtocolError::UnexpectedMessage {
            game_id: request.game_id.clone(),
            expected: format!(
                "GameInvite {request:?} or GameDecline for {}",
                request.game_id
            ),
            received: format!("{response:?}"),
        }),
    }
}

/// Application-level response to a move, separate from transport success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveSendOutcome {
    /// The opponent acknowledged exactly the move that was sent.
    Acknowledged { ack: MoveAck, peer_id: String },
    /// The opponent refused the move; nothing was committed remotely.
    Rejected {
        decline: GameDecline,
        peer_id: String,
    },
}

/// Terminal move failures: never retried or committed locally.
#[derive(Debug)]
pub enum MoveProtocolError {
    /// Handshake peer ≠ stored opponent; nothing was sent on that connection.
    PeerMismatch { expected: String, actual: String },
    /// Verified response envelope signer ≠ handshake peer / opponent.
    SignerMismatch { expected: String, actual: String },
    /// Uncorrelated or unexpected response (from `classify_move_response`).
    Response(ChessProtocolError),
    /// Invalid signature, invalid timestamp or a malformed response.
    Connection(ConnectionError),
}

impl fmt::Display for MoveProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PeerMismatch { expected, actual } => write!(
                f,
                "authenticated peer {actual} is not the game opponent {expected}"
            ),
            Self::SignerMismatch { expected, actual } => write!(
                f,
                "move response was signed by {actual}, not the game opponent {expected}"
            ),
            Self::Response(e) => write!(f, "{e}"),
            Self::Connection(e) => write!(f, "invalid move response: {e}"),
        }
    }
}

impl std::error::Error for MoveProtocolError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Response(e) => Some(e),
            Self::Connection(e) => Some(e),
            Self::PeerMismatch { .. } | Self::SignerMismatch { .. } => None,
        }
    }
}

/// Why a move send produced no trustworthy response.
#[derive(Debug)]
pub enum MoveSendError {
    Protocol(MoveProtocolError),
    /// Transport or local send failure. `may_have_been_sent` is false only
    /// when no send was attempted during this operation.
    Delivery {
        may_have_been_sent: bool,
        source: anyhow::Error,
    },
}

impl fmt::Display for MoveSendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(e) => write!(f, "{e}"),
            Self::Delivery { source, .. } => write!(f, "move delivery failed: {source:#}"),
        }
    }
}

impl std::error::Error for MoveSendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(e) => Some(e),
            Self::Delivery { source, .. } => Some(source.as_ref()),
        }
    }
}

/// Correlate a received response with the exact move sent to `expected_peer`.
pub fn classify_move_response(
    request: &ChessMove,
    expected_peer: &str,
    outcome: SendOutcome,
) -> Result<MoveSendOutcome, ChessProtocolError> {
    let SendOutcome { response, peer_id } = outcome;
    if expected_peer.is_empty() || peer_id != expected_peer {
        return Err(ChessProtocolError::SecurityViolation {
            game_id: request.game_id.clone(),
            violation: format!(
                "Move response peer {peer_id} does not match game opponent {expected_peer}"
            ),
        });
    }
    let expected_ack = MoveAck::for_move(request);
    match response {
        Message::MoveAck(ack) if ack == expected_ack => {
            Ok(MoveSendOutcome::Acknowledged { ack, peer_id })
        }
        Message::GameDecline(decline) if decline.game_id == request.game_id => {
            Ok(MoveSendOutcome::Rejected { decline, peer_id })
        }
        response => Err(ChessProtocolError::UnexpectedMessage {
            game_id: request.game_id.clone(),
            expected: format!("{expected_ack:?} or GameDecline for {}", request.game_id),
            received: format!("{response:?}"),
        }),
    }
}

/// Lost or timed-out connections; the same move may be resent on a new one.
fn is_transport_failure(error: &ConnectionError) -> bool {
    match error {
        ConnectionError::WireProtocol(wire) => matches!(
            wire,
            WireProtocolError::Timeout(_)
                | WireProtocolError::ReadTimeout { .. }
                | WireProtocolError::WriteTimeout { .. }
                | WireProtocolError::OperationTimeout { .. }
                | WireProtocolError::UnexpectedEof { .. }
                | WireProtocolError::ConnectionClosed { .. }
                | WireProtocolError::Io(_)
        ),
        ConnectionError::ConnectionClosed | ConnectionError::Io(_) => true,
        ConnectionError::HandshakeFailed { .. }
        | ConnectionError::AuthenticationFailed { .. }
        | ConnectionError::PeerIdentityMismatch { .. }
        | ConnectionError::InvalidSignature
        | ConnectionError::InvalidTimestamp => false,
    }
}

/// One move request/response exchange on an authenticated connection.
enum MoveExchangeFailure {
    /// Retryable on a new connection.
    Transport(ConnectionError),
    /// The move could not be written for a non-transport reason.
    LocalWrite(ConnectionError),
    Protocol(MoveProtocolError),
}

async fn exchange_move(
    connection: &mut Connection,
    message: Message,
    peer_id: &str,
    expected_peer: &str,
) -> Result<Message, MoveExchangeFailure> {
    connection.send_message(message).await.map_err(|e| {
        if is_transport_failure(&e) {
            MoveExchangeFailure::Transport(e)
        } else {
            MoveExchangeFailure::LocalWrite(e)
        }
    })?;
    let (response, signer) = connection.receive_message().await.map_err(|e| {
        if let ConnectionError::PeerIdentityMismatch { expected, actual } = e {
            return MoveExchangeFailure::Protocol(MoveProtocolError::SignerMismatch {
                expected,
                actual,
            });
        }
        if is_transport_failure(&e) {
            MoveExchangeFailure::Transport(e)
        } else {
            MoveExchangeFailure::Protocol(MoveProtocolError::Connection(e))
        }
    })?;
    // A valid signature from any identity other than the opponent is not an answer.
    if signer != peer_id || signer != expected_peer {
        return Err(MoveExchangeFailure::Protocol(
            MoveProtocolError::SignerMismatch {
                expected: expected_peer.to_string(),
                actual: signer,
            },
        ));
    }
    Ok(response)
}

fn authenticated_peer(connection: &Connection) -> Result<String> {
    connection
        .peer_identity()
        .filter(|id| !id.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("Connection has no authenticated peer identity"))
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

    /// Complete the handshake without transmitting a chess payload.
    pub async fn connect_authenticated(&self, address: &str) -> Result<(Connection, String)> {
        let connection = self
            .get_or_create_connection_with_strategy(
                address,
                RetryStrategy::for_cli_operation("invite"),
            )
            .await?;
        let peer_id = authenticated_peer(&connection)?;
        Ok((connection, peer_id))
    }

    /// Send a persisted invitation on the initial connection. Every reconnect
    /// must authenticate as the stored opponent before sending any payload.
    pub async fn send_game_invite_on(
        &self,
        address: &str,
        connection: Connection,
        expected_peer: &str,
        invite: GameInvite,
    ) -> Result<InviteOutcome> {
        let strategy = RetryStrategy::for_cli_operation("invite");
        let mut initial = Some(connection);
        let mut last_error = None;
        for attempt in 1..=strategy.max_attempts() {
            let connected = match initial.take() {
                Some(conn) => Ok(conn),
                None => self
                    .connect_authenticated(address)
                    .await
                    .map(|(conn, _)| conn),
            };
            let result = match connected {
                Ok(mut conn) => {
                    let peer_id = authenticated_peer(&conn)?;
                    if peer_id != expected_peer {
                        anyhow::bail!("Authenticated opponent mismatch: expected {expected_peer}, got {peer_id}");
                    }
                    async {
                        conn.send_message(Message::GameInvite(invite.clone()))
                            .await?;
                        let (response, _) = conn.receive_message().await?;
                        Ok::<_, anyhow::Error>(SendOutcome { response, peer_id })
                    }
                    .await
                }
                Err(e) => Err(e),
            };
            match result {
                Ok(outcome) => return Ok(classify_invite_response(&invite, outcome)?),
                Err(e) => {
                    if is_peer_identity_mismatch(&e)
                        || FailureClass::classify_error(&e) == FailureClass::NoRetry
                    {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
            if attempt < strategy.max_attempts() {
                tokio::time::sleep(self.calculate_retry_delay_for_strategy(attempt, strategy))
                    .await;
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Invitation retries exhausted")))
    }

    /// Send a game acceptance with retry logic
    pub async fn send_game_accept(
        &self,
        peer_address: &str,
        game_id: String,
        accept: GameAccept,
    ) -> Result<GameAcceptOutcome> {
        if game_id != accept.game_id {
            return Err(ChessProtocolError::UnexpectedMessage {
                game_id,
                expected: "acceptance payload with the requested game ID".to_string(),
                received: format!("{accept:?}"),
            }
            .into());
        }
        let message = Message::GameAccept(accept.clone());

        match self
            .send_message_with_retry(peer_address, message.clone(), &game_id)
            .await
        {
            Ok(outcome) => {
                // Received rejections and protocol errors must never be queued for retry.
                Ok(classify_accept_response(&accept, outcome)?)
            }
            Err(e) => {
                warn!("Failed to send game acceptance to {}: {}", peer_address, e);
                if is_peer_identity_mismatch(&e) {
                    return Err(e);
                }
                // Store as pending message for when peer comes online
                self.store_pending_message(peer_address, message, game_id)
                    .await?;
                Err(e)
            }
        }
    }

    /// Send the complete prepared move and classify the opponent's answer.
    ///
    /// Moves are never queued: every failure is returned, and re-running the
    /// same move resends the identical payload, which the opponent handles
    /// idempotently. Transport failures are retried on a new connection that
    /// must again authenticate as `expected_peer` before anything is sent.
    pub async fn send_chess_move(
        &self,
        peer_address: &str,
        expected_peer: &str,
        chess_move: ChessMove,
    ) -> Result<MoveSendOutcome, MoveSendError> {
        let strategy = RetryStrategy::for_cli_operation("move");
        let message = Message::Move(chess_move.clone());
        let mut may_have_been_sent = false;
        let mut last_error = None;

        for attempt in 1..=strategy.max_attempts() {
            let retry = match self
                .get_or_create_connection_with_strategy(peer_address, strategy)
                .await
            {
                Ok(mut connection) => {
                    let peer_id = connection.peer_identity().unwrap_or_default().to_string();
                    if expected_peer.is_empty() || peer_id != expected_peer {
                        return Err(MoveSendError::Protocol(MoveProtocolError::PeerMismatch {
                            expected: expected_peer.to_string(),
                            actual: peer_id,
                        }));
                    }
                    // A failed write may still have transmitted part or all of the frame.
                    may_have_been_sent = true;
                    match exchange_move(&mut connection, message.clone(), &peer_id, expected_peer)
                        .await
                    {
                        Ok(response) => {
                            let outcome = classify_move_response(
                                &chess_move,
                                expected_peer,
                                SendOutcome { response, peer_id },
                            )
                            .map_err(|e| MoveSendError::Protocol(MoveProtocolError::Response(e)))?;
                            info!("Move response from {}: {:?}", peer_address, outcome);
                            return Ok(outcome);
                        }
                        Err(MoveExchangeFailure::Protocol(e)) => {
                            return Err(MoveSendError::Protocol(e));
                        }
                        Err(MoveExchangeFailure::LocalWrite(e)) => {
                            return Err(MoveSendError::Delivery {
                                may_have_been_sent,
                                source: e.into(),
                            });
                        }
                        Err(MoveExchangeFailure::Transport(e)) => {
                            warn!(
                                "Move transport to {} failed (attempt {}): {}",
                                peer_address, attempt, e
                            );
                            last_error = Some(anyhow::Error::from(e));
                            true
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to connect to {} for move (attempt {}): {:#}",
                        peer_address, attempt, e
                    );
                    let retry = FailureClass::classify_error(&e) != FailureClass::NoRetry;
                    last_error = Some(e);
                    retry
                }
            };

            if !retry {
                break;
            }
            if attempt < strategy.max_attempts() {
                tokio::time::sleep(self.calculate_retry_delay_for_strategy(attempt, strategy))
                    .await;
            }
        }

        Err(MoveSendError::Delivery {
            may_have_been_sent,
            source: last_error.unwrap_or_else(|| anyhow::anyhow!("Move send attempts exhausted")),
        })
    }

    /// Send a message with retry logic and connection management
    async fn send_message_with_retry(
        &self,
        peer_address: &str,
        message: Message,
        game_id: &str,
    ) -> Result<SendOutcome> {
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
    ) -> Result<SendOutcome> {
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
                    let peer_id = authenticated_peer(&connection)?;
                    // Send the message
                    match connection.send_message(message.clone()).await {
                        Ok(()) => {
                            // Now receive the response
                            match connection.receive_message().await {
                                Ok((response, _sender)) => {
                                    // Update connection as healthy
                                    self.update_connection_health(peer_address, true).await;
                                    return Ok(SendOutcome { response, peer_id });
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to receive response from {} (attempt {}): {}",
                                        peer_address, attempt, e
                                    );
                                    // Mark connection as unhealthy
                                    self.update_connection_health(peer_address, false).await;
                                    // Remove the failed connection
                                    self.remove_connection(peer_address).await;
                                    let mismatch =
                                        matches!(e, ConnectionError::PeerIdentityMismatch { .. });
                                    let error = anyhow::Error::from(e).context("Receive failed");
                                    if mismatch
                                        || FailureClass::classify_error(&error)
                                            == FailureClass::NoRetry
                                    {
                                        return Err(error);
                                    }
                                    last_error = Some(error);
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
        .with_context(|| format!("Failed to connect to {peer_address}"))?;

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
            if matches!(pending_msg.message, Message::Move(_)) {
                // Moves are never queued; resending one outside `mate move` could
                // reach the opponent without any local commit following it.
                warn!(
                    "Dropping queued move for game {} without sending it",
                    pending_msg.game_id
                );
                continue;
            }
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
                    if !is_peer_identity_mismatch(&e) && pending_msg.attempts < max_attempts {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::chess::generate_game_id;

    #[test]
    fn mismatch_detection_uses_typed_sources_through_contexts() {
        for (expected, actual) in [
            ("expected", "actual"),
            ("authentication", "protocol identity handshake"),
            ("timeout", "connection closed"),
        ] {
            let error = anyhow::Error::from(ConnectionError::PeerIdentityMismatch {
                expected: expected.into(),
                actual: actual.into(),
            })
            .context("inner")
            .context("outer");
            assert!(is_peer_identity_mismatch(&error));
        }
        assert!(!is_peer_identity_mismatch(&anyhow::anyhow!(
            "Message signer actual does not match connection peer expected"
        )));
    }

    #[tokio::test]
    async fn decline_mismatch_is_terminal_and_pending_decline_is_not_requeued() {
        use crate::messages::{wire::FramedMessage, SignedEnvelope};
        use tokio::net::TcpListener;

        for queued in [false, true] {
            let manager = NetworkManager::with_config(
                Arc::new(Identity::generate().unwrap()),
                NetworkConfig {
                    default_retry_strategy: RetryStrategy::Normal,
                    ..NetworkConfig::default()
                },
            );
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap().to_string();
            let peer = Identity::generate().unwrap();
            let other = Identity::generate().unwrap();
            let game_id = generate_game_id();
            let message = Message::GameDecline(GameDecline::new_no_reason(game_id.clone()));
            if queued {
                manager
                    .store_pending_message(&address, message.clone(), game_id.clone())
                    .await
                    .unwrap();
            }
            let operation = async {
                if queued {
                    assert_eq!(manager.send_pending_messages(&address).await.unwrap(), 0);
                } else {
                    let error = manager
                        .send_message_with_retry(&address, message.clone(), &game_id)
                        .await
                        .unwrap_err();
                    assert!(is_peer_identity_mismatch(&error));
                    assert!(matches!(
                        crate::cli::error_handler::cli_error_from_anyhow(error),
                        crate::cli::error_handler::CliError::Connection(
                            ConnectionError::PeerIdentityMismatch { .. }
                        )
                    ));
                }
                let stats = manager.get_network_stats().await;
                assert_eq!(stats.total_pending_messages, 0);
                assert_eq!(stats.healthy_connections, 0);
            };
            let remote = async {
                let (mut stream, _) = listener.accept().await.unwrap();
                let framed = FramedMessage::default();
                let hello = framed
                    .read_message(&mut stream)
                    .await
                    .unwrap()
                    .get_message()
                    .unwrap();
                let response = Message::new_pong(
                    hello.get_nonce(),
                    format!("HANDSHAKE_RESPONSE:{}", peer.peer_id()),
                );
                framed
                    .write_message(
                        &mut stream,
                        &SignedEnvelope::create(&response, &peer, None).unwrap(),
                    )
                    .await
                    .unwrap();
                let request = framed
                    .read_message(&mut stream)
                    .await
                    .unwrap()
                    .get_message()
                    .unwrap();
                assert_eq!(
                    serde_json::to_value(request).unwrap(),
                    serde_json::to_value(&message).unwrap()
                );
                framed
                    .write_message(
                        &mut stream,
                        &SignedEnvelope::create(&message, &other, None).unwrap(),
                    )
                    .await
                    .unwrap();
                assert!(
                    framed.read_message(&mut stream).await.is_err(),
                    "failed connection must be dropped"
                );
            };
            tokio::time::timeout(Duration::from_secs(5), async {
                tokio::join!(operation, remote)
            })
            .await
            .unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err(),
                "mismatch must not reconnect"
            );
        }
    }

    #[test]
    fn peer_identity_mismatch_is_not_a_transport_failure_regardless_of_peer_ids() {
        // Peer IDs are diagnostics, never input to the Move retry policy.
        for (expected, actual) in [
            ("expected-peer", "actual-peer"),
            ("connection closed", "timeout"),
            ("authentication", "protocol identity handshake"),
        ] {
            assert!(!is_transport_failure(
                &ConnectionError::PeerIdentityMismatch {
                    expected: expected.to_string(),
                    actual: actual.to_string(),
                }
            ));
        }
    }

    #[tokio::test]
    async fn queued_moves_are_dropped_without_sending() {
        let manager = NetworkManager::new(Arc::new(Identity::generate().unwrap()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let mv = ChessMove::new(generate_game_id(), 1, "e2e4".into(), "a".repeat(64));
        manager
            .store_pending_message(&address, Message::Move(mv.clone()), mv.game_id.clone())
            .await
            .unwrap();
        assert_eq!(manager.get_network_stats().await.total_pending_messages, 1);

        assert_eq!(manager.send_pending_messages(&address).await.unwrap(), 0);

        assert_eq!(manager.get_network_stats().await.total_pending_messages, 0);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), listener.accept())
                .await
                .is_err(),
            "a queued move must never be transmitted"
        );
    }
}
