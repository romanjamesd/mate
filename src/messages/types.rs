use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Ping { nonce: u64, payload: String },
    Pong { nonce: u64, payload: String },
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
    pub fn get_nonce(&self) -> u64 {
        match self {
            Message::Ping { nonce, .. } => *nonce,
            Message::Pong { nonce, .. } => *nonce,
        }
    }

    /// Get the payload from either Ping or Pong message
    pub fn get_payload(&self) -> &str {
        match self {
            Message::Ping { payload, .. } => payload,
            Message::Pong { payload, .. } => payload,
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

    /// Get the message type as a string
    pub fn message_type(&self) -> &'static str {
        match self {
            Message::Ping { .. } => "Ping",
            Message::Pong { .. } => "Pong",
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SignedEnvelope {
    pub message: Vec<u8>,      // Serialized Message
    pub signature: Vec<u8>,    // Ed25519 signature
    pub sender: String,        // PeerId
    pub timestamp: u64,        // Unix timestamp
}

impl SignedEnvelope {
    pub fn new(message: Vec<u8>, signature: Vec<u8>, sender: String, timestamp: u64) -> Self {
        // Implementation placeholder
        todo!()
    }
    
    pub fn verify_signature(&self) -> bool {
        // Implementation placeholder
        todo!()
    }
}
