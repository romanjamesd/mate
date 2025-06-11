use thiserror::Error;

/// Comprehensive error types for the storage layer with enhanced context and recovery information
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database connection failed: {0}")]
    ConnectionFailed(#[from] rusqlite::Error),

    #[error("Migration failed at version {version}: {message}")]
    MigrationFailed { version: i32, message: String },

    #[error("Game not found: {id}")]
    GameNotFound { id: String },

    #[error("Message not found: {id}")]
    MessageNotFound { id: String },

    #[error("Invalid data for {field}: {reason}")]
    InvalidData { field: String, reason: String },

    #[error("Serialization error for {context}: {source}")]
    SerializationError {
        context: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("IO error during {operation}: {source}")]
    IoError {
        operation: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Database path error: {message}")]
    DatabasePathError { message: String },

    #[error("Transaction failed during {operation}: {reason}")]
    TransactionFailed { operation: String, reason: String },

    #[error("Connection pool exhausted: {active_connections}/{max_connections}")]
    ConnectionPoolExhausted {
        active_connections: usize,
        max_connections: usize,
    },

    #[error("Database locked: {operation} timed out after {timeout_ms}ms")]
    DatabaseLocked { operation: String, timeout_ms: u64 },

    #[error("Schema version mismatch: current={current}, expected={expected}")]
    SchemaVersionMismatch { current: i32, expected: i32 },

    #[error("Constraint violation in {table}.{column}: {constraint}")]
    ConstraintViolation {
        table: String,
        column: String,
        constraint: String,
    },

    #[error("Database corruption detected: {details}")]
    DatabaseCorruption { details: String },

    #[error("Backup operation failed for {operation}: {reason}")]
    BackupFailed { operation: String, reason: String },

    #[error("Query timeout: {query} exceeded {timeout_ms}ms")]
    QueryTimeout { query: String, timeout_ms: u64 },

    #[error("Resource limit exceeded: {resource} ({current}/{limit})")]
    ResourceLimitExceeded {
        resource: String,
        current: u64,
        limit: u64,
    },

    #[error("Configuration error: {setting} has invalid value '{value}': {reason}")]
    ConfigurationError {
        setting: String,
        value: String,
        reason: String,
    },
}

impl StorageError {
    /// Create a new MigrationFailed error with context
    pub fn migration_failed(version: i32, message: impl Into<String>) -> Self {
        Self::MigrationFailed {
            version,
            message: message.into(),
        }
    }

    /// Create a new GameNotFound error
    pub fn game_not_found(id: impl Into<String>) -> Self {
        Self::GameNotFound { id: id.into() }
    }

    /// Create a new MessageNotFound error
    pub fn message_not_found(id: impl Into<String>) -> Self {
        Self::MessageNotFound { id: id.into() }
    }

    /// Create a new InvalidData error with context
    pub fn invalid_data(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidData {
            field: field.into(),
            reason: reason.into(),
        }
    }

    /// Create a new SerializationError with context
    pub fn serialization_error(context: impl Into<String>, source: serde_json::Error) -> Self {
        Self::SerializationError {
            context: context.into(),
            source,
        }
    }

    /// Create a new IoError with operation context
    pub fn io_error(operation: impl Into<String>, source: std::io::Error) -> Self {
        Self::IoError {
            operation: operation.into(),
            source,
        }
    }

    /// Create a new DatabasePathError
    pub fn database_path_error(message: impl Into<String>) -> Self {
        Self::DatabasePathError {
            message: message.into(),
        }
    }

    /// Create a new TransactionFailed error
    pub fn transaction_failed(operation: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::TransactionFailed {
            operation: operation.into(),
            reason: reason.into(),
        }
    }

    /// Create a new DatabaseLocked error
    pub fn database_locked(operation: impl Into<String>, timeout_ms: u64) -> Self {
        Self::DatabaseLocked {
            operation: operation.into(),
            timeout_ms,
        }
    }

    /// Create a new ConstraintViolation error
    pub fn constraint_violation(
        table: impl Into<String>,
        column: impl Into<String>,
        constraint: impl Into<String>,
    ) -> Self {
        Self::ConstraintViolation {
            table: table.into(),
            column: column.into(),
            constraint: constraint.into(),
        }
    }

    /// Create a new DatabaseCorruption error
    pub fn database_corruption(details: impl Into<String>) -> Self {
        Self::DatabaseCorruption {
            details: details.into(),
        }
    }

    /// Create a new QueryTimeout error
    pub fn query_timeout(query: impl Into<String>, timeout_ms: u64) -> Self {
        Self::QueryTimeout {
            query: query.into(),
            timeout_ms,
        }
    }

    /// Determine if this error is recoverable through retry
    pub fn is_recoverable(&self) -> bool {
        match self {
            // Connection issues are often recoverable
            StorageError::ConnectionFailed(rusqlite_err) => match rusqlite_err {
                rusqlite::Error::SqliteFailure(err, _) => match err.code {
                    rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => true,
                    _ => false,
                },
                _ => false,
            },

            // Temporary resource issues are recoverable
            StorageError::DatabaseLocked { .. }
            | StorageError::ConnectionPoolExhausted { .. }
            | StorageError::QueryTimeout { .. } => true,

            // Data corruption and schema issues are not recoverable
            StorageError::DatabaseCorruption { .. }
            | StorageError::SchemaVersionMismatch { .. }
            | StorageError::MigrationFailed { .. } => false,

            // Constraint violations and data errors are not recoverable
            StorageError::ConstraintViolation { .. }
            | StorageError::InvalidData { .. }
            | StorageError::SerializationError { .. } => false,

            // Not found errors are not recoverable through retry
            StorageError::GameNotFound { .. } | StorageError::MessageNotFound { .. } => false,

            // Configuration and path errors are not recoverable
            StorageError::ConfigurationError { .. } | StorageError::DatabasePathError { .. } => {
                false
            }

            // IO and transaction errors may be recoverable depending on cause
            StorageError::IoError { .. } | StorageError::TransactionFailed { .. } => true,

            // Backup failures are generally not recoverable
            StorageError::BackupFailed { .. } => false,

            // Resource limit errors may be recoverable after resources are freed
            StorageError::ResourceLimitExceeded { .. } => true,
        }
    }

    /// Determine if this error indicates a critical system problem
    pub fn is_critical(&self) -> bool {
        match self {
            StorageError::DatabaseCorruption { .. }
            | StorageError::SchemaVersionMismatch { .. }
            | StorageError::MigrationFailed { .. } => true,
            _ => false,
        }
    }

    /// Get the error category for logging and monitoring
    pub fn category(&self) -> ErrorCategory {
        match self {
            StorageError::ConnectionFailed(_)
            | StorageError::ConnectionPoolExhausted { .. }
            | StorageError::DatabaseLocked { .. } => ErrorCategory::Connection,

            StorageError::GameNotFound { .. } | StorageError::MessageNotFound { .. } => {
                ErrorCategory::NotFound
            }

            StorageError::InvalidData { .. }
            | StorageError::SerializationError { .. }
            | StorageError::ConstraintViolation { .. } => ErrorCategory::DataValidation,

            StorageError::MigrationFailed { .. }
            | StorageError::SchemaVersionMismatch { .. }
            | StorageError::DatabaseCorruption { .. } => ErrorCategory::Schema,

            StorageError::TransactionFailed { .. } => ErrorCategory::Transaction,

            StorageError::IoError { .. } | StorageError::DatabasePathError { .. } => {
                ErrorCategory::FileSystem
            }

            StorageError::QueryTimeout { .. } => ErrorCategory::Performance,

            StorageError::ResourceLimitExceeded { .. } => ErrorCategory::Resource,

            StorageError::ConfigurationError { .. } => ErrorCategory::Configuration,

            StorageError::BackupFailed { .. } => ErrorCategory::Backup,
        }
    }

    /// Get recovery suggestions for this error
    pub fn recovery_suggestion(&self) -> &'static str {
        match self {
            StorageError::ConnectionFailed(_) => {
                "Check database file permissions and disk space. Retry operation."
            }
            StorageError::DatabaseLocked { .. } => {
                "Another process may be using the database. Wait and retry."
            }
            StorageError::ConnectionPoolExhausted { .. } => {
                "Too many concurrent connections. Wait for connections to close and retry."
            }
            StorageError::GameNotFound { .. } | StorageError::MessageNotFound { .. } => {
                "Verify the ID is correct. The record may have been deleted."
            }
            StorageError::InvalidData { .. } => {
                "Check data format and constraints. Fix input data and retry."
            }
            StorageError::SerializationError { .. } => {
                "Data format is invalid. Check JSON structure and field types."
            }
            StorageError::MigrationFailed { .. } => {
                "Database migration failed. Check logs and consider manual intervention."
            }
            StorageError::SchemaVersionMismatch { .. } => {
                "Database schema version mismatch. Run migrations or update application."
            }
            StorageError::TransactionFailed { .. } => {
                "Transaction was rolled back. Check constraints and retry operation."
            }
            StorageError::DatabaseCorruption { .. } => {
                "Database corruption detected. Restore from backup or recreate database."
            }
            StorageError::IoError { .. } => {
                "File system error. Check disk space, permissions, and filesystem health."
            }
            StorageError::DatabasePathError { .. } => {
                "Database path is invalid. Check directory permissions and path format."
            }
            StorageError::QueryTimeout { .. } => {
                "Query took too long to execute. Optimize query or increase timeout."
            }
            StorageError::ResourceLimitExceeded { .. } => {
                "Resource limit exceeded. Wait for resources to be freed and retry."
            }
            StorageError::ConfigurationError { .. } => {
                "Configuration is invalid. Check settings and fix configuration."
            }
            StorageError::BackupFailed { .. } => {
                "Backup operation failed. Check storage space and permissions."
            }
            StorageError::ConstraintViolation { .. } => {
                "Database constraint violated. Check data values and fix input."
            }
        }
    }
}

/// Error categories for monitoring and metrics
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    Connection,
    NotFound,
    DataValidation,
    Schema,
    Transaction,
    FileSystem,
    Performance,
    Resource,
    Configuration,
    Backup,
}

impl ErrorCategory {
    /// Get a human-readable name for this category
    pub fn name(&self) -> &'static str {
        match self {
            ErrorCategory::Connection => "Connection",
            ErrorCategory::NotFound => "Not Found",
            ErrorCategory::DataValidation => "Data Validation",
            ErrorCategory::Schema => "Schema",
            ErrorCategory::Transaction => "Transaction",
            ErrorCategory::FileSystem => "File System",
            ErrorCategory::Performance => "Performance",
            ErrorCategory::Resource => "Resource",
            ErrorCategory::Configuration => "Configuration",
            ErrorCategory::Backup => "Backup",
        }
    }
}

/// Convenience type alias for storage results
pub type Result<T> = std::result::Result<T, StorageError>;

/// Error context builder for providing additional information
#[derive(Debug, Default)]
pub struct ErrorContext {
    pub operation: Option<String>,
    pub table: Option<String>,
    pub record_id: Option<String>,
    pub additional_info: Vec<(String, String)>,
}

impl ErrorContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn operation(mut self, op: impl Into<String>) -> Self {
        self.operation = Some(op.into());
        self
    }

    pub fn table(mut self, table: impl Into<String>) -> Self {
        self.table = Some(table.into());
        self
    }

    pub fn record_id(mut self, id: impl Into<String>) -> Self {
        self.record_id = Some(id.into());
        self
    }

    pub fn info(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.additional_info.push((key.into(), value.into()));
        self
    }

    /// Convert a rusqlite error to StorageError with context
    pub fn from_rusqlite_error(self, err: rusqlite::Error) -> StorageError {
        match err {
            rusqlite::Error::SqliteFailure(sqlite_err, msg) => {
                match sqlite_err.code {
                    rusqlite::ErrorCode::ConstraintViolation => StorageError::constraint_violation(
                        self.table.unwrap_or_else(|| "unknown".to_string()),
                        "unknown",
                        msg.unwrap_or_else(|| "constraint violation".to_string()),
                    ),
                    rusqlite::ErrorCode::DatabaseBusy => {
                        StorageError::database_locked(
                            self.operation.unwrap_or_else(|| "unknown".to_string()),
                            5000, // Default 5 second timeout
                        )
                    }
                    _ => StorageError::ConnectionFailed(rusqlite::Error::SqliteFailure(
                        sqlite_err, msg,
                    )),
                }
            }
            _ => StorageError::ConnectionFailed(err),
        }
    }
}
