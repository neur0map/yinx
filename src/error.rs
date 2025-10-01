use std::path::PathBuf;
use thiserror::Error;

/// Main error type for Yinx application
#[derive(Error, Debug)]
pub enum YinxError {
    /// Configuration related errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Configuration validation errors
    #[error("Configuration validation failed: {errors:?}")]
    ConfigValidation { errors: Vec<ValidationError> },

    /// Configuration file not found
    #[error("Configuration file not found: {path}")]
    ConfigNotFound { path: PathBuf },

    /// Invalid configuration value
    #[error("Invalid configuration value at {path}: {message}")]
    InvalidConfigValue { path: String, message: String },

    /// Session related errors
    #[error("Session error: {0}")]
    Session(String),

    /// Session not found
    #[error("Session not found: {id}")]
    SessionNotFound { id: String },

    /// IO errors
    #[error("IO error: {context}: {source}")]
    Io {
        source: std::io::Error,
        context: String,
    },

    /// TOML deserialization errors
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),

    /// TOML serialization errors
    #[error("TOML serialization error: {0}")]
    TomlSerialization(#[from] toml::ser::Error),

    /// JSON errors
    #[error("JSON error: {context}: {source}")]
    Json {
        source: serde_json::Error,
        context: String,
    },

    /// Database errors
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// Daemon errors
    #[error("Daemon error: {0}")]
    Daemon(String),

    /// Daemon not running
    #[error("Daemon is not running")]
    DaemonNotRunning,

    /// Daemon already running
    #[error("Daemon is already running (PID: {pid})")]
    DaemonAlreadyRunning { pid: u32 },

    /// Generic errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Configuration validation error
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// Path to the configuration key that failed validation
    pub path: String,
    /// Error message describing the validation failure
    pub message: String,
}

impl ValidationError {
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

/// Result type for Yinx operations
pub type Result<T> = std::result::Result<T, YinxError>;
