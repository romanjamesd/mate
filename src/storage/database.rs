use crate::storage::errors::{Result, StorageError};
use crate::storage::schema;
use directories::ProjectDirs;
use rusqlite::{Connection, Statement};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Game ID generator that creates unique, human-readable IDs
pub struct GameIdGenerator {
    counter: AtomicU32,
    peer_id_short: String,
}

impl GameIdGenerator {
    pub fn new(peer_id: &str) -> Self {
        // Use first 9 characters of peer ID to include "test_peer"
        let peer_id_short = peer_id.chars().take(9).collect();
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

/// Connection statistics for monitoring
#[derive(Debug, Default)]
pub struct ConnectionStats {
    pub operations_count: AtomicU64,
    pub transaction_count: AtomicU64,
    pub error_count: AtomicU64,
    pub total_time_ms: AtomicU64,
}

impl ConnectionStats {
    pub fn record_operation(&self, duration: Duration) {
        self.operations_count.fetch_add(1, Ordering::Relaxed);
        self.total_time_ms
            .fetch_add(duration.as_millis() as u64, Ordering::Relaxed);
    }

    pub fn record_transaction(&self) {
        self.transaction_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> (u64, u64, u64, u64) {
        (
            self.operations_count.load(Ordering::Relaxed),
            self.transaction_count.load(Ordering::Relaxed),
            self.error_count.load(Ordering::Relaxed),
            self.total_time_ms.load(Ordering::Relaxed),
        )
    }
}

/// Connection wrapper with health monitoring
struct ManagedConnection {
    conn: Connection,
    last_health_check: Instant,
}

// Safety: We ensure thread safety through the Arc<Mutex<ManagedConnection>> wrapper
// SQLite connections are safe to use across threads when properly synchronized
unsafe impl Send for ManagedConnection {}
unsafe impl Sync for ManagedConnection {}

impl ManagedConnection {
    fn new(conn: Connection) -> Self {
        Self {
            conn,
            last_health_check: Instant::now(),
        }
    }

    /// Check if the connection is healthy
    fn is_healthy(&mut self) -> bool {
        // Only check health every 30 seconds to avoid overhead
        if self.last_health_check.elapsed() < Duration::from_secs(30) {
            return true;
        }

        // Simple health check - try to execute a basic query
        let healthy = self
            .conn
            .prepare("SELECT 1")
            .and_then(|mut stmt| stmt.exists([]))
            .unwrap_or(false);

        if healthy {
            self.last_health_check = Instant::now();
        }

        healthy
    }
}

/// Main database interface with enhanced connection management
pub struct Database {
    managed_conn: Arc<Mutex<ManagedConnection>>,
    game_id_generator: GameIdGenerator,
    stats: ConnectionStats,
}

impl Database {
    /// Create a new Database instance with the given peer ID
    pub fn new(peer_id: &str) -> Result<Self> {
        let db_path = get_database_path()?;

        // Ensure the parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::database_path_error(format!(
                    "Failed to create database directory: {}",
                    e
                ))
            })?;
        }

        let conn = Self::create_optimized_connection(&db_path)?;
        let managed_conn = ManagedConnection::new(conn);

        let database = Database {
            managed_conn: Arc::new(Mutex::new(managed_conn)),
            game_id_generator: GameIdGenerator::new(peer_id),
            stats: ConnectionStats::default(),
        };

        // Initialize schema and run migrations
        database.run_migrations()?;

        Ok(database)
    }

    /// Create a connection with optimal SQLite settings
    fn create_optimized_connection(db_path: &PathBuf) -> Result<Connection> {
        let conn = Connection::open(db_path)?;

        // Apply optimal SQLite pragmas for our use case
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?; // Good balance of safety/speed
        conn.pragma_update(None, "cache_size", -64000)?; // 64MB cache
        conn.pragma_update(None, "temp_store", "memory")?; // Store temp tables in memory
        conn.pragma_update(None, "mmap_size", 268435456i64)?; // 256MB memory map

        Ok(conn)
    }

    /// Generate a new unique game ID
    pub fn generate_game_id(&self) -> String {
        self.game_id_generator.generate()
    }

    /// Run database migrations
    fn run_migrations(&self) -> Result<()> {
        let managed_conn = self.managed_conn.lock().unwrap();
        schema::initialize_schema(&managed_conn.conn)
    }

    /// Get database statistics
    pub fn get_connection_stats(&self) -> (u64, u64, u64, u64) {
        self.stats.get_stats()
    }

    /// Check connection health and attempt recovery if needed
    pub fn check_connection_health(&self) -> Result<bool> {
        let mut managed_conn = self.managed_conn.lock().unwrap();

        if !managed_conn.is_healthy() {
            self.stats.record_error();
            return Ok(false);
        }

        Ok(true)
    }

    /// Execute a closure with access to the connection
    /// This method allows controlled access to the connection while maintaining thread safety
    pub fn with_connection<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let start_time = Instant::now();
        let mut managed_conn = self.managed_conn.lock().unwrap();

        // Health check before operation
        if !managed_conn.is_healthy() {
            self.stats.record_error();
            return Err(StorageError::ConnectionFailed(
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some("Connection is not healthy".to_string()),
                ),
            ));
        }

        match f(&managed_conn.conn) {
            Ok(result) => {
                self.stats.record_operation(start_time.elapsed());
                Ok(result)
            }
            Err(e) => {
                self.stats.record_error();
                Err(e)
            }
        }
    }

    /// Execute a closure with access to prepared statements
    pub fn with_prepared_statement<T, F>(&self, sql: &str, f: F) -> Result<T>
    where
        F: FnOnce(&mut Statement) -> Result<T>,
    {
        let start_time = Instant::now();
        let mut managed_conn = self.managed_conn.lock().unwrap();

        // Health check before operation
        if !managed_conn.is_healthy() {
            self.stats.record_error();
            return Err(StorageError::ConnectionFailed(
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some("Connection is not healthy".to_string()),
                ),
            ));
        }

        // Create statement on-demand - this is safer than caching with unsound lifetimes
        let mut stmt = managed_conn
            .conn
            .prepare(sql)
            .map_err(StorageError::ConnectionFailed)?;

        match f(&mut stmt) {
            Ok(result) => {
                self.stats.record_operation(start_time.elapsed());
                Ok(result)
            }
            Err(e) => {
                self.stats.record_error();
                Err(e)
            }
        }
    }

    /// Execute a transaction with automatic rollback on error
    pub fn with_transaction<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let start_time = Instant::now();
        let mut managed_conn = self.managed_conn.lock().unwrap();

        // Health check before transaction
        if !managed_conn.is_healthy() {
            self.stats.record_error();
            return Err(StorageError::ConnectionFailed(
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_MISUSE),
                    Some("Connection is not healthy".to_string()),
                ),
            ));
        }

        let tx = managed_conn
            .conn
            .unchecked_transaction()
            .map_err(StorageError::ConnectionFailed)?;

        match f(&tx) {
            Ok(result) => {
                tx.commit().map_err(StorageError::ConnectionFailed)?;
                self.stats.record_operation(start_time.elapsed());
                self.stats.record_transaction();
                Ok(result)
            }
            Err(e) => {
                let _ = tx.rollback(); // Ignore rollback errors, return original error
                self.stats.record_error();
                Err(e)
            }
        }
    }

    /// Perform database maintenance (VACUUM, ANALYZE, etc.)
    pub fn perform_maintenance(&self) -> Result<()> {
        self.with_connection(|conn| {
            // Run ANALYZE to update query planner statistics
            conn.execute("ANALYZE", [])
                .map_err(StorageError::ConnectionFailed)?;

            // VACUUM is expensive, so we might want to make this optional
            // or run it less frequently
            // conn.execute("VACUUM", [])
            //     .map_err(|e| StorageError::ConnectionFailed(e))?;

            Ok(())
        })
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
    // Check for test override environment variable first
    if let Ok(custom_data_dir) = std::env::var("MATE_DATA_DIR") {
        let data_dir = PathBuf::from(custom_data_dir);
        return Ok(data_dir.join("database.sqlite"));
    }

    let project_dirs = ProjectDirs::from("dev", "mate", "mate").ok_or_else(|| {
        StorageError::database_path_error("Failed to determine application data directory")
    })?;

    let data_dir = project_dirs.data_dir();
    Ok(data_dir.join("database.sqlite"))
}

// Re-export for backwards compatibility
impl Database {
    /// Legacy method - use with_connection instead
    #[deprecated(note = "Use with_connection for better error handling")]
    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        // This is a hack to maintain backwards compatibility
        // In a real implementation, you'd want to phase this out
        unimplemented!("This method is deprecated, use with_connection instead")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_id_generator() {
        let generator = GameIdGenerator::new("abcdef1234567890");

        let id1 = generator.generate();
        let id2 = generator.generate();

        // IDs should be different
        assert_ne!(id1, id2);

        // Both should start with the short peer ID (9 characters)
        assert!(id1.starts_with("abcdef123"));
        assert!(id2.starts_with("abcdef123"));

        // Should have the expected format
        let parts: Vec<&str> = id1.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "abcdef123");
        assert!(parts[1].parse::<u64>().is_ok()); // timestamp
        assert!(parts[2].parse::<u32>().is_ok()); // counter
    }

    #[test]
    fn test_database_path() {
        let path = get_database_path().unwrap();
        assert!(path.to_string_lossy().contains("mate"));
        assert!(path.to_string_lossy().ends_with("database.sqlite"));
    }

    #[test]
    fn test_connection_stats() {
        let stats = ConnectionStats::default();

        stats.record_operation(Duration::from_millis(100));
        stats.record_transaction();
        stats.record_error();

        let (ops, txns, errors, time) = stats.get_stats();
        assert_eq!(ops, 1);
        assert_eq!(txns, 1);
        assert_eq!(errors, 1);
        assert!(time >= 100);
    }
}
