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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_ping() {
        let msg = Message::new_ping(12345, "test payload".to_string());
        assert!(msg.is_ping());
        assert!(!msg.is_pong());
        assert_eq!(msg.get_nonce(), 12345);
        assert_eq!(msg.get_payload(), "test payload");
        assert_eq!(msg.message_type(), "Ping");
    }

    #[test]
    fn test_new_pong() {
        let msg = Message::new_pong(67890, "response payload".to_string());
        assert!(msg.is_pong());
        assert!(!msg.is_ping());
        assert_eq!(msg.get_nonce(), 67890);
        assert_eq!(msg.get_payload(), "response payload");
        assert_eq!(msg.message_type(), "Pong");
    }

    #[test]
    fn test_message_accessors() {
        let ping = Message::Ping {
            nonce: 111,
            payload: "ping data".to_string(),
        };
        let pong = Message::Pong {
            nonce: 222,
            payload: "pong data".to_string(),
        };

        assert_eq!(ping.get_nonce(), 111);
        assert_eq!(ping.get_payload(), "ping data");
        assert_eq!(pong.get_nonce(), 222);
        assert_eq!(pong.get_payload(), "pong data");
    }

    #[test]
    fn test_message_type_detection() {
        let ping = Message::new_ping(1, "test".to_string());
        let pong = Message::new_pong(2, "test".to_string());

        assert!(ping.is_ping());
        assert!(!ping.is_pong());
        assert!(!pong.is_ping());
        assert!(pong.is_pong());
    }
}
