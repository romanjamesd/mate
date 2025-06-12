use crate::storage::errors::{Result, StorageError};
use rusqlite::Connection;

pub const CURRENT_SCHEMA_VERSION: i32 = 1;

/// Migration represents a single database migration
pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub sql: &'static str,
}

/// All database migrations in order
pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    description: "Initial schema with games and messages tables",
    sql: r#"
            -- Games table
            CREATE TABLE games (
                id TEXT PRIMARY KEY,
                opponent_peer_id TEXT NOT NULL,
                my_color TEXT NOT NULL CHECK(my_color IN ('white', 'black')),
                status TEXT NOT NULL CHECK(status IN ('pending', 'active', 'completed', 'abandoned')),
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                completed_at INTEGER,
                result TEXT CHECK(result IN ('win', 'loss', 'draw', 'abandoned')),
                metadata TEXT
            );

            -- Messages table
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL,
                message_type TEXT NOT NULL,
                content TEXT NOT NULL,
                signature TEXT NOT NULL,
                sender_peer_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (game_id) REFERENCES games(id) ON DELETE CASCADE
            );

            -- Schema migrations tracking table
            CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL,
                description TEXT NOT NULL
            );

            -- Indexes for performance
            CREATE INDEX idx_games_opponent ON games(opponent_peer_id);
            CREATE INDEX idx_games_status ON games(status);
            CREATE INDEX idx_games_created ON games(created_at DESC);
            CREATE INDEX idx_messages_game ON messages(game_id, created_at);
            CREATE INDEX idx_messages_type ON messages(message_type);
            CREATE INDEX idx_messages_sender ON messages(sender_peer_id);
        "#,
}];

/// Initialize the database schema and run any pending migrations
pub fn initialize_schema(conn: &Connection) -> Result<()> {
    // Enable important SQLite features
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|e| {
            StorageError::migration_failed(0, format!("Failed to enable foreign keys: {}", e))
        })?;

    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| {
            StorageError::migration_failed(0, format!("Failed to enable WAL mode: {}", e))
        })?;

    // Check if schema_migrations table exists
    let migrations_exist = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='schema_migrations'")
        .and_then(|mut stmt| stmt.exists([]))
        .unwrap_or(false);

    if !migrations_exist {
        // First time setup - run all migrations
        run_all_migrations(conn)?;
    } else {
        // Run any pending migrations
        run_pending_migrations(conn)?;
    }

    Ok(())
}

/// Run all migrations from scratch
fn run_all_migrations(conn: &Connection) -> Result<()> {
    let tx = conn.unchecked_transaction().map_err(|e| {
        StorageError::migration_failed(-1, format!("Failed to start transaction: {}", e))
    })?;

    for migration in MIGRATIONS {
        execute_migration(&tx, migration)?;
    }

    tx.commit().map_err(|e| {
        StorageError::migration_failed(-1, format!("Failed to commit migrations: {}", e))
    })?;

    Ok(())
}

/// Run any pending migrations
fn run_pending_migrations(conn: &Connection) -> Result<()> {
    let current_version = get_current_version(conn)?;

    let pending_migrations: Vec<&Migration> = MIGRATIONS
        .iter()
        .filter(|m| m.version > current_version)
        .collect();

    if pending_migrations.is_empty() {
        return Ok(());
    }

    let tx = conn.unchecked_transaction().map_err(|e| {
        StorageError::migration_failed(-1, format!("Failed to start transaction: {}", e))
    })?;

    for migration in pending_migrations {
        execute_migration(&tx, migration)?;
    }

    tx.commit().map_err(|e| {
        StorageError::migration_failed(-1, format!("Failed to commit migrations: {}", e))
    })?;

    Ok(())
}

/// Execute a single migration
fn execute_migration(conn: &Connection, migration: &Migration) -> Result<()> {
    conn.execute_batch(migration.sql).map_err(|e| {
        StorageError::migration_failed(
            migration.version,
            format!("Failed to execute migration {}: {}", migration.version, e),
        )
    })?;

    // Record the migration
    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at, description) VALUES (?1, ?2, ?3)",
        (
            migration.version,
            current_timestamp(),
            migration.description,
        ),
    )
    .map_err(|e| {
        StorageError::migration_failed(
            migration.version,
            format!("Failed to record migration {}: {}", migration.version, e),
        )
    })?;

    Ok(())
}

/// Get the current schema version
fn get_current_version(conn: &Connection) -> Result<i32> {
    let version = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get::<_, Option<i32>>(0)
        })
        .map_err(|e| {
            StorageError::migration_failed(-1, format!("Failed to get current version: {}", e))
        })?
        .unwrap_or(0);

    Ok(version)
}

/// Get current Unix timestamp
fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
