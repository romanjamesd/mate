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

    /// Create a new GameInvite message
    ///
    /// # Arguments
    /// * `game_id` - Unique game identifier (should be a valid UUID)
    /// * `suggested_color` - Optional color suggestion for the invitee
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::generate_game_id;
    /// use mate::chess::Color;
    ///
    /// let msg = Message::new_game_invite(generate_game_id(), Some(Color::White));
    /// assert!(msg.is_chess_message());
    /// ```
    pub fn new_game_invite(game_id: String, suggested_color: Option<crate::chess::Color>) -> Self {
        use crate::messages::chess::GameInvite;
        Message::GameInvite(GameInvite::new(game_id, suggested_color))
    }

    /// Create a new GameAccept message
    ///
    /// # Arguments
    /// * `game_id` - Game identifier being accepted
    /// * `accepted_color` - Color the accepter wants to play as
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::generate_game_id;
    /// use mate::chess::Color;
    ///
    /// let msg = Message::new_game_accept(generate_game_id(), Color::Black);
    /// assert!(msg.is_chess_message());
    /// ```
    pub fn new_game_accept(game_id: String, accepted_color: crate::chess::Color) -> Self {
        use crate::messages::chess::GameAccept;
        Message::GameAccept(GameAccept::new(game_id, accepted_color))
    }

    /// Create a new GameDecline message
    ///
    /// # Arguments
    /// * `game_id` - Game identifier being declined
    /// * `reason` - Optional reason for declining
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::generate_game_id;
    ///
    /// let msg = Message::new_game_decline(generate_game_id(), Some("Already in a game".to_string()));
    /// assert!(msg.is_chess_message());
    /// ```
    pub fn new_game_decline(game_id: String, reason: Option<String>) -> Self {
        use crate::messages::chess::GameDecline;
        Message::GameDecline(GameDecline::new(game_id, reason))
    }

    /// Create a new Move message
    ///
    /// # Arguments
    /// * `game_id` - Game identifier for the move
    /// * `chess_move` - Chess move in algebraic notation (e.g., "e2e4")
    /// * `board_state_hash` - SHA-256 hash of the board state after the move
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::{generate_game_id, hash_board_state};
    /// use mate::chess::Board;
    ///
    /// let board = Board::new();
    /// let msg = Message::new_move(
    ///     generate_game_id(),
    ///     "e2e4".to_string(),
    ///     hash_board_state(&board)
    /// );
    /// assert!(msg.is_chess_message());
    /// ```
    pub fn new_move(game_id: String, chess_move: String, board_state_hash: String) -> Self {
        use crate::messages::chess::Move;
        Message::Move(Move::new(game_id, chess_move, board_state_hash))
    }

    /// Create a new MoveAck message
    ///
    /// # Arguments
    /// * `game_id` - Game identifier for the acknowledgment
    /// * `move_id` - Optional move identifier being acknowledged
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::generate_game_id;
    ///
    /// let msg = Message::new_move_ack(generate_game_id(), Some("move-123".to_string()));
    /// assert!(msg.is_chess_message());
    /// ```
    pub fn new_move_ack(game_id: String, move_id: Option<String>) -> Self {
        use crate::messages::chess::MoveAck;
        Message::MoveAck(MoveAck::new(game_id, move_id))
    }

    /// Create a new SyncRequest message
    ///
    /// # Arguments
    /// * `game_id` - Game identifier to request sync for
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::generate_game_id;
    ///
    /// let msg = Message::new_sync_request(generate_game_id());
    /// assert!(msg.is_chess_message());
    /// ```
    pub fn new_sync_request(game_id: String) -> Self {
        use crate::messages::chess::SyncRequest;
        Message::SyncRequest(SyncRequest::new(game_id))
    }

    /// Create a new SyncResponse message
    ///
    /// # Arguments
    /// * `game_id` - Game identifier for the sync response
    /// * `board_state` - Current board state in FEN notation
    /// * `move_history` - Complete move history in algebraic notation
    /// * `board_state_hash` - SHA-256 hash of the current board state
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::{generate_game_id, hash_board_state};
    /// use mate::chess::Board;
    ///
    /// let board = Board::new();
    /// let msg = Message::new_sync_response(
    ///     generate_game_id(),
    ///     board.to_fen(),
    ///     vec!["e2e4".to_string(), "e7e5".to_string()],
    ///     hash_board_state(&board)
    /// );
    /// assert!(msg.is_chess_message());
    /// ```
    pub fn new_sync_response(
        game_id: String,
        board_state: String,
        move_history: Vec<String>,
        board_state_hash: String,
    ) -> Self {
        use crate::messages::chess::SyncResponse;
        Message::SyncResponse(SyncResponse::new(
            game_id,
            board_state,
            move_history,
            board_state_hash,
        ))
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

    /// Serialize the message to JSON format for debugging and interoperability
    ///
    /// This method provides human-readable serialization primarily for debugging,
    /// logging, and interoperability with non-Rust systems. For efficient network
    /// transmission, use `serialize()` which uses binary format.
    ///
    /// # Returns
    /// - `Ok(String)` - Successfully serialized JSON string
    /// - `Err(serde_json::Error)` - Serialization failed
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::{GameInvite, generate_game_id};
    /// use mate::chess::Color;
    ///
    /// let invite = GameInvite::new(generate_game_id(), Some(Color::White));
    /// let msg = Message::GameInvite(invite);
    /// let json = msg.to_json().expect("Failed to serialize to JSON");
    /// assert!(json.contains("GameInvite"));
    /// ```
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize a message from JSON format
    ///
    /// This method provides the counterpart to `to_json()` for debugging and
    /// interoperability purposes. For efficient network transmission, use
    /// `deserialize()` which handles binary format.
    ///
    /// # Arguments
    /// * `json` - JSON string to deserialize
    ///
    /// # Returns
    /// - `Ok(Message)` - Successfully deserialized message
    /// - `Err(serde_json::Error)` - Deserialization failed
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    ///
    /// let original = Message::new_ping(42, "test".to_string());
    /// let json = original.to_json().unwrap();
    /// let restored = Message::from_json(&json).expect("Failed to deserialize from JSON");
    /// assert_eq!(original.get_nonce(), restored.get_nonce());
    /// ```
    pub fn from_json(json: &str) -> Result<Message, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Estimate the serialized size of this message in bytes
    ///
    /// Provides an estimate of how large this message will be when serialized
    /// using the binary format. This is useful for wire protocol planning and
    /// ensuring messages don't exceed size limits.
    ///
    /// Note: This is an estimate and may not exactly match the actual serialized size
    /// due to compression and encoding variations in bincode.
    ///
    /// # Returns
    /// Estimated size in bytes
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    ///
    /// let small_msg = Message::new_ping(42, "hi".to_string());
    /// let large_msg = Message::new_ping(42, "a".repeat(1000));
    /// assert!(large_msg.estimated_size() > small_msg.estimated_size());
    /// ```
    pub fn estimated_size(&self) -> usize {
        match self {
            Message::Ping { payload, .. } => {
                // Base overhead + nonce (8 bytes) + payload length + string overhead
                32 + 8 + payload.len()
            }
            Message::Pong { payload, .. } => {
                // Similar to Ping
                32 + 8 + payload.len()
            }
            Message::GameInvite(invite) => {
                // Base overhead + game_id (UUID ~36 chars) + optional color (1 byte)
                32 + invite.game_id.len() + 8
            }
            Message::GameAccept(accept) => {
                // Base overhead + game_id + color (1 byte)
                32 + accept.game_id.len() + 8
            }
            Message::GameDecline(decline) => {
                // Base overhead + game_id + optional reason
                let reason_size = decline.reason.as_ref().map_or(0, |r| r.len());
                32 + decline.game_id.len() + reason_size + 8
            }
            Message::Move(mv) => {
                // Base overhead + game_id + chess_move + board_state_hash (64 chars)
                32 + mv.game_id.len() + mv.chess_move.len() + mv.board_state_hash.len() + 16
            }
            Message::MoveAck(ack) => {
                // Base overhead + game_id + optional move_id
                let move_id_size = ack.move_id.as_ref().map_or(0, |id| id.len());
                32 + ack.game_id.len() + move_id_size + 8
            }
            Message::SyncRequest(req) => {
                // Base overhead + game_id
                32 + req.game_id.len() + 8
            }
            Message::SyncResponse(resp) => {
                // Base overhead + game_id + board_state (FEN ~80 chars) + move_history + hash
                let move_history_size: usize = resp.move_history.iter().map(|m| m.len() + 4).sum();
                32 + resp.game_id.len()
                    + resp.board_state.len()
                    + move_history_size
                    + resp.board_state_hash.len()
                    + 32
            }
        }
    }

    /// Check if this message is likely to be large (for wire protocol planning)
    ///
    /// Returns true if this message type is expected to potentially be large
    /// and may require special handling in the wire protocol (e.g., streaming,
    /// compression, or increased size limits).
    ///
    /// # Returns
    /// `true` if the message could be large, `false` for typically small messages
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::{SyncResponse, generate_game_id};
    ///
    /// let ping = Message::new_ping(42, "hello".to_string());
    /// assert!(!ping.is_potentially_large());
    ///
    /// let sync_resp = SyncResponse::new(
    ///     generate_game_id(),
    ///     "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string(),
    ///     vec!["e2e4".to_string(); 100], // Large move history
    ///     "abcd".repeat(16) // 64-char hash
    /// );
    /// let sync_msg = Message::SyncResponse(sync_resp);
    /// assert!(sync_msg.is_potentially_large());
    /// ```
    pub fn is_potentially_large(&self) -> bool {
        match self {
            // Ping/Pong are typically small
            Message::Ping { .. } | Message::Pong { .. } => false,
            // Game management messages are typically small
            Message::GameInvite(_) | Message::GameAccept(_) | Message::GameDecline(_) => false,
            // Move messages are small
            Message::Move(_) | Message::MoveAck(_) => false,
            // Sync requests are small
            Message::SyncRequest(_) => false,
            // Sync responses can be large due to move history and board state
            Message::SyncResponse(_) => true,
        }
    }

    /// Get a summary string for logging purposes
    ///
    /// Returns a concise, human-readable summary of the message that's safe
    /// for logging without exposing sensitive data or creating overly long log entries.
    ///
    /// # Returns
    /// A string suitable for logging that describes the message type and key identifiers
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::{GameInvite, generate_game_id};
    /// use mate::chess::Color;
    ///
    /// let game_id = generate_game_id();
    /// let invite = GameInvite::new(game_id.clone(), Some(Color::White));
    /// let msg = Message::GameInvite(invite);
    /// let summary = msg.log_summary();
    /// assert!(summary.contains("GameInvite"));
    /// assert!(summary.contains(&game_id[..8])); // First 8 chars of game ID
    /// ```
    pub fn log_summary(&self) -> String {
        match self {
            Message::Ping { nonce, .. } => format!("Ping(nonce={nonce})"),
            Message::Pong { nonce, .. } => format!("Pong(nonce={nonce})"),
            Message::GameInvite(invite) => {
                let color_str = invite
                    .suggested_color
                    .map_or("any".to_string(), |c| format!("{c:?}"));
                let game_id_short = &invite.game_id[..8.min(invite.game_id.len())];
                format!("GameInvite(game={game_id_short}, color={color_str})")
            }
            Message::GameAccept(accept) => {
                let game_id_short = &accept.game_id[..8.min(accept.game_id.len())];
                let accepted_color = accept.accepted_color;
                format!("GameAccept(game={game_id_short}, color={accepted_color:?})")
            }
            Message::GameDecline(decline) => {
                let reason_info = decline.reason.as_ref().map_or("none".to_string(), |r| {
                    let len = r.len();
                    format!("{len}chars")
                });
                let game_id_short = &decline.game_id[..8.min(decline.game_id.len())];
                format!("GameDecline(game={game_id_short}, reason={reason_info})")
            }
            Message::Move(mv) => {
                let game_id_short = &mv.game_id[..8.min(mv.game_id.len())];
                let chess_move = &mv.chess_move;
                format!("Move(game={game_id_short}, move={chess_move})")
            }
            Message::MoveAck(ack) => {
                let move_id_info = ack.move_id.as_ref().map_or("none".to_string(), |id| {
                    let len = id.len();
                    format!("{len}chars")
                });
                let game_id_short = &ack.game_id[..8.min(ack.game_id.len())];
                format!("MoveAck(game={game_id_short}, move_id={move_id_info})")
            }
            Message::SyncRequest(req) => {
                let game_id_short = &req.game_id[..8.min(req.game_id.len())];
                format!("SyncRequest(game={game_id_short})")
            }
            Message::SyncResponse(resp) => {
                let game_id_short = &resp.game_id[..8.min(resp.game_id.len())];
                let moves_len = resp.move_history.len();
                format!("SyncResponse(game={game_id_short}, moves={moves_len})")
            }
        }
    }

    /// Validate the message format and constraints
    ///
    /// Performs comprehensive validation of the message using the validation
    /// functions from the chess module. This includes checking game ID formats,
    /// chess move validity, and other message-specific constraints.
    ///
    /// # Returns
    /// - `Ok(())` - Message is valid
    /// - `Err(ValidationError)` - Message validation failed
    ///
    /// # Example
    /// ```
    /// use mate::messages::types::Message;
    /// use mate::messages::chess::{GameInvite, generate_game_id};
    /// use mate::chess::Color;
    ///
    /// let valid_invite = GameInvite::new(generate_game_id(), Some(Color::White));
    /// let valid_msg = Message::GameInvite(valid_invite);
    /// assert!(valid_msg.validate().is_ok());
    ///
    /// let invalid_invite = GameInvite::new("not-a-uuid".to_string(), None);
    /// let invalid_msg = Message::GameInvite(invalid_invite);
    /// assert!(invalid_msg.validate().is_err());
    /// ```
    pub fn validate(&self) -> Result<(), crate::messages::chess::ValidationError> {
        use crate::messages::chess::{
            validate_game_accept, validate_game_decline, validate_game_invite, validate_move_ack,
            validate_move_message, validate_sync_request, validate_sync_response,
        };

        // First perform the basic validation
        let basic_validation = match self {
            // Ping/Pong messages don't require additional validation beyond type safety
            Message::Ping { .. } | Message::Pong { .. } => Ok(()),
            // Validate chess messages using their specific validation functions
            Message::GameInvite(invite) => validate_game_invite(invite),
            Message::GameAccept(accept) => validate_game_accept(accept),
            Message::GameDecline(decline) => validate_game_decline(decline),
            Message::Move(mv) => validate_move_message(mv),
            Message::MoveAck(ack) => validate_move_ack(ack),
            Message::SyncRequest(req) => validate_sync_request(req),
            Message::SyncResponse(resp) => validate_sync_response(resp),
        };

        // If basic validation passes, perform enhanced security validation
        basic_validation?;

        // Perform additional security validation for chess messages
        crate::messages::chess::security::validate_message_security(self).map_err(
            |security_error| {
                crate::messages::chess::ValidationError::InvalidMessageFormat(format!(
                    "Security validation failed: {security_error}"
                ))
            },
        )?;

        Ok(())
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
            let signature_len = signature.len();
            return Err(anyhow::anyhow!(
                "Invalid signature length: expected {ED25519_SIGNATURE_LENGTH} bytes, got {signature_len}"
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
            .map_err(|e| anyhow::anyhow!("Failed to deserialize message: {e}"))
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
