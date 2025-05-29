pub mod connection;
pub mod server;
pub mod client;

pub use connection::{Connection, ConnectionError};
pub use server::Server;
pub use client::Client;

// Re-export wire protocol types for convenience
pub use crate::messages::wire::{WireConfig, WireProtocolError};
