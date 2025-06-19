use crate::crypto::identity::{Identity, PeerId};
use crate::messages::chess::{
    GameAccept, GameDecline, GameInvite, Move, MoveAck, SyncRequest, SyncResponse,
};
use anyhow::{Context, Result};
use ed25519_dalek::Signature;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Default maximum age for messages in seconds (5 minutes)
pub const DEFAULT_MAX_MESSAGE_AGE_SECONDS: u64 = 300;

/// Expected Ed25519 signature length in bytes
pub const ED25519_SIGNATURE_LENGTH: usize = 64;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    // Existing variants
    Ping { nonce: u64, payload: String },
    Pong { nonce: u64, payload: String },

    // New chess variants
    GameInvite(GameInvite),
    GameAccept(GameAccept),
    GameDecline(GameDecline),
    Move(Move),
    MoveAck(MoveAck),
    SyncRequest(SyncRequest),
    SyncResponse(SyncResponse),
}

impl Message {
    /// Create a new Ping message
    pub fn new_ping(nonce: u64, payload: String) -> Self {
        Message::Ping { nonce, payload }
    }

    /// Create a new Pong message
    pub fn new_pong(nonce: u64, payload: String) -> Self {
        Message::Pong { nonce, payload }
    }

    /// Get the nonce from either Ping or Pong message
    /// Panics for chess messages as they don't have nonces
    pub fn get_nonce(&self) -> u64 {
        match self {
            Message::Ping { nonce, .. } => *nonce,
            Message::Pong { nonce, .. } => *nonce,
            Message::GameInvite(_)
            | Message::GameAccept(_)
            | Message::GameDecline(_)
            | Message::Move(_)
            | Message::MoveAck(_)
            | Message::SyncRequest(_)
            | Message::SyncResponse(_) => {
                panic!("get_nonce() called on chess message - use get_game_id() instead")
            }
        }
    }

    /// Get the payload from either Ping or Pong message
    /// Panics for chess messages as they don't have payloads
    pub fn get_payload(&self) -> &str {
        match self {
            Message::Ping { payload, .. } => payload,
            Message::Pong { payload, .. } => payload,
            Message::GameInvite(_)
            | Message::GameAccept(_)
            | Message::GameDecline(_)
            | Message::Move(_)
            | Message::MoveAck(_)
            | Message::SyncRequest(_)
            | Message::SyncResponse(_) => {
                panic!("get_payload() called on chess message - chess messages don't have payloads")
            }
        }
    }

    /// Check if this is a Ping message
    pub fn is_ping(&self) -> bool {
        matches!(self, Message::Ping { .. })
    }

    /// Check if this is a Pong message
    pub fn is_pong(&self) -> bool {
        matches!(self, Message::Pong { .. })
    }

    /// Check if this is a chess message
    pub fn is_chess_message(&self) -> bool {
        matches!(
            self,
            Message::GameInvite(_)
                | Message::GameAccept(_)
                | Message::GameDecline(_)
                | Message::Move(_)
                | Message::MoveAck(_)
                | Message::SyncRequest(_)
                | Message::SyncResponse(_)
        )
    }

    /// Get the game ID from chess messages
    /// Returns None for Ping/Pong messages
    pub fn get_game_id(&self) -> Option<&str> {
        match self {
            Message::GameInvite(msg) => Some(&msg.game_id),
            Message::GameAccept(msg) => Some(&msg.game_id),
            Message::GameDecline(msg) => Some(&msg.game_id),
            Message::Move(msg) => Some(&msg.game_id),
            Message::MoveAck(msg) => Some(&msg.game_id),
            Message::SyncRequest(msg) => Some(&msg.game_id),
            Message::SyncResponse(msg) => Some(&msg.game_id),
            Message::Ping { .. } | Message::Pong { .. } => None,
        }
    }

    /// Get the message type as a string
    pub fn message_type(&self) -> &'static str {
        match self {
            Message::Ping { .. } => "Ping",
            Message::Pong { .. } => "Pong",
            Message::GameInvite(_) => "GameInvite",
            Message::GameAccept(_) => "GameAccept",
            Message::GameDecline(_) => "GameDecline",
            Message::Move(_) => "Move",
            Message::MoveAck(_) => "MoveAck",
            Message::SyncRequest(_) => "SyncRequest",
            Message::SyncResponse(_) => "SyncResponse",
        }
    }

    /// Serialize the message to binary format using bincode
    ///
    /// # Returns
    /// - `Ok(Vec<u8>)` - Successfully serialized message bytes
    /// - `Err(bincode::Error)` - Serialization failed
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    ///
    /// let msg = Message::new_ping(42, "hello".to_string());
    /// let bytes = msg.serialize().expect("Failed to serialize");
    /// assert!(!bytes.is_empty());
    /// ```
    pub fn serialize(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// Deserialize binary data back into a Message using bincode
    ///
    /// # Arguments
    /// * `data` - Binary data to deserialize
    ///
    /// # Returns
    /// - `Ok(Message)` - Successfully deserialized message
    /// - `Err(bincode::Error)` - Deserialization failed (invalid format, corrupted data, etc.)
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    ///
    /// let original = Message::new_pong(123, "world".to_string());
    /// let bytes = original.serialize().unwrap();
    /// let restored = Message::deserialize(&bytes).expect("Failed to deserialize");
    /// assert_eq!(original.get_nonce(), restored.get_nonce());
    /// ```
    pub fn deserialize(data: &[u8]) -> Result<Message, bincode::Error> {
        bincode::deserialize(data)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SignedEnvelope {
    pub message: Vec<u8>,   // Serialized Message
    pub signature: Vec<u8>, // Ed25519 signature
    pub sender: String,     // PeerId
    pub timestamp: u64,     // Unix timestamp
}

impl SignedEnvelope {
    /// Create a new SignedEnvelope with validation
    ///
    /// # Arguments
    /// * `message` - Serialized message bytes (must not be empty)
    /// * `signature` - Ed25519 signature bytes (must be exactly 64 bytes)
    /// * `sender` - PeerId of the message sender
    /// * `timestamp` - Unix timestamp in seconds
    ///
    /// # Returns
    /// * `Result<SignedEnvelope>` - Successfully created envelope or validation error
    ///
    /// # Errors
    /// * Empty message bytes
    /// * Invalid signature length
    /// * Invalid PeerId format
    pub fn new(
        message: Vec<u8>,
        signature: Vec<u8>,
        sender: String,
        timestamp: u64,
    ) -> Result<Self> {
        // Validate message is not empty
        if message.is_empty() {
            return Err(anyhow::anyhow!("Message bytes cannot be empty"));
        }

        // Validate signature length
        if signature.len() != ED25519_SIGNATURE_LENGTH {
            return Err(anyhow::anyhow!(
                "Invalid signature length: expected {} bytes, got {}",
                ED25519_SIGNATURE_LENGTH,
                signature.len()
            ));
        }

        // Validate PeerId format by attempting conversion
        let peer_id = PeerId::from_string(sender.clone());
        peer_id
            .to_verifying_key()
            .context("Invalid sender PeerId format")?;

        Ok(SignedEnvelope {
            message,
            signature,
            sender,
            timestamp,
        })
    }

    /// Create a signed envelope from a Message and Identity
    ///
    /// # Arguments
    /// * `message` - The Message to wrap in an envelope
    /// * `identity` - Identity to sign the message with
    /// * `timestamp` - Optional timestamp (uses current time if None)
    ///
    /// # Returns
    /// * `Result<SignedEnvelope>` - Successfully created and signed envelope
    ///
    /// # Errors
    /// * Message serialization failure
    /// * System time error (if timestamp is None)
    pub fn create(message: &Message, identity: &Identity, timestamp: Option<u64>) -> Result<Self> {
        // Serialize the message
        let message_bytes = message.serialize().context("Failed to serialize message")?;

        // Get timestamp (current time if not provided)
        let envelope_timestamp = match timestamp {
            Some(ts) => ts,
            None => SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .context("Failed to get current timestamp")?
                .as_secs(),
        };

        // Sign the serialized message
        let signature = identity.sign(&message_bytes);
        let signature_bytes = signature.to_bytes().to_vec();

        // Get sender PeerId
        let sender = identity.peer_id().as_str().to_string();

        // Create envelope using the new() method for validation
        Self::new(message_bytes, signature_bytes, sender, envelope_timestamp)
    }

    /// Verify the signature of this envelope
    ///
    /// # Returns
    /// * `bool` - True if signature is valid, false otherwise
    ///
    /// # Notes
    /// * Returns false on any error (invalid PeerId, malformed signature, etc.)
    /// * Uses constant-time verification for security
    pub fn verify_signature(&self) -> bool {
        // Convert sender to PeerId and then to VerifyingKey
        let peer_id = PeerId::from_string(self.sender.clone());
        let verifying_key = match peer_id.to_verifying_key() {
            Ok(key) => key,
            Err(_) => return false, // Invalid PeerId format
        };

        // Convert signature bytes to Signature
        let signature_array: [u8; 64] = match self.signature.as_slice().try_into() {
            Ok(arr) => arr,
            Err(_) => return false, // Invalid signature length
        };

        let signature = Signature::from_bytes(&signature_array);

        // Verify signature against message bytes
        Identity::verify(&verifying_key, &self.message, &signature)
    }

    /// Deserialize the message from this envelope
    ///
    /// # Returns
    /// * `Result<Message>` - Deserialized message or error
    pub fn get_message(&self) -> Result<Message> {
        Message::deserialize(&self.message)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize message: {}", e))
    }

    /// Get the sender PeerId
    pub fn sender(&self) -> &str {
        &self.sender
    }

    /// Get the timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Check if the envelope timestamp is within the acceptable age
    ///
    /// # Arguments
    /// * `max_age_seconds` - Maximum acceptable age in seconds
    ///
    /// # Returns
    /// * `bool` - True if timestamp is valid (not too old, not in future)
    pub fn is_timestamp_valid(&self, max_age_seconds: u64) -> bool {
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(_) => return false, // System time error
        };

        // Check if message is too old
        if self.timestamp + max_age_seconds < now {
            return false;
        }

        // Check if message is from the future (allow small clock skew)
        const MAX_FUTURE_SKEW_SECONDS: u64 = 60; // 1 minute
        if self.timestamp > now + MAX_FUTURE_SKEW_SECONDS {
            return false;
        }

        true
    }

    /// Get the age of this envelope in seconds
    ///
    /// # Returns
    /// * `u64` - Age in seconds (0 if timestamp is in the future)
    pub fn get_age_seconds(&self) -> u64 {
        let now = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(_) => return 0, // System time error
        };

        now.saturating_sub(self.timestamp)
    }
}
