use crate::crypto::Identity;
use crate::messages::{Message, SignedEnvelope};
use crate::messages::wire::{FramedMessage, WireConfig, WireProtocolError};
use tokio::net::TcpStream;
use anyhow::{Result, Context};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, warn, info, instrument};
use thiserror::Error;
use rand;

#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Wire protocol error: {0}")]
    WireProtocol(#[from] WireProtocolError),
    #[error("Handshake failed: {reason}")]
    HandshakeFailed { reason: String },
    #[error("Authentication failed for peer: {peer_id}")]
    AuthenticationFailed { peer_id: String },
    #[error("Connection closed unexpectedly")]
    ConnectionClosed,
    #[error("Invalid signature in received message")]
    InvalidSignature,
    #[error("Message timestamp validation failed")]
    InvalidTimestamp,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct Connection {
    stream: TcpStream,
    peer_id: Option<String>,
    identity: Arc<Identity>,
    framed_message: FramedMessage,
}

impl Connection {
    pub async fn new(stream: TcpStream, identity: Arc<Identity>) -> Self {
        info!("Creating new connection with default network configuration");
        
        // Initialize FramedMessage with network-optimized default configuration (Step 5.1)
        let framed_message = FramedMessage::for_network();
        
        debug!("Connection initialized with peer address: {:?}, using network-optimized config", 
               stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap()));
        
        Self {
            stream,
            peer_id: None, // Will be set during handshake
            identity,
            framed_message,
        }
    }
    
    /// Create a new Connection with custom WireConfig for advanced configuration
    pub async fn new_with_config(
        stream: TcpStream, 
        identity: Arc<Identity>, 
        wire_config: WireConfig
    ) -> Self {
        info!("Creating new connection with custom wire config");
        
        // Initialize FramedMessage with custom WireConfig
        let framed_message = FramedMessage::new(wire_config);
        
        debug!("Connection initialized with custom config and peer address: {:?}", 
               stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap()));
        
        Self {
            stream,
            peer_id: None, // Will be set during handshake
            identity,
            framed_message,
        }
    }
    
    #[instrument(level = "debug", skip(self, msg), fields(msg_type = msg.message_type()))]
    pub async fn send_message(&mut self, msg: Message) -> Result<(), ConnectionError> {
        info!("Sending {} message", msg.message_type());
        debug!("Message nonce: {}, payload_len: {}", msg.get_nonce(), msg.get_payload().len());
        
        // Create SignedEnvelope using our identity
        let envelope = SignedEnvelope::create(&msg, &self.identity, None)
            .map_err(|e| {
                error!("Failed to create signed envelope: {}", e);
                ConnectionError::WireProtocol(WireProtocolError::Serialization(
                    bincode::Error::from(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Envelope creation failed: {}", e)
                    ))
                ))
            })?;
        
        debug!("Created signed envelope with sender: {}", envelope.sender());
        
        // Get the message size for logging
        let envelope_size = bincode::serialize(&envelope)
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        
        debug!("Envelope size: {} bytes", envelope_size);
        
        // Use framed_message to write with default timeout
        self.framed_message.write_message_with_default_timeout(&mut self.stream, &envelope)
            .await
            .map_err(|e| {
                error!("Failed to write message: {}", e);
                ConnectionError::WireProtocol(WireProtocolError::WriteTimeout {
                    timeout: Duration::from_secs(30), // Default timeout
                })
            })?;
        
        info!("Successfully sent {} message ({} bytes)", msg.message_type(), envelope_size);
        Ok(())
    }
    
    #[instrument(level = "debug", skip(self), fields(peer_id = self.peer_id.as_deref()))]
    pub async fn receive_message(&mut self) -> Result<(Message, String), ConnectionError> {
        info!("Waiting to receive message");
        
        // Use framed_message to read with default timeout
        let envelope = self.framed_message.read_message_with_default_timeout(&mut self.stream)
            .await
            .map_err(|e| {
                error!("Failed to read message: {}", e);
                ConnectionError::WireProtocol(WireProtocolError::ReadTimeout {
                    timeout: Duration::from_secs(30), // Default timeout
                })
            })?;
        
        debug!("Received envelope from sender: {}", envelope.sender());
        
        // Verify the signature of the received envelope
        if !envelope.verify_signature() {
            error!("Signature verification failed for message from {}", envelope.sender());
            return Err(ConnectionError::InvalidSignature);
        }
        
        debug!("Signature verification passed for sender: {}", envelope.sender());
        
        // Validate timestamp (using default max age of 5 minutes)
        const MAX_MESSAGE_AGE_SECONDS: u64 = 300; // 5 minutes
        if !envelope.is_timestamp_valid(MAX_MESSAGE_AGE_SECONDS) {
            warn!(
                "Message timestamp validation failed for sender: {}, age: {} seconds",
                envelope.sender(),
                envelope.get_age_seconds()
            );
            return Err(ConnectionError::InvalidTimestamp);
        }
        
        // Extract the message from the envelope
        let message = envelope.get_message()
            .map_err(|e| {
                error!("Failed to deserialize message: {}", e);
                ConnectionError::WireProtocol(WireProtocolError::CorruptedData {
                    reason: format!("Message deserialization failed: {}", e),
                })
            })?;
        
        let sender_id = envelope.sender().to_string();
        
        info!(
            "Successfully received {} message from {} (age: {} seconds)",
            message.message_type(),
            sender_id,
            envelope.get_age_seconds()
        );
        
        debug!(
            "Message details - nonce: {}, payload_len: {}",
            message.get_nonce(),
            message.get_payload().len()
        );
        
        Ok((message, sender_id))
    }
    
    #[instrument(level = "debug", skip(self), fields(local_peer = self.identity.peer_id().as_str()))]
    pub async fn handshake(&mut self) -> Result<String> {
        info!("Starting handshake protocol");
        
        // Step 5.1: Use appropriate handshake timeout from network configuration constants
        // Generate a unique handshake nonce to prevent replay attacks
        let handshake_nonce = rand::random::<u64>();
        
        // Create handshake request message with local identity information
        // Using a special payload format: "HANDSHAKE_REQUEST:<peer_id>"
        let local_peer_id = self.identity.peer_id().as_str().to_string();
        let handshake_payload = format!("HANDSHAKE_REQUEST:{}", local_peer_id);
        let handshake_request = Message::new_ping(handshake_nonce, handshake_payload);
        
        debug!(
            handshake_nonce = handshake_nonce,
            local_peer_id = %local_peer_id,
            "Sending handshake request"
        );
        
        // Send handshake request
        self.send_message(handshake_request).await
            .context("Failed to send handshake request")?;
        
        info!("Handshake request sent, waiting for response");
        
        // Receive handshake response with Step 5.1 network-appropriate timeout
        const HANDSHAKE_TIMEOUT_SECONDS: u64 = 10; // Using NETWORK_DEFAULT_HANDSHAKE_TIMEOUT
        let receive_result = tokio::time::timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT_SECONDS),
            self.receive_message()
        ).await;
        
        let (response_message, peer_identity) = match receive_result {
            Ok(Ok((msg, peer_id))) => (msg, peer_id),
            Ok(Err(e)) => {
                error!("Failed to receive handshake response: {}", e);
                return Err(anyhow::Error::from(e))
                    .context("Failed to receive handshake response");
            }
            Err(_) => {
                error!("Handshake response timed out after {} seconds", HANDSHAKE_TIMEOUT_SECONDS);
                return Err(anyhow::anyhow!("Handshake response timeout after {} seconds", HANDSHAKE_TIMEOUT_SECONDS))
                    .context("Handshake failed due to timeout");
            }
        };
        
        debug!(
            peer_identity = %peer_identity,
            response_nonce = response_message.get_nonce(),
            response_payload = response_message.get_payload(),
            "Received handshake response"
        );
        
        // Validate that this is a proper handshake response
        if !response_message.is_pong() {
            error!("Expected Pong message for handshake response, got {}", response_message.message_type());
            return Err(anyhow::anyhow!("Expected Pong message, got {}", response_message.message_type()))
                .context("Invalid handshake response message type");
        }
        
        // Validate the nonce matches our request
        if response_message.get_nonce() != handshake_nonce {
            error!(
                expected_nonce = handshake_nonce,
                received_nonce = response_message.get_nonce(),
                "Handshake response nonce mismatch"
            );
            return Err(anyhow::anyhow!(
                "Nonce mismatch: expected {}, got {}", 
                handshake_nonce, 
                response_message.get_nonce()
            )).context("Handshake nonce validation failed");
        }
        
        // Validate the response payload format: "HANDSHAKE_RESPONSE:<peer_id>"
        let expected_response_prefix = "HANDSHAKE_RESPONSE:";
        let response_payload = response_message.get_payload();
        
        if !response_payload.starts_with(expected_response_prefix) {
            error!(
                expected_prefix = expected_response_prefix,
                received_payload = response_payload,
                "Invalid handshake response payload format"
            );
            return Err(anyhow::anyhow!(
                "Invalid response payload format: expected '{}' prefix, got '{}'", 
                expected_response_prefix, 
                response_payload
            )).context("Handshake response payload validation failed");
        }
        
        // Extract peer ID from response payload
        let response_peer_id = response_payload
            .strip_prefix(expected_response_prefix)
            .unwrap_or("")
            .to_string();
        
        // Validate that the peer ID in the payload matches the one from the signed envelope
        if response_peer_id != peer_identity {
            error!(
                payload_peer_id = %response_peer_id,
                envelope_peer_id = %peer_identity,
                "Peer ID mismatch between payload and envelope"
            );
            return Err(anyhow::anyhow!(
                "Peer ID mismatch: payload has '{}', envelope has '{}'", 
                response_peer_id, 
                peer_identity
            )).context("Handshake peer identity validation failed");
        }
        
        // Validate peer identity format (basic check)
        if response_peer_id.is_empty() {
            error!("Received empty peer ID in handshake response");
            return Err(anyhow::anyhow!("Empty peer ID in handshake response"))
                .context("Handshake peer identity validation failed");
        }
        
        // Store the authenticated peer identity
        self.peer_id = Some(peer_identity.clone());
        
        info!(
            peer_id = %peer_identity,
            "Handshake completed successfully"
        );
        
        debug!(
            local_peer = %local_peer_id,
            remote_peer = %peer_identity,
            handshake_nonce = handshake_nonce,
            "Handshake authentication successful"
        );
        
        Ok(peer_identity)
    }

    /// Check if the connection has completed the handshake and is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.peer_id.is_some()
    }

    /// Get the authenticated peer identity, if available
    pub fn peer_identity(&self) -> Option<&str> {
        self.peer_id.as_deref()
    }

    /// Close the connection gracefully
    /// 
    /// This method attempts to shutdown the TCP stream gracefully.
    /// Note: The actual close operation is handled by dropping the TcpStream.
    #[instrument(level = "debug", skip(self))]
    pub async fn close(&mut self) -> Result<(), ConnectionError> {
        info!("Closing connection to peer: {:?}", self.peer_id);
        
        // In Tokio, TcpStream doesn't have an explicit close method
        // The connection will be closed when the stream is dropped
        // We could implement a graceful shutdown here if needed
        
        // Mark the connection as closed by clearing peer_id
        if let Some(peer_id) = &self.peer_id {
            debug!("Connection to {} marked as closed", peer_id);
        }
        self.peer_id = None;
        
        info!("Connection closed successfully");
        Ok(())
    }

    /// Check if the connection is closed
    pub fn is_closed(&self) -> bool {
        // For now, we consider a connection closed if we've explicitly cleared the peer_id
        // In a more sophisticated implementation, we might also check the TCP stream state
        !self.is_authenticated()
    }

    /// Get the local socket address of this connection
    pub fn local_addr(&self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.stream.local_addr()
    }

    /// Get the remote socket address of this connection
    pub fn peer_addr(&self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.stream.peer_addr()
    }

    /// Handle an incoming handshake request (server-side handshake handling)
    /// 
    /// This method should be called by servers to respond to incoming handshake requests.
    /// It expects to receive a handshake request and responds with appropriate handshake response.
    /// 
    /// # Returns
    /// * `Ok(String)` - The authenticated peer identity from the handshake request
    /// * `Err(anyhow::Error)` - Handshake failure
    #[instrument(level = "debug", skip(self), fields(local_peer = self.identity.peer_id().as_str()))]
    pub async fn handle_handshake_request(&mut self) -> Result<String> {
        info!("Waiting for incoming handshake request");
        
        // Receive handshake request with timeout
        const HANDSHAKE_TIMEOUT_SECONDS: u64 = 10; // 10 seconds for handshake
        let receive_result = tokio::time::timeout(
            Duration::from_secs(HANDSHAKE_TIMEOUT_SECONDS),
            self.receive_message()
        ).await;
        
        let (request_message, peer_identity) = match receive_result {
            Ok(Ok((msg, peer_id))) => (msg, peer_id),
            Ok(Err(e)) => {
                error!("Failed to receive handshake request: {}", e);
                return Err(anyhow::Error::from(e))
                    .context("Failed to receive handshake request");
            }
            Err(_) => {
                error!("Handshake request timed out after {} seconds", HANDSHAKE_TIMEOUT_SECONDS);
                return Err(anyhow::anyhow!("Handshake request timeout after {} seconds", HANDSHAKE_TIMEOUT_SECONDS))
                    .context("Handshake failed due to timeout");
            }
        };
        
        debug!(
            peer_identity = %peer_identity,
            request_nonce = request_message.get_nonce(),
            request_payload = request_message.get_payload(),
            "Received handshake request"
        );
        
        // Validate that this is a proper handshake request
        if !request_message.is_ping() {
            error!("Expected Ping message for handshake request, got {}", request_message.message_type());
            return Err(anyhow::anyhow!("Expected Ping message, got {}", request_message.message_type()))
                .context("Invalid handshake request message type");
        }
        
        // Validate the request payload format: "HANDSHAKE_REQUEST:<peer_id>"
        let expected_request_prefix = "HANDSHAKE_REQUEST:";
        let request_payload = request_message.get_payload();
        
        if !request_payload.starts_with(expected_request_prefix) {
            error!(
                expected_prefix = expected_request_prefix,
                received_payload = request_payload,
                "Invalid handshake request payload format"
            );
            return Err(anyhow::anyhow!(
                "Invalid request payload format: expected '{}' prefix, got '{}'", 
                expected_request_prefix, 
                request_payload
            )).context("Handshake request payload validation failed");
        }
        
        // Extract peer ID from request payload
        let request_peer_id = request_payload
            .strip_prefix(expected_request_prefix)
            .unwrap_or("")
            .to_string();
        
        // Validate that the peer ID in the payload matches the one from the signed envelope
        if request_peer_id != peer_identity {
            error!(
                payload_peer_id = %request_peer_id,
                envelope_peer_id = %peer_identity,
                "Peer ID mismatch between payload and envelope in request"
            );
            return Err(anyhow::anyhow!(
                "Peer ID mismatch in request: payload has '{}', envelope has '{}'", 
                request_peer_id, 
                peer_identity
            )).context("Handshake request peer identity validation failed");
        }
        
        // Validate peer identity format (basic check)
        if request_peer_id.is_empty() {
            error!("Received empty peer ID in handshake request");
            return Err(anyhow::anyhow!("Empty peer ID in handshake request"))
                .context("Handshake request peer identity validation failed");
        }
        
        // Create handshake response message
        let local_peer_id = self.identity.peer_id().as_str().to_string();
        let response_payload = format!("HANDSHAKE_RESPONSE:{}", local_peer_id);
        let handshake_response = Message::new_pong(request_message.get_nonce(), response_payload);
        
        debug!(
            response_nonce = request_message.get_nonce(),
            local_peer_id = %local_peer_id,
            "Sending handshake response"
        );
        
        // Send handshake response
        self.send_message(handshake_response).await
            .context("Failed to send handshake response")?;
        
        // Store the authenticated peer identity
        self.peer_id = Some(peer_identity.clone());
        
        info!(
            peer_id = %peer_identity,
            "Handshake request handled successfully"
        );
        
        debug!(
            local_peer = %local_peer_id,
            remote_peer = %peer_identity,
            handshake_nonce = request_message.get_nonce(),
            "Handshake authentication successful (server-side)"
        );
        
        Ok(peer_identity)
    }
}
