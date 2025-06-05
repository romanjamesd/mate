pub mod client;
pub mod connection;
pub mod server;

pub use client::Client;
pub use connection::{Connection, ConnectionError};
pub use server::Server;

// Re-export wire protocol types for convenience
pub use crate::messages::wire::{WireConfig, WireProtocolError};
