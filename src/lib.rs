pub mod cli;
pub mod crypto;
pub mod messages;
pub mod network;

// Re-export key types for easy testing
pub use crypto::{Identity, PeerId};
pub use messages::{Message, SignedEnvelope};
pub use network::{Client, Connection, Server};
