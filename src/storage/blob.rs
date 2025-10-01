//! Content-addressed blob storage with BLAKE3 hashing
//!
//! Provides deduplication and efficient storage of capture outputs

use crate::error::{Result, YinxError};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Content-addressed blob storage
pub struct BlobStore {
    base_path: PathBuf,
    compression_enabled: bool,
    compression_threshold: usize,
}

impl BlobStore {
    /// Create a new blob store at the given base path
    pub fn new(base_path: PathBuf, compression_threshold: usize) -> Result<Self> {
        // Create blobs directory if it doesn't exist
        let blobs_dir = base_path.join("blobs");
        fs::create_dir_all(&blobs_dir).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to create blobs directory: {}", blobs_dir.display()),
        })?;

        Ok(Self {
            base_path,
            compression_enabled: true,
            compression_threshold,
        })
    }

    /// Write data to blob storage, returning the hash
    /// Returns (hash, was_compressed, was_new)
    pub fn write(&self, data: &[u8]) -> Result<(String, bool, bool)> {
        // Calculate BLAKE3 hash
        let hash = self.hash_data(data);

        // Check if blob already exists
        let blob_path = self.blob_path(&hash);
        if blob_path.exists() {
            return Ok((hash, false, false));
        }

        // Decide whether to compress
        let should_compress = self.compression_enabled && data.len() >= self.compression_threshold;

        // Write to temporary file first (atomic write)
        let temp_path = self.temp_path(&hash);
        let parent = temp_path
            .parent()
            .ok_or_else(|| YinxError::Config("Invalid blob path".to_string()))?;
        fs::create_dir_all(parent).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to create parent directory: {}", parent.display()),
        })?;

        let mut file = fs::File::create(&temp_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to create temp blob file: {}", temp_path.display()),
        })?;

        if should_compress {
            // Compress with zstd
            let compressed = zstd::encode_all(data, 3).map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to compress blob data".to_string(),
            })?;
            file.write_all(&compressed).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to write compressed blob: {}", temp_path.display()),
            })?;
        } else {
            file.write_all(data).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to write blob data: {}", temp_path.display()),
            })?;
        }

        file.sync_all().map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to sync blob file: {}", temp_path.display()),
        })?;
        drop(file);

        // Atomically rename to final location
        let final_parent = blob_path
            .parent()
            .ok_or_else(|| YinxError::Config("Invalid blob path".to_string()))?;
        fs::create_dir_all(final_parent).map_err(|e| YinxError::Io {
            source: e,
            context: format!(
                "Failed to create blob parent directory: {}",
                final_parent.display()
            ),
        })?;
        fs::rename(&temp_path, &blob_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!(
                "Failed to rename temp blob to final location: {} -> {}",
                temp_path.display(),
                blob_path.display()
            ),
        })?;

        Ok((hash, should_compress, true))
    }

    /// Read data from blob storage
    pub fn read(&self, hash: &str) -> Result<Vec<u8>> {
        let blob_path = self.blob_path(hash);

        if !blob_path.exists() {
            return Err(YinxError::Config(format!("Blob not found: {}", hash)));
        }

        let mut file = fs::File::open(&blob_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to open blob file: {}", blob_path.display()),
        })?;
        let mut data = Vec::new();
        file.read_to_end(&mut data).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to read blob data: {}", blob_path.display()),
        })?;

        // Try to decompress (if it fails, assume it wasn't compressed)
        match zstd::decode_all(&data[..]) {
            Ok(decompressed) => Ok(decompressed),
            Err(_) => Ok(data), // Not compressed or decompression failed
        }
    }

    /// Check if a blob exists
    pub fn exists(&self, hash: &str) -> bool {
        self.blob_path(hash).exists()
    }

    /// Delete a blob (use carefully - should only be called after checking ref count)
    pub fn delete(&self, hash: &str) -> Result<()> {
        let blob_path = self.blob_path(hash);
        if blob_path.exists() {
            fs::remove_file(&blob_path).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to delete blob: {}", blob_path.display()),
            })?;
        }
        Ok(())
    }

    /// Get the size of a blob
    pub fn size(&self, hash: &str) -> Result<u64> {
        let blob_path = self.blob_path(hash);
        let metadata = fs::metadata(&blob_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to get blob metadata: {}", blob_path.display()),
        })?;
        Ok(metadata.len())
    }

    /// Hash data using BLAKE3
    fn hash_data(&self, data: &[u8]) -> String {
        let hash = blake3::hash(data);
        // Use 32 hex characters (16 bytes) for reasonable uniqueness
        format!("{:.32}", hash.to_hex())
    }

    /// Get the path for a blob given its hash
    /// Uses two-level sharding: blobs/ab/cd/abcdef123456...
    fn blob_path(&self, hash: &str) -> PathBuf {
        let shard1 = &hash[0..2];
        let shard2 = &hash[2..4];
        self.base_path
            .join("blobs")
            .join(shard1)
            .join(shard2)
            .join(hash)
    }

    /// Get temporary path for atomic writes
    fn temp_path(&self, hash: &str) -> PathBuf {
        let shard1 = &hash[0..2];
        let shard2 = &hash[2..4];
        self.base_path
            .join("blobs")
            .join(shard1)
            .join(shard2)
            .join(format!("{}.tmp", hash))
    }

    /// Garbage collect unused blobs (should be called with ref counts from database)
    pub fn gc(&self, referenced_hashes: &[String]) -> Result<GcStats> {
        let mut stats = GcStats::default();

        // Walk through all blobs
        self.walk_blobs(|hash, path| {
            stats.total_blobs += 1;

            if !referenced_hashes.contains(&hash.to_string()) {
                // Blob is not referenced, delete it
                if let Ok(metadata) = fs::metadata(path) {
                    stats.freed_bytes += metadata.len();
                }

                if fs::remove_file(path).is_ok() {
                    stats.deleted_blobs += 1;
                }
            }

            Ok(())
        })?;

        Ok(stats)
    }

    /// Walk through all blobs in storage
    fn walk_blobs<F>(&self, mut callback: F) -> Result<()>
    where
        F: FnMut(&str, &Path) -> Result<()>,
    {
        let blobs_dir = self.base_path.join("blobs");

        if !blobs_dir.exists() {
            return Ok(());
        }

        for shard1 in fs::read_dir(&blobs_dir).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to read blobs directory: {}", blobs_dir.display()),
        })? {
            let shard1 = shard1.map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to read shard1 directory entry".to_string(),
            })?;
            if !shard1.path().is_dir() {
                continue;
            }

            for shard2 in fs::read_dir(shard1.path()).map_err(|e| YinxError::Io {
                source: e,
                context: format!(
                    "Failed to read shard1 directory: {}",
                    shard1.path().display()
                ),
            })? {
                let shard2 = shard2.map_err(|e| YinxError::Io {
                    source: e,
                    context: "Failed to read shard2 directory entry".to_string(),
                })?;
                if !shard2.path().is_dir() {
                    continue;
                }

                for entry in fs::read_dir(shard2.path()).map_err(|e| YinxError::Io {
                    source: e,
                    context: format!(
                        "Failed to read shard2 directory: {}",
                        shard2.path().display()
                    ),
                })? {
                    let entry = entry.map_err(|e| YinxError::Io {
                        source: e,
                        context: "Failed to read blob entry".to_string(),
                    })?;
                    let path = entry.path();

                    if path.is_file() {
                        if let Some(filename) = path.file_name() {
                            if let Some(hash) = filename.to_str() {
                                // Skip temporary files
                                if !hash.ends_with(".tmp") {
                                    callback(hash, &path)?;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Statistics from garbage collection
#[derive(Debug, Default)]
pub struct GcStats {
    pub total_blobs: usize,
    pub deleted_blobs: usize,
    pub freed_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_blob_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf(), 1024).unwrap();

        let data = b"Hello, World!";
        let (hash, compressed, is_new) = store.write(data).unwrap();

        assert!(is_new);
        assert!(!compressed); // Too small to compress

        let read_data = store.read(&hash).unwrap();
        assert_eq!(data, &read_data[..]);
    }

    #[test]
    fn test_blob_deduplication() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf(), 1024).unwrap();

        let data = b"Test data";

        let (hash1, _, is_new1) = store.write(data).unwrap();
        assert!(is_new1);

        let (hash2, _, is_new2) = store.write(data).unwrap();
        assert!(!is_new2);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_blob_compression() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf(), 10).unwrap();

        // Large enough to trigger compression
        let data = vec![b'A'; 2000];
        let (hash, compressed, _) = store.write(&data).unwrap();

        assert!(compressed);

        let read_data = store.read(&hash).unwrap();
        assert_eq!(data, read_data);
    }

    #[test]
    fn test_blob_exists() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf(), 1024).unwrap();

        let data = b"Exists test";
        let (hash, _, _) = store.write(data).unwrap();

        assert!(store.exists(&hash));
        assert!(!store.exists("nonexistent_hash"));
    }

    #[test]
    fn test_blob_path_sharding() {
        let temp_dir = TempDir::new().unwrap();
        let store = BlobStore::new(temp_dir.path().to_path_buf(), 1024).unwrap();

        let hash = "abcdef1234567890";
        let path = store.blob_path(hash);

        let path_str = path.to_str().unwrap();
        assert!(path_str.contains("/blobs/ab/cd/"));
    }
}
