use crate::storage::errors::{Result, StorageError};
use rusqlite::Connection;

pub const CURRENT_SCHEMA_VERSION: i32 = 2;

/// Migration represents a single database migration
pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub sql: &'static str,
}

/// All database migrations in order
pub const MIGRATIONS: &[Migration] = &[
    Migration {
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
    },
    Migration {
        version: 2,
        description: "Order per-game message reads by insertion id",
        sql: r#"
            DROP INDEX IF EXISTS idx_messages_game;
            CREATE INDEX IF NOT EXISTS idx_messages_game_order
                ON messages(game_id, id);
        "#,
    },
];

/// Initialize the database schema and run any pending migrations
pub fn initialize_schema(conn: &Connection) -> Result<()> {
    // Enable important SQLite features
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|e| {
            StorageError::migration_failed(0, format!("Failed to enable foreign keys: {e}"))
        })?;

    // Don't override journal mode here - let the connection setup handle it
    // The journal mode was already set in create_optimized_connection based on environment

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
        StorageError::migration_failed(-1, format!("Failed to start transaction: {e}"))
    })?;

    for migration in MIGRATIONS {
        execute_migration(&tx, migration)?;
    }

    tx.commit().map_err(|e| {
        StorageError::migration_failed(-1, format!("Failed to commit migrations: {e}"))
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
        StorageError::migration_failed(-1, format!("Failed to start transaction: {e}"))
    })?;

    for migration in pending_migrations {
        execute_migration(&tx, migration)?;
    }

    tx.commit().map_err(|e| {
        StorageError::migration_failed(-1, format!("Failed to commit migrations: {e}"))
    })?;

    Ok(())
}

/// Execute a single migration
fn execute_migration(conn: &Connection, migration: &Migration) -> Result<()> {
    conn.execute_batch(migration.sql).map_err(|e| {
        let version = migration.version;
        StorageError::migration_failed(
            migration.version,
            format!("Failed to execute migration {version}: {e}"),
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
        let version = migration.version;
        StorageError::migration_failed(
            migration.version,
            format!("Failed to record migration {version}: {e}"),
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
            StorageError::migration_failed(-1, format!("Failed to get current version: {e}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn version_one_database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        execute_migration(&conn, &MIGRATIONS[0]).unwrap();
        conn
    }

    fn index_exists(conn: &Connection, name: &str) -> bool {
        conn.prepare("SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1")
            .unwrap()
            .exists([name])
            .unwrap()
    }

    fn assert_version_two_schema(conn: &Connection) {
        assert_eq!(get_current_version(conn).unwrap(), 2);
        assert!(!index_exists(conn, "idx_messages_game"));
        assert!(index_exists(conn, "idx_messages_game_order"));

        let columns: Vec<String> = conn
            .prepare("PRAGMA index_info('idx_messages_game_order')")
            .unwrap()
            .query_map([], |row| row.get(2))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(columns, ["game_id", "id"]);
    }

    #[test]
    fn migration_versions_are_increasing_and_match_current_version() {
        assert_eq!(CURRENT_SCHEMA_VERSION, MIGRATIONS.last().unwrap().version);
        assert!(MIGRATIONS
            .windows(2)
            .all(|pair| pair[0].version < pair[1].version));
    }

    #[test]
    fn fresh_database_has_insertion_order_index() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();
        assert_version_two_schema(&conn);
    }

    #[test]
    fn version_one_upgrade_preserves_messages_and_autoincrement_sequence() {
        let conn = version_one_database();
        conn.execute_batch(
            r#"
            INSERT INTO games VALUES
                ('game', 'opponent', 'white', 'active', 100, 100, NULL, NULL, NULL);
            INSERT INTO messages VALUES
                (1, 'game', 'Move', 'first', 'sig1', 'sender', 300),
                (2, 'game', 'Move', 'second', 'sig2', 'opponent', 200),
                (3, 'game', 'Move', 'deleted', 'sig3', 'sender', 100);
            DELETE FROM messages WHERE id = 3;
            CREATE TEMP TABLE original_games AS SELECT * FROM games;
            CREATE TEMP TABLE original_messages AS SELECT * FROM messages;
            "#,
        )
        .unwrap();

        initialize_schema(&conn).unwrap();
        assert_version_two_schema(&conn);

        for table in ["games", "messages"] {
            // Compare every column in both directions to detect lost, added, or changed rows.
            for (left, right) in [
                (table.to_string(), format!("original_{table}")),
                (format!("original_{table}"), table.to_string()),
            ] {
                let changed = conn
                    .prepare(&format!(
                        "SELECT * FROM {left} EXCEPT SELECT * FROM {right}"
                    ))
                    .unwrap()
                    .exists([])
                    .unwrap();
                assert!(!changed, "migration changed {table} rows");
            }
        }

        conn.execute(
            "INSERT INTO messages (game_id, message_type, content, signature, sender_peer_id, created_at)
             VALUES ('game', 'Move', 'next', 'sig4', 'sender', 50)",
            [],
        )
        .unwrap();
        assert_eq!(conn.last_insert_rowid(), 4);
    }

    #[test]
    fn repeated_initialization_does_not_reapply_migrations() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();
        initialize_schema(&conn).unwrap();
        assert_version_two_schema(&conn);
        let recorded: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(recorded, 2);
    }

    #[test]
    fn failed_upgrade_restores_old_index_and_version() {
        let conn = version_one_database();
        // Fail after the index swap, while recording migration 2.
        conn.execute_batch(
            "CREATE TRIGGER reject_version_two BEFORE INSERT ON schema_migrations
             WHEN NEW.version = 2 BEGIN SELECT RAISE(ABORT, 'test migration failure'); END;",
        )
        .unwrap();

        let error = initialize_schema(&conn).unwrap_err();
        assert!(matches!(
            error,
            StorageError::MigrationFailed { version: 2, .. }
        ));
        assert_eq!(get_current_version(&conn).unwrap(), 1);
        assert!(index_exists(&conn, "idx_messages_game"));
        assert!(!index_exists(&conn, "idx_messages_game_order"));

        conn.execute_batch("DROP TRIGGER reject_version_two")
            .unwrap();
        initialize_schema(&conn).unwrap();
        assert_version_two_schema(&conn);
    }
}
