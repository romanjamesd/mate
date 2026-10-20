use crate::storage::errors::{Result, StorageError};
use rusqlite::Connection;

pub const CURRENT_SCHEMA_VERSION: i32 = 3;

/// Migration represents a single database migration
pub struct Migration {
    pub version: i32,
    pub description: &'static str,
    pub sql: &'static str,
    pub precheck: Option<fn(&Connection) -> Result<()>>,
}

/// All database migrations in order
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        precheck: None,
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
        precheck: None,
        description: "Order per-game message reads by insertion id",
        sql: r#"
            DROP INDEX IF EXISTS idx_messages_game;
            CREATE INDEX IF NOT EXISTS idx_messages_game_order
                ON messages(game_id, id);
        "#,
    },
    Migration {
        version: 3,
        description: "Require explicit ply for chess moves",
        precheck: Some(reject_legacy_moves),
        sql: r#"
            ALTER TABLE messages ADD COLUMN ply INTEGER CHECK (
                (message_type = 'Move' AND ply IS NOT NULL
                 AND typeof(ply) = 'integer' AND ply BETWEEN 1 AND 4294967295)
                OR (message_type <> 'Move' AND ply IS NULL)
            );
            CREATE UNIQUE INDEX idx_messages_game_ply
                ON messages(game_id, ply) WHERE message_type = 'Move';
        "#,
    },
];

fn reject_legacy_moves(conn: &Connection) -> Result<()> {
    let (rows, games): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COUNT(DISTINCT game_id) FROM messages WHERE message_type = 'Move'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| {
            StorageError::migration_failed(3, format!("Failed to inspect legacy moves: {e}"))
        })?;
    if rows != 0 {
        let path: String = conn
            .query_row(
                "SELECT file FROM pragma_database_list WHERE name = 'main'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| {
                StorageError::migration_failed(3, format!("Failed to locate database: {e}"))
            })?;
        let location = if path.is_empty() {
            "in-memory database"
        } else {
            &path
        };
        return Err(StorageError::migration_failed(3, format!(
            "This development database predates required move sequencing: {rows} Move rows; affected games: {games}. Recreate the database at {location} (the default path is selected by get_database_path() / MATE_DATA_DIR). No data was changed."
        )));
    }
    Ok(())
}

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
    if let Some(precheck) = migration.precheck {
        precheck(conn)?;
    }
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

    fn assert_current_schema(conn: &Connection) {
        assert_eq!(get_current_version(conn).unwrap(), CURRENT_SCHEMA_VERSION);
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
        let index_sql: String = conn
            .query_row(
                "SELECT sql FROM sqlite_master WHERE name = 'idx_messages_game_ply'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(index_sql.contains("CREATE UNIQUE INDEX"));
        assert!(index_sql.contains("WHERE message_type = 'Move'"));
        assert!(conn
            .prepare("SELECT 1 FROM pragma_table_info('messages') WHERE name = 'ply'")
            .unwrap()
            .exists([])
            .unwrap());
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
        assert_current_schema(&conn);
    }

    #[test]
    fn version_one_upgrade_preserves_messages_and_autoincrement_sequence() {
        let conn = version_one_database();
        conn.execute_batch(
            r#"
            INSERT INTO games VALUES
                ('game', 'opponent', 'white', 'active', 100, 100, NULL, NULL, NULL);
            INSERT INTO messages VALUES
                (1, 'game', 'GameInvite', 'first', 'sig1', 'sender', 300),
                (2, 'game', 'GameAccept', 'second', 'sig2', 'opponent', 200),
                (3, 'game', 'Chat', 'deleted', 'sig3', 'sender', 100);
            DELETE FROM messages WHERE id = 3;
            CREATE TEMP TABLE original_games AS SELECT * FROM games;
            CREATE TEMP TABLE original_messages AS SELECT *, NULL AS ply FROM messages;
            "#,
        )
        .unwrap();

        initialize_schema(&conn).unwrap();
        assert_current_schema(&conn);

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
             VALUES ('game', 'Chat', 'next', 'sig4', 'sender', 50)",
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
        assert_current_schema(&conn);
        let recorded: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(recorded, 3);
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
        assert_current_schema(&conn);
    }
    fn version_two_database() -> Connection {
        let conn = version_one_database();
        execute_migration(&conn, &MIGRATIONS[1]).unwrap();
        conn
    }

    #[test]
    fn version_two_move_free_upgrade_preserves_data_and_sequence() {
        let conn = version_two_database();
        conn.execute_batch(
            "INSERT INTO games VALUES ('game', 'peer', 'white', 'pending', 1, 2, NULL, NULL, NULL);
             INSERT INTO messages VALUES (1, 'game', 'GameInvite', 'invite', 'sig', 'peer', 3),
                                         (2, 'game', 'GameAccept', 'accept', 'sig2', 'self', 4),
                                         (7, 'game', 'Chat', 'deleted', 'sig3', 'self', 5);
             DELETE FROM messages WHERE id = 7;
             CREATE TEMP TABLE original_games AS SELECT * FROM games;
             CREATE TEMP TABLE original_messages AS SELECT *, NULL AS ply FROM messages;",
        )
        .unwrap();
        initialize_schema(&conn).unwrap();
        assert_current_schema(&conn);
        for table in ["games", "messages"] {
            for (left, right) in [
                (table.to_string(), format!("original_{table}")),
                (format!("original_{table}"), table.to_string()),
            ] {
                assert!(!conn
                    .prepare(&format!(
                        "SELECT * FROM {left} EXCEPT SELECT * FROM {right}"
                    ))
                    .unwrap()
                    .exists([])
                    .unwrap());
            }
        }
        conn.execute("INSERT INTO messages (game_id, message_type, content, signature, sender_peer_id, created_at)
                      VALUES ('game', 'Chat', 'next', 'sig', 'self', 6)", []).unwrap();
        assert_eq!(conn.last_insert_rowid(), 8);
    }

    #[test]
    fn legacy_moves_refuse_upgrade_without_changes() {
        let conn = version_two_database();
        conn.execute_batch(
            "INSERT INTO games VALUES ('game', 'peer', 'white', 'active', 1, 2, NULL, NULL, NULL);
             INSERT INTO messages VALUES (1, 'game', 'Move', 'legacy', 'sig', 'peer', 3),
                                         (2, 'game', 'Move', 'legacy2', 'sig2', 'self', 4);
             CREATE TEMP TABLE original_messages AS SELECT * FROM messages;
             CREATE TEMP TABLE original_games AS SELECT * FROM games;",
        )
        .unwrap();
        let error = initialize_schema(&conn).unwrap_err();
        assert!(matches!(
            error,
            StorageError::MigrationFailed { version: 3, .. }
        ));
        let message = error.to_string();
        assert!(message.contains("2 Move rows; affected games: 1"));
        assert!(message.contains("Recreate the database"));
        assert_eq!(get_current_version(&conn).unwrap(), 2);
        assert!(!index_exists(&conn, "idx_messages_game_ply"));
        assert!(!conn
            .prepare("SELECT 1 FROM pragma_table_info('messages') WHERE name = 'ply'")
            .unwrap()
            .exists([])
            .unwrap());
        for table in ["games", "messages"] {
            for (left, right) in [
                (table.to_string(), format!("original_{table}")),
                (format!("original_{table}"), table.to_string()),
            ] {
                assert!(!conn
                    .prepare(&format!(
                        "SELECT * FROM {left} EXCEPT SELECT * FROM {right}"
                    ))
                    .unwrap()
                    .exists([])
                    .unwrap());
            }
        }
        // The SQL constraint is also a backstop if the Rust precheck is bypassed.
        assert!(conn.execute_batch(MIGRATIONS[2].sql).is_err());
    }

    #[test]
    fn raw_sql_enforces_move_ply_constraints() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO games VALUES
            ('game', 'peer', 'white', 'active', 1, 1, NULL, NULL, NULL),
            ('other', 'peer', 'black', 'active', 1, 1, NULL, NULL, NULL);",
        )
        .unwrap();
        let insert = |game: &str, kind: &str, ply: &str| {
            conn.execute_batch(&format!(
            "INSERT INTO messages (game_id, message_type, content, signature, sender_peer_id, created_at, ply)
             VALUES ('{game}', '{kind}', '{{}}', 'sig', 'peer', 1, {ply});"
        ))
        };
        for ply in ["NULL", "0", "-1", "4294967296", "1.5", "X'01'", "'invalid'"] {
            assert!(insert("game", "Move", ply).is_err(), "accepted {ply}");
        }
        assert!(insert("game", "GameAccept", "1").is_err());
        insert("game", "GameAccept", "NULL").unwrap();
        insert("game", "Move", "1").unwrap();
        assert!(insert("game", "Move", "1").is_err());
        insert("other", "Move", "1").unwrap();
        insert("game", "Move", "4294967295").unwrap();
        // INTEGER affinity intentionally accepts numeric text after coercion.
        insert("game", "Move", "'2'").unwrap();
    }

    #[test]
    fn move_history_and_lookup_use_partial_index() {
        let conn = Connection::open_in_memory().unwrap();
        initialize_schema(&conn).unwrap();
        for sql in [
            "SELECT id, game_id, ply, content, signature, sender_peer_id, created_at
             FROM messages WHERE game_id = 'game' AND message_type = 'Move' ORDER BY ply ASC",
            "SELECT id, game_id, ply, content, signature, sender_peer_id, created_at
             FROM messages WHERE game_id = 'game' AND message_type = 'Move' AND ply = 1",
        ] {
            let plan: Vec<String> = conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map([], |row| row.get(3))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap();
            assert!(
                plan.iter()
                    .any(|line| line.contains("idx_messages_game_ply")),
                "{plan:?}"
            );
            assert!(
                !plan.iter().any(|line| line.contains("TEMP B-TREE")),
                "{plan:?}"
            );
        }
    }
}
