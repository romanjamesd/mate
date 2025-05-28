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
    fn test_ping_serialization_roundtrip() {
        let original = Message::new_ping(42, "test payload".to_string());
        
        // Serialize
        let serialized = original.serialize().expect("Failed to serialize ping");
        assert!(!serialized.is_empty(), "Serialized data should not be empty");
        
        // Deserialize
        let deserialized = Message::deserialize(&serialized).expect("Failed to deserialize ping");
        
        // Verify roundtrip
        assert_eq!(original.get_nonce(), deserialized.get_nonce());
        assert_eq!(original.get_payload(), deserialized.get_payload());
        assert!(deserialized.is_ping());
    }

    #[test]
    fn test_pong_serialization_roundtrip() {
        let original = Message::new_pong(999, "response data".to_string());
        
        // Serialize
        let serialized = original.serialize().expect("Failed to serialize pong");
        assert!(!serialized.is_empty(), "Serialized data should not be empty");
        
        // Deserialize
        let deserialized = Message::deserialize(&serialized).expect("Failed to deserialize pong");
        
        // Verify roundtrip
        assert_eq!(original.get_nonce(), deserialized.get_nonce());
        assert_eq!(original.get_payload(), deserialized.get_payload());
        assert!(deserialized.is_pong());
    }

    #[test]
    fn test_empty_payload_serialization() {
        let original = Message::new_ping(0, String::new());
        
        let serialized = original.serialize().expect("Failed to serialize empty payload");
        let deserialized = Message::deserialize(&serialized).expect("Failed to deserialize empty payload");
        
        assert_eq!(original.get_nonce(), deserialized.get_nonce());
        assert_eq!(original.get_payload(), deserialized.get_payload());
        assert!(deserialized.get_payload().is_empty());
    }

    #[test]
    fn test_large_payload_serialization() {
        let large_payload = "x".repeat(10000); // 10KB payload
        let original = Message::new_pong(u64::MAX, large_payload.clone());
        
        let serialized = original.serialize().expect("Failed to serialize large payload");
        let deserialized = Message::deserialize(&serialized).expect("Failed to deserialize large payload");
        
        assert_eq!(original.get_nonce(), deserialized.get_nonce());
        assert_eq!(original.get_payload(), deserialized.get_payload());
        assert_eq!(deserialized.get_payload().len(), 10000);
    }

    #[test]
    fn test_unicode_payload_serialization() {
        let unicode_payload = "Hello 世界 🌍 Здравствуй мир";
        let original = Message::new_ping(42, unicode_payload.to_string());
        
        let serialized = original.serialize().expect("Failed to serialize unicode payload");
        let deserialized = Message::deserialize(&serialized).expect("Failed to deserialize unicode payload");
        
        assert_eq!(original.get_payload(), deserialized.get_payload());
        assert_eq!(deserialized.get_payload(), unicode_payload);
    }

    #[test]
    fn test_deserialize_invalid_data() {
        let invalid_data = vec![0xFF, 0xFE, 0xFD]; // Random invalid bytes
        let result = Message::deserialize(&invalid_data);
        
        assert!(result.is_err(), "Should fail to deserialize invalid data");
    }

    #[test]
    fn test_deserialize_empty_data() {
        let empty_data = vec![];
        let result = Message::deserialize(&empty_data);
        
        assert!(result.is_err(), "Should fail to deserialize empty data");
    }

    #[test]
    fn test_serialization_deterministic() {
        let message = Message::new_ping(123, "deterministic test".to_string());
        
        let serialized1 = message.serialize().expect("First serialization failed");
        let serialized2 = message.serialize().expect("Second serialization failed");
        
        assert_eq!(serialized1, serialized2, "Serialization should be deterministic");
    }

    #[test]
    fn test_different_messages_different_serialization() {
        let ping = Message::new_ping(42, "test".to_string());
        let pong = Message::new_pong(42, "test".to_string());
        
        let ping_bytes = ping.serialize().expect("Failed to serialize ping");
        let pong_bytes = pong.serialize().expect("Failed to serialize pong");
        
        assert_ne!(ping_bytes, pong_bytes, "Different message types should serialize differently");
    }
}
