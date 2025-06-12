pub mod database;
pub mod errors;
pub mod games;
pub mod messages;
pub mod models;
pub mod schema;

// Re-export key types for easy access
pub use database::Database;
pub use errors::StorageError;
pub use models::{Game, GameStatus, Message, PlayerColor};

// Re-export commonly used functions
pub use database::get_database_path;
