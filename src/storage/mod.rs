//! Storage layer for Yinx
//!
//! Provides content-addressed blob storage and structured database access

pub mod blob;
pub mod database;

use crate::error::Result;
use std::path::{Path, PathBuf};

pub use blob::{BlobStore, GcStats};
pub use database::{Database, DbPool, DbStats};

/// Storage manager that coordinates blob and database storage
pub struct StorageManager {
    pub blob_store: BlobStore,
    pub database: Database,
    base_path: PathBuf,
}

impl StorageManager {
    /// Create a new storage manager
    pub fn new(base_path: PathBuf) -> Result<Self> {
        // Initialize dual-zone structure
        let machine_zone = base_path.join("store");
        let human_zone = base_path.join("reports");

        // Create directories
        std::fs::create_dir_all(&machine_zone).map_err(|e| crate::error::YinxError::Io {
            source: e,
            context: format!(
                "Failed to create machine zone directory: {}",
                machine_zone.display()
            ),
        })?;
        std::fs::create_dir_all(&human_zone).map_err(|e| crate::error::YinxError::Io {
            source: e,
            context: format!(
                "Failed to create human zone directory: {}",
                human_zone.display()
            ),
        })?;
        std::fs::create_dir_all(machine_zone.join("vectors")).map_err(|e| {
            crate::error::YinxError::Io {
                source: e,
                context: "Failed to create vectors directory".to_string(),
            }
        })?;
        std::fs::create_dir_all(machine_zone.join("keywords")).map_err(|e| {
            crate::error::YinxError::Io {
                source: e,
                context: "Failed to create keywords directory".to_string(),
            }
        })?;

        // Initialize blob store
        let blob_store = BlobStore::new(machine_zone.clone(), 1024)?; // Compress if > 1KB

        // Initialize database
        let db_path = machine_zone.join("db.sqlite");
        let database = Database::new(&db_path)?;

        Ok(Self {
            blob_store,
            database,
            base_path,
        })
    }

    /// Get the machine zone path (internal, rebuildable data)
    pub fn machine_zone(&self) -> PathBuf {
        self.base_path.join("store")
    }

    /// Get the human zone path (reports, evidence, exports)
    pub fn human_zone(&self) -> PathBuf {
        self.base_path.join("reports")
    }

    /// Get path for session reports
    pub fn session_report_dir(&self, session_name: &str) -> PathBuf {
        self.human_zone().join(session_name)
    }

    /// Ensure session report directory exists
    pub fn ensure_session_report_dir(&self, session_name: &str) -> Result<PathBuf> {
        let dir = self.session_report_dir(session_name);
        std::fs::create_dir_all(&dir).map_err(|e| crate::error::YinxError::Io {
            source: e,
            context: format!(
                "Failed to create session report directory: {}",
                dir.display()
            ),
        })?;
        std::fs::create_dir_all(dir.join("evidence")).map_err(|e| crate::error::YinxError::Io {
            source: e,
            context: "Failed to create evidence directory".to_string(),
        })?;
        std::fs::create_dir_all(dir.join("export")).map_err(|e| crate::error::YinxError::Io {
            source: e,
            context: "Failed to create export directory".to_string(),
        })?;
        Ok(dir)
    }

    /// Get combined storage statistics
    pub fn stats(&self) -> Result<StorageStats> {
        let db_stats = self.database.stats()?;

        Ok(StorageStats {
            db: db_stats,
            machine_zone_size: Self::dir_size(&self.machine_zone())?,
            human_zone_size: Self::dir_size(&self.human_zone())?,
        })
    }

    /// Calculate directory size recursively
    fn dir_size(path: &Path) -> Result<u64> {
        let mut size = 0u64;

        if path.is_dir() {
            for entry in std::fs::read_dir(path).map_err(|e| crate::error::YinxError::Io {
                source: e,
                context: format!(
                    "Failed to read directory for size calculation: {}",
                    path.display()
                ),
            })? {
                let entry = entry.map_err(|e| crate::error::YinxError::Io {
                    source: e,
                    context: "Failed to read directory entry for size calculation".to_string(),
                })?;
                let path = entry.path();

                if path.is_dir() {
                    size += Self::dir_size(&path)?;
                } else {
                    size += entry
                        .metadata()
                        .map_err(|e| crate::error::YinxError::Io {
                            source: e,
                            context: format!("Failed to get file metadata: {}", path.display()),
                        })?
                        .len();
                }
            }
        }

        Ok(size)
    }
}

/// Combined storage statistics
#[derive(Debug)]
pub struct StorageStats {
    pub db: DbStats,
    pub machine_zone_size: u64,
    pub human_zone_size: u64,
}

impl StorageStats {
    /// Get total storage size
    pub fn total_size(&self) -> u64 {
        self.machine_zone_size + self.human_zone_size
    }

    /// Format size as human-readable string
    pub fn format_size(bytes: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = bytes as f64;
        let mut unit_idx = 0;

        while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
            size /= 1024.0;
            unit_idx += 1;
        }

        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_storage_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let storage = StorageManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Check dual-zone structure
        assert!(storage.machine_zone().exists());
        assert!(storage.human_zone().exists());
        assert!(storage.machine_zone().join("vectors").exists());
        assert!(storage.machine_zone().join("keywords").exists());
    }

    #[test]
    fn test_session_report_dir() {
        let temp_dir = TempDir::new().unwrap();
        let storage = StorageManager::new(temp_dir.path().to_path_buf()).unwrap();

        let session_dir = storage.ensure_session_report_dir("test_session").unwrap();

        assert!(session_dir.exists());
        assert!(session_dir.join("evidence").exists());
        assert!(session_dir.join("export").exists());
    }

    #[test]
    fn test_format_size() {
        assert_eq!(StorageStats::format_size(0), "0.00 B");
        assert_eq!(StorageStats::format_size(1023), "1023.00 B");
        assert_eq!(StorageStats::format_size(1024), "1.00 KB");
        assert_eq!(StorageStats::format_size(1024 * 1024), "1.00 MB");
        assert_eq!(StorageStats::format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_full_integration() {
        use rusqlite::params;

        let temp_dir = TempDir::new().unwrap();
        let storage = StorageManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Verify database file exists
        let db_path = storage.machine_zone().join("db.sqlite");
        assert!(db_path.exists(), "Database file should exist");

        // Test blob storage
        let test_data = b"This is test capture output from nmap scan";
        let (hash, compressed, is_new) = storage.blob_store.write(test_data).unwrap();
        assert!(is_new, "First write should be new");
        assert!(!compressed, "Small data should not be compressed");
        assert_eq!(hash.len(), 32, "Hash should be 32 characters");

        // Verify blob can be read back
        let read_data = storage.blob_store.read(&hash).unwrap();
        assert_eq!(
            test_data,
            &read_data[..],
            "Read data should match written data"
        );

        // Test database operations
        let conn = storage.database.get_conn().unwrap();

        // Insert a test session
        conn.execute(
            "INSERT INTO sessions (id, name, started_at, status, capture_count, blob_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params!["test-session-1", "Test Session", 1000000, "active", 0, 0],
        )
        .unwrap();

        // Insert a test capture
        conn.execute(
            "INSERT INTO captures (session_id, timestamp, command, output_hash, tool, exit_code)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "test-session-1",
                1000001,
                "nmap -sV 192.168.1.1",
                &hash,
                "nmap",
                0
            ],
        )
        .unwrap();

        // Insert blob metadata
        conn.execute(
            "INSERT INTO blobs (hash, size, created_at, compressed, ref_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&hash, test_data.len() as i64, 1000000, false, 1],
        )
        .unwrap();

        // Verify we can query back the data
        let session_name: String = conn
            .query_row(
                "SELECT name FROM sessions WHERE id = ?1",
                params!["test-session-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(session_name, "Test Session");

        let capture_command: String = conn
            .query_row(
                "SELECT command FROM captures WHERE session_id = ?1",
                params!["test-session-1"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(capture_command, "nmap -sV 192.168.1.1");

        // Test foreign key constraint works
        let result = conn.execute(
            "INSERT INTO captures (session_id, timestamp, command, output_hash)
             VALUES (?1, ?2, ?3, ?4)",
            params!["non-existent-session", 1000002, "test", "abc123"],
        );
        assert!(
            result.is_err(),
            "Foreign key constraint should prevent insert"
        );

        // Test storage stats
        let stats = storage.stats().unwrap();
        assert_eq!(stats.db.session_count, 1);
        assert_eq!(stats.db.capture_count, 1);
        assert_eq!(stats.db.blob_count, 1);
        assert!(stats.machine_zone_size > 0, "Machine zone should have data");

        println!("Integration test passed!");
        println!("  - Database created at: {:?}", db_path);
        println!("  - Sessions: {}", stats.db.session_count);
        println!("  - Captures: {}", stats.db.capture_count);
        println!("  - Blobs: {}", stats.db.blob_count);
        println!(
            "  - Total size: {}",
            StorageStats::format_size(stats.total_size())
        );
    }

    #[test]
    fn test_blob_compression_large_data() {
        let temp_dir = TempDir::new().unwrap();
        let storage = StorageManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Create large data that should be compressed
        let large_data = vec![b'A'; 10000];
        let (hash, compressed, is_new) = storage.blob_store.write(&large_data).unwrap();

        assert!(is_new, "Should be new blob");
        assert!(compressed, "Large data should be compressed");

        // Verify compressed size is smaller
        let blob_size = storage.blob_store.size(&hash).unwrap();
        assert!(
            blob_size < large_data.len() as u64,
            "Compressed size should be smaller"
        );

        // Verify decompression works
        let read_data = storage.blob_store.read(&hash).unwrap();
        assert_eq!(
            large_data, read_data,
            "Decompressed data should match original"
        );
    }
}
