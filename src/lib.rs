pub mod chess;
pub mod cli;
pub mod crypto;
pub mod messages;
pub mod network;
pub mod storage;

// Re-export key types for easy testing (preserve existing + add chess)
pub use chess::{ChessError, Color, Move, Piece, PieceType, Position};
pub use crypto::{Identity, PeerId};
pub use messages::{Message, SignedEnvelope};
pub use network::{Client, Connection, Server};
pub use storage::{Database, StorageError};
