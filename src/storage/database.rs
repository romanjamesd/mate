use crate::storage::errors::{Result, StorageError};
use crate::storage::schema;
use directories::ProjectDirs;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Game ID generator that creates unique, human-readable IDs
pub struct GameIdGenerator {
    counter: AtomicU32,
    peer_id_short: String,
}

impl GameIdGenerator {
    pub fn new(peer_id: &str) -> Self {
        // Use first 8 characters of peer ID for readability
        let peer_id_short = peer_id.chars().take(8).collect();
        Self {
            counter: AtomicU32::new(0),
            peer_id_short,
        }
    }

    pub fn generate(&self) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let counter = self.counter.fetch_add(1, Ordering::SeqCst);

        format!("{}-{}-{:03}", self.peer_id_short, timestamp, counter)
    }
}

/// Main database interface
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    game_id_generator: GameIdGenerator,
}

impl Database {
    /// Create a new Database instance with the given peer ID
    pub fn new(peer_id: &str) -> Result<Self> {
        let db_path = get_database_path()?;

        // Ensure the parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::DatabasePathError(format!(
                    "Failed to create database directory: {}",
                    e
                ))
            })?;
        }

        let conn = Connection::open(&db_path)?;

        let database = Database {
            conn: Arc::new(Mutex::new(conn)),
            game_id_generator: GameIdGenerator::new(peer_id),
        };

        // Initialize schema and run migrations
        database.run_migrations()?;

        Ok(database)
    }

    /// Generate a new unique game ID
    pub fn generate_game_id(&self) -> String {
        self.game_id_generator.generate()
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        schema::initialize_schema(&*conn)
    }

    /// Get a reference to the connection for operations
    /// This method allows controlled access to the connection while maintaining thread safety
    pub fn with_connection<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().unwrap();
        f(&*conn)
    }

    /// Execute a transaction with automatic rollback on error
    pub fn with_transaction<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let conn = self.conn.lock().unwrap();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| StorageError::ConnectionFailed(e))?;

        match f(&tx) {
            Ok(result) => {
                tx.commit().map_err(|e| StorageError::ConnectionFailed(e))?;
                Ok(result)
            }
            Err(e) => {
                let _ = tx.rollback(); // Ignore rollback errors, return original error
                Err(e)
            }
        }
    }

    /// Get current Unix timestamp
    pub fn current_timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

/// Get the appropriate database path for the current platform
pub fn get_database_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from("dev", "mate", "mate").ok_or_else(|| {
        StorageError::DatabasePathError(
            "Failed to determine application data directory".to_string(),
        )
    })?;

    let data_dir = project_dirs.data_dir();
    Ok(data_dir.join("database.sqlite"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_game_id_generator() {
        let generator = GameIdGenerator::new("abcdef1234567890");

        let id1 = generator.generate();
        let id2 = generator.generate();

        // IDs should be different
        assert_ne!(id1, id2);

        // Both should start with the short peer ID
        assert!(id1.starts_with("abcdef12"));
        assert!(id2.starts_with("abcdef12"));

        // Should have the expected format
        let parts: Vec<&str> = id1.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "abcdef12");
        assert!(parts[1].parse::<u64>().is_ok()); // timestamp
        assert!(parts[2].parse::<u32>().is_ok()); // counter
    }

    #[test]
    fn test_database_path() {
        let path = get_database_path().unwrap();
        assert!(path.to_string_lossy().contains("mate"));
        assert!(path.to_string_lossy().ends_with("database.sqlite"));
    }
}
