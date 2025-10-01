// Process management for daemon: PID files, lock files, and process checks

use crate::error::{Result, YinxError};
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Manages PID and lock files for the daemon process
#[derive(Clone)]
pub struct ProcessManager {
    pid_file: PathBuf,
    lock_file: PathBuf,
}

impl ProcessManager {
    /// Create a new ProcessManager with the given PID and lock file paths
    pub fn new(pid_file: PathBuf) -> Self {
        let lock_file = pid_file.with_extension("lock");
        Self {
            pid_file,
            lock_file,
        }
    }

    /// Check if the daemon is currently running
    pub fn is_running(&self) -> bool {
        if let Ok(pid) = self.read_pid() {
            // Check if process exists by sending signal 0
            kill(Pid::from_raw(pid), None).is_ok()
        } else {
            false
        }
    }

    /// Acquire lock and write PID file
    /// Returns an error if daemon is already running
    pub fn acquire(&self) -> Result<()> {
        // Check if already running
        if self.is_running() {
            return Err(YinxError::Daemon("Daemon is already running".to_string()));
        }

        // Try to acquire lock file
        self.acquire_lock()?;

        // Write PID file
        let pid = std::process::id();
        self.write_pid(pid)?;

        Ok(())
    }

    /// Release lock and remove PID file
    pub fn release(&self) -> Result<()> {
        // Remove PID file
        if self.pid_file.exists() {
            std::fs::remove_file(&self.pid_file).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to remove PID file: {:?}", self.pid_file),
            })?;
        }

        // Release lock file
        self.release_lock()?;

        Ok(())
    }

    /// Read PID from file
    pub fn read_pid(&self) -> Result<i32> {
        if !self.pid_file.exists() {
            return Err(YinxError::Daemon("PID file not found".to_string()));
        }

        let mut file = File::open(&self.pid_file).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to open PID file: {:?}", self.pid_file),
        })?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to read PID file".to_string(),
            })?;

        contents
            .trim()
            .parse()
            .map_err(|_| YinxError::Daemon("Invalid PID in file".to_string()))
    }

    /// Write PID to file
    fn write_pid(&self, pid: u32) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.pid_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to create directory: {:?}", parent),
            })?;
        }

        let mut file = File::create(&self.pid_file).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to create PID file: {:?}", self.pid_file),
        })?;

        file.write_all(pid.to_string().as_bytes())
            .map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to write PID to file".to_string(),
            })?;

        Ok(())
    }

    /// Acquire exclusive lock file
    fn acquire_lock(&self) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.lock_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to create directory: {:?}", parent),
            })?;
        }

        // Try to create lock file exclusively
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.lock_file)
            .map_err(|_| {
                YinxError::Daemon(
                    "Failed to acquire lock - daemon may already be running".to_string(),
                )
            })?;

        Ok(())
    }

    /// Release lock file
    fn release_lock(&self) -> Result<()> {
        if self.lock_file.exists() {
            std::fs::remove_file(&self.lock_file).map_err(|e| YinxError::Io {
                source: e,
                context: format!("Failed to remove lock file: {:?}", self.lock_file),
            })?;
        }
        Ok(())
    }

    /// Send signal to daemon process
    pub fn signal(&self, sig: Signal) -> Result<()> {
        let pid = self.read_pid()?;
        kill(Pid::from_raw(pid), sig)
            .map_err(|_| YinxError::Daemon(format!("Failed to send signal to process {}", pid)))?;
        Ok(())
    }

    /// Get the PID file path
    pub fn pid_file(&self) -> &Path {
        &self.pid_file
    }

    /// Get the lock file path
    pub fn lock_file(&self) -> &Path {
        &self.lock_file
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_process_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = temp_dir.path().join("test.pid");
        let pm = ProcessManager::new(pid_file.clone());

        assert_eq!(pm.pid_file(), pid_file);
        assert_eq!(pm.lock_file(), pid_file.with_extension("lock"));
    }

    #[test]
    fn test_not_running_initially() {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = temp_dir.path().join("test.pid");
        let pm = ProcessManager::new(pid_file);

        assert!(!pm.is_running());
    }

    #[test]
    fn test_acquire_and_release() {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = temp_dir.path().join("test.pid");
        let pm = ProcessManager::new(pid_file.clone());

        // Acquire should succeed
        pm.acquire().unwrap();

        // PID file should exist
        assert!(pid_file.exists());
        assert!(pm.lock_file().exists());

        // Should be marked as running
        assert!(pm.is_running());

        // Release should succeed
        pm.release().unwrap();

        // Files should be cleaned up
        assert!(!pid_file.exists());
        assert!(!pm.lock_file().exists());
    }

    #[test]
    fn test_cannot_acquire_twice() {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = temp_dir.path().join("test.pid");
        let pm1 = ProcessManager::new(pid_file.clone());
        let pm2 = ProcessManager::new(pid_file);

        // First acquire should succeed
        pm1.acquire().unwrap();

        // Second acquire should fail
        assert!(pm2.acquire().is_err());

        // Cleanup
        pm1.release().unwrap();
    }

    #[test]
    fn test_read_write_pid() {
        let temp_dir = TempDir::new().unwrap();
        let pid_file = temp_dir.path().join("test.pid");
        let pm = ProcessManager::new(pid_file);

        // Acquire writes PID
        pm.acquire().unwrap();

        // Read PID back
        let pid = pm.read_pid().unwrap();
        assert_eq!(pid, std::process::id() as i32);

        // Cleanup
        pm.release().unwrap();
    }
}
