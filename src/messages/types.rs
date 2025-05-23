use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Ping { nonce: u64, payload: String },
    Pong { nonce: u64, payload: String },
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
