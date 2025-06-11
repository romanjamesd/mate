use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database connection failed: {0}")]
    ConnectionFailed(#[from] rusqlite::Error),

    #[error("Migration failed: {0}")]
    MigrationFailed(String),

    #[error("Game not found: {0}")]
    GameNotFound(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Database path error: {0}")]
    DatabasePathError(String),
}

pub type Result<T> = std::result::Result<T, StorageError>;
