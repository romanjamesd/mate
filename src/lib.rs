pub mod crypto;
pub mod network;
pub mod messages;

// Re-export key types for easy testing
pub use crypto::Identity;
pub use messages::{Message, SignedEnvelope};
pub use network::{Connection, Server, Client};