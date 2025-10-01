//! Session management
//!
//! Handles creation, storage, and lifecycle management of capture sessions
use crate::error::{Result, YinxError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Session is actively capturing
    Active,
    /// Session is paused
    Paused,
    /// Session has been stopped
    Stopped,
}

/// A capture session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: Uuid,

    /// Human-readable session name
    pub name: String,

    /// When the session was started
    pub started_at: DateTime<Utc>,

    /// When the session was stopped (if applicable)
    pub stopped_at: Option<DateTime<Utc>>,

    /// Number of captures in this session
    pub capture_count: u64,

    /// Number of unique blobs stored
    pub blob_count: u64,

    /// Current session status
    pub status: SessionStatus,

    /// Custom metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Session {
    /// Create a new session
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            started_at: Utc::now(),
            stopped_at: None,
            capture_count: 0,
            blob_count: 0,
            status: SessionStatus::Active,
            metadata: HashMap::new(),
        }
    }

    /// Create a new session with generated name based on timestamp
    pub fn new_with_timestamp() -> Self {
        let name = format!("session_{}", Utc::now().format("%Y%m%d_%H%M%S"));
        Self::new(name)
    }

    /// Stop the session
    pub fn stop(&mut self) {
        self.stopped_at = Some(Utc::now());
        self.status = SessionStatus::Stopped;
    }

    /// Pause the session
    pub fn pause(&mut self) {
        self.status = SessionStatus::Paused;
    }

    /// Resume the session
    pub fn resume(&mut self) {
        self.status = SessionStatus::Active;
    }

    /// Get session duration
    pub fn duration(&self) -> chrono::Duration {
        let end = self.stopped_at.unwrap_or_else(Utc::now);
        end - self.started_at
    }

    /// Increment capture count
    pub fn increment_capture_count(&mut self) {
        self.capture_count += 1;
    }

    /// Increment blob count
    pub fn increment_blob_count(&mut self) {
        self.blob_count += 1;
    }

    /// Save session to file
    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let session_dir = data_dir.join("sessions").join(self.id.to_string());
        std::fs::create_dir_all(&session_dir).map_err(|e| YinxError::Io {
            source: e,
            context: format!(
                "Failed to create session directory: {}",
                session_dir.display()
            ),
        })?;

        let state_file = session_dir.join("state.json");
        let content = serde_json::to_string_pretty(self).map_err(|e| YinxError::Json {
            source: e,
            context: "Failed to serialize session state".to_string(),
        })?;
        std::fs::write(&state_file, content).map_err(|e| YinxError::Io {
            source: e,
            context: format!(
                "Failed to write session state file: {}",
                state_file.display()
            ),
        })?;

        Ok(())
    }

    /// Load session from file
    pub fn load(data_dir: &Path, id: &Uuid) -> Result<Self> {
        let state_file = data_dir
            .join("sessions")
            .join(id.to_string())
            .join("state.json");

        if !state_file.exists() {
            return Err(YinxError::SessionNotFound { id: id.to_string() });
        }

        let content = std::fs::read_to_string(&state_file).map_err(|e| YinxError::Io {
            source: e,
            context: format!(
                "Failed to read session state file: {}",
                state_file.display()
            ),
        })?;
        let session = serde_json::from_str(&content).map_err(|e| YinxError::Json {
            source: e,
            context: "Failed to deserialize session state".to_string(),
        })?;

        Ok(session)
    }

    /// Get the directory for this session
    pub fn session_dir(&self, data_dir: &Path) -> PathBuf {
        data_dir.join("sessions").join(self.id.to_string())
    }
}

/// Session manager for CRUD operations
pub struct SessionManager {
    data_dir: PathBuf,
    current_session: Option<Session>,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            current_session: None,
        }
    }

    /// Create and start a new session
    pub fn create_session(&mut self, name: Option<String>) -> Result<&Session> {
        let session = match name {
            Some(n) => Session::new(n),
            None => Session::new_with_timestamp(),
        };

        session.save(&self.data_dir)?;
        self.current_session = Some(session);

        Ok(self.current_session.as_ref().unwrap())
    }

    /// Get the current active session
    pub fn current_session(&self) -> Option<&Session> {
        self.current_session.as_ref()
    }

    /// Get mutable reference to current session
    pub fn current_session_mut(&mut self) -> Option<&mut Session> {
        self.current_session.as_mut()
    }

    /// Stop the current session
    pub fn stop_session(&mut self) -> Result<()> {
        if let Some(session) = &mut self.current_session {
            session.stop();
            session.save(&self.data_dir)?;
            self.current_session = None;
            Ok(())
        } else {
            Err(YinxError::Session("No active session".to_string()))
        }
    }

    /// Load a session by ID
    pub fn load_session(&mut self, id: &Uuid) -> Result<&Session> {
        let session = Session::load(&self.data_dir, id)?;
        self.current_session = Some(session);
        Ok(self.current_session.as_ref().unwrap())
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions_dir = self.data_dir.join("sessions");

        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();

        for entry in std::fs::read_dir(&sessions_dir).map_err(|e| YinxError::Io {
            source: e,
            context: format!(
                "Failed to read sessions directory: {}",
                sessions_dir.display()
            ),
        })? {
            let entry = entry.map_err(|e| YinxError::Io {
                source: e,
                context: "Failed to read directory entry".to_string(),
            })?;
            let path = entry.path();

            if path.is_dir() {
                if let Ok(id) = Uuid::parse_str(&entry.file_name().to_string_lossy()) {
                    if let Ok(session) = Session::load(&self.data_dir, &id) {
                        sessions.push(session);
                    }
                }
            }
        }

        // Sort by started_at descending (newest first)
        sessions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        Ok(sessions)
    }

    /// Find session by name
    pub fn find_by_name(&self, name: &str) -> Result<Option<Session>> {
        let sessions = self.list_sessions()?;
        Ok(sessions.into_iter().find(|s| s.name == name))
    }

    /// Delete a session
    pub fn delete_session(&self, id: &Uuid) -> Result<()> {
        let session_dir = self.data_dir.join("sessions").join(id.to_string());

        if !session_dir.exists() {
            return Err(YinxError::SessionNotFound { id: id.to_string() });
        }

        std::fs::remove_dir_all(&session_dir).map_err(|e| YinxError::Io {
            source: e,
            context: format!(
                "Failed to delete session directory: {}",
                session_dir.display()
            ),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_creation() {
        let session = Session::new("test_session");
        assert_eq!(session.name, "test_session");
        assert_eq!(session.status, SessionStatus::Active);
        assert!(session.stopped_at.is_none());
    }

    #[test]
    fn test_session_stop() {
        let mut session = Session::new("test");
        session.stop();
        assert_eq!(session.status, SessionStatus::Stopped);
        assert!(session.stopped_at.is_some());
    }

    #[test]
    fn test_session_manager() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path().to_path_buf());

        let session = manager.create_session(Some("test".to_string())).unwrap();
        assert_eq!(session.name, "test");

        let sessions = manager.list_sessions().unwrap();
        assert_eq!(sessions.len(), 1);
    }
}
