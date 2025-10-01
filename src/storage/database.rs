//! SQLite database management with migrations
//!
//! Provides structured storage for sessions, captures, and metadata

use crate::error::{Result, YinxError};
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::path::Path;

/// Database connection pool
pub type DbPool = Pool<SqliteConnectionManager>;

/// Database manager with migration support
pub struct Database {
    pool: DbPool,
}

impl Database {
    /// Create a new database connection
    pub fn new(db_path: &Path) -> Result<Self> {
        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to create database directory: {:?}", parent),
            })?;
        }

        // Create connection manager
        let manager = SqliteConnectionManager::file(db_path);

        // Build pool with configuration
        let pool = Pool::builder()
            .max_size(16) // Max 16 connections
            .build(manager)
            .map_err(|e| YinxError::Config(format!("Failed to create connection pool: {}", e)))?;

        // Configure connection
        {
            let conn = pool
                .get()
                .map_err(|e| YinxError::Config(format!("Failed to get connection: {}", e)))?;

            // Enable WAL mode for better concurrency
            conn.execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA foreign_keys = ON;
                PRAGMA busy_timeout = 5000;
                ",
            )?;
        }

        let db = Self { pool };

        // Run migrations
        db.migrate()?;

        Ok(db)
    }

    /// Get a connection from the pool
    pub fn get_conn(&self) -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
        self.pool
            .get()
            .map_err(|e| YinxError::Config(format!("Failed to get connection: {}", e)))
    }

    /// Run database migrations
    fn migrate(&self) -> Result<()> {
        let conn = self.get_conn()?;

        // Create migrations table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS _migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            )",
            [],
        )?;

        // Get current version
        let current_version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM _migrations",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        // Apply migrations
        for (version, migration) in MIGRATIONS.iter().enumerate() {
            let version = version as i32 + 1;

            if version > current_version {
                tracing::info!("Applying migration {}", version);

                // Execute migration
                conn.execute_batch(migration)?;

                // Record migration
                conn.execute(
                    "INSERT INTO _migrations (version, applied_at) VALUES (?1, datetime('now'))",
                    params![version],
                )?;
            }
        }

        Ok(())
    }

    /// Get database statistics
    pub fn stats(&self) -> Result<DbStats> {
        let conn = self.get_conn()?;

        let session_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))?;

        let capture_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM captures", [], |row| row.get(0))?;

        let blob_count: i64 = conn.query_row("SELECT COUNT(*) FROM blobs", [], |row| row.get(0))?;

        let chunk_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;

        let entity_count: i64 =
            conn.query_row("SELECT COUNT(*) FROM entities", [], |row| row.get(0))?;

        let total_size: i64 =
            conn.query_row("SELECT COALESCE(SUM(size), 0) FROM blobs", [], |row| {
                row.get(0)
            })?;

        Ok(DbStats {
            session_count: session_count as usize,
            capture_count: capture_count as usize,
            blob_count: blob_count as usize,
            chunk_count: chunk_count as usize,
            entity_count: entity_count as usize,
            total_size_bytes: total_size as u64,
        })
    }
}

/// Database statistics
#[derive(Debug)]
pub struct DbStats {
    pub session_count: usize,
    pub capture_count: usize,
    pub blob_count: usize,
    pub chunk_count: usize,
    pub entity_count: usize,
    pub total_size_bytes: u64,
}

/// Database migrations (each string is one migration)
const MIGRATIONS: &[&str] = &[
    // Migration 1: Initial schema
    r#"
    -- Sessions table
    CREATE TABLE sessions (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        started_at INTEGER NOT NULL,
        stopped_at INTEGER,
        status TEXT NOT NULL,
        capture_count INTEGER NOT NULL DEFAULT 0,
        blob_count INTEGER NOT NULL DEFAULT 0
    );

    CREATE INDEX idx_sessions_started_at ON sessions(started_at);
    CREATE INDEX idx_sessions_status ON sessions(status);

    -- Captures table (raw terminal data)
    CREATE TABLE captures (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL,
        timestamp INTEGER NOT NULL,
        command TEXT,
        output_hash TEXT NOT NULL,
        tool TEXT,
        exit_code INTEGER,
        cwd TEXT,
        FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
    );

    CREATE INDEX idx_captures_session ON captures(session_id);
    CREATE INDEX idx_captures_timestamp ON captures(timestamp);
    CREATE INDEX idx_captures_tool ON captures(tool);

    -- Blobs table (content-addressed storage metadata)
    CREATE TABLE blobs (
        hash TEXT PRIMARY KEY,
        size INTEGER NOT NULL,
        created_at INTEGER NOT NULL,
        compressed BOOLEAN NOT NULL,
        ref_count INTEGER NOT NULL DEFAULT 1
    );

    CREATE INDEX idx_blobs_created_at ON blobs(created_at);

    -- Chunks table (filtered/clustered content for embedding)
    CREATE TABLE chunks (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        capture_id INTEGER NOT NULL,
        blob_hash TEXT NOT NULL,
        representative_text TEXT NOT NULL,
        cluster_size INTEGER DEFAULT 1,
        metadata TEXT,  -- JSON metadata
        FOREIGN KEY (capture_id) REFERENCES captures(id) ON DELETE CASCADE,
        FOREIGN KEY (blob_hash) REFERENCES blobs(hash)
    );

    CREATE INDEX idx_chunks_capture ON chunks(capture_id);
    CREATE INDEX idx_chunks_blob ON chunks(blob_hash);

    -- Embeddings table
    CREATE TABLE embeddings (
        chunk_id INTEGER PRIMARY KEY,
        vector BLOB NOT NULL,
        model TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        FOREIGN KEY (chunk_id) REFERENCES chunks(id) ON DELETE CASCADE
    );

    CREATE INDEX idx_embeddings_model ON embeddings(model);

    -- Entities table
    CREATE TABLE entities (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        capture_id INTEGER NOT NULL,
        type TEXT NOT NULL,
        value TEXT NOT NULL,
        context TEXT,
        confidence REAL NOT NULL DEFAULT 1.0,
        FOREIGN KEY (capture_id) REFERENCES captures(id) ON DELETE CASCADE
    );

    CREATE INDEX idx_entities_capture ON entities(capture_id);
    CREATE INDEX idx_entities_type ON entities(type);
    CREATE INDEX idx_entities_value ON entities(value);
    "#,
];

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let _db = Database::new(&db_path).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn test_migrations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let _db = Database::new(&db_path).unwrap();

        // Check migrations table exists
        let conn = _db.get_conn().unwrap();
        let version: i32 = conn
            .query_row("SELECT MAX(version) FROM _migrations", [], |row| row.get(0))
            .unwrap();

        assert_eq!(version, MIGRATIONS.len() as i32);
    }

    #[test]
    fn test_schema_exists() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let db = Database::new(&db_path).unwrap();
        let conn = db.get_conn().unwrap();

        // Verify all tables exist
        let tables = vec![
            "sessions",
            "captures",
            "blobs",
            "chunks",
            "embeddings",
            "entities",
        ];

        for table in tables {
            let count: i32 = conn
                .query_row(
                    &format!(
                        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='{}'",
                        table
                    ),
                    [],
                    |row| row.get(0),
                )
                .unwrap();

            assert_eq!(count, 1, "Table {} should exist", table);
        }
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let _db = Database::new(&db_path).unwrap();
        let conn = _db.get_conn().unwrap();

        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();

        assert_eq!(fk_enabled, 1);
    }
}
