//! Configuration management for Yinx
//!
//! This module handles loading, validation, and management of configuration
//! following the configuration-driven design principles outlined in ARCHITECTURE.md

use crate::error::{Result, YinxError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

mod validator;

pub use validator::ConfigValidator;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "_meta")]
    pub meta: MetaConfig,
    pub storage: StorageConfig,
    pub capture: CaptureConfig,
    pub daemon: DaemonConfig,
    pub patterns: PatternsConfig,
    pub embedding: EmbeddingConfig,
    pub llm: LlmConfig,
    pub indexing: IndexingConfig,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileOverrides>,
}

/// Metadata about the configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaConfig {
    pub schema_version: String,
    #[serde(default = "current_timestamp")]
    pub created_at: String,
    #[serde(default = "current_timestamp")]
    pub last_modified: String,
}

fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub data_dir: PathBuf,
    pub max_blob_size: String,
}

/// Capture configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureConfig {
    pub buffer_size: usize,
    pub flush_interval: String,
}

/// Daemon configuration for process and IPC management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub socket_path: PathBuf,
    pub pid_file: PathBuf,
    pub log_file: PathBuf,
    pub max_connections: usize,
}

/// Pattern configuration - paths to pattern definition files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternsConfig {
    pub entities_file: PathBuf,
    pub tools_file: PathBuf,
    pub filters_file: PathBuf,
}

/// Embedding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    pub model: String,
    pub mode: String, // "offline" or "online"
    pub batch_size: usize,
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub enabled: bool,
    pub provider: String,
    pub api_key_env: String,
    pub model: String,
    pub temperature: f32,
}

/// Indexing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    pub vector_dim: usize,
    pub hnsw_ef_construction: usize,
    pub hnsw_m: usize,
}

/// Profile-specific configuration overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_enabled: Option<bool>,
}

impl Config {
    /// Load configuration from a file
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(YinxError::ConfigNotFound {
                path: path.to_path_buf(),
            });
        }

        let content = std::fs::read_to_string(path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to read config file: {:?}", path),
        })?;
        let mut config: Config = toml::from_str(&content)?;

        // Apply environment variable overrides
        config.apply_env_overrides();

        // Validate configuration
        ConfigValidator::validate(&config)?;

        Ok(config)
    }

    /// Save configuration to a file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to write config file: {:?}", path),
        })?;
        Ok(())
    }

    /// Load configuration with a specific profile applied
    pub fn load_with_profile(path: &Path, profile: &str) -> Result<Self> {
        let mut config = Self::load(path)?;
        config.apply_profile(profile)?;
        Ok(config)
    }

    /// Apply a profile's overrides to the configuration
    pub fn apply_profile(&mut self, profile: &str) -> Result<()> {
        if let Some(overrides) = self.profiles.get(profile) {
            if let Some(mode) = &overrides.embedding_mode {
                self.embedding.mode = mode.clone();
            }
            if let Some(model) = &overrides.embedding_model {
                self.embedding.model = model.clone();
            }
            if let Some(enabled) = overrides.llm_enabled {
                self.llm.enabled = enabled;
            }
        }
        Ok(())
    }

    /// Apply environment variable overrides
    /// Environment variables in format: YINX_SECTION__KEY=value
    pub fn apply_env_overrides(&mut self) {
        for (key, value) in std::env::vars() {
            if let Some(config_key) = key.strip_prefix("YINX_") {
                if let Err(e) = self.set_value_from_env(config_key, &value) {
                    tracing::warn!("Failed to apply env override {}: {}", key, e);
                }
            }
        }
    }

    fn set_value_from_env(&mut self, path: &str, value: &str) -> Result<()> {
        // Simple implementation for common overrides
        match path {
            "LLM__ENABLED" => {
                self.llm.enabled = value.parse().map_err(|_| YinxError::InvalidConfigValue {
                    path: path.to_string(),
                    message: format!("Cannot parse '{}' as boolean", value),
                })?;
            }
            "LLM__MODEL" => {
                self.llm.model = value.to_string();
            }
            "EMBEDDING__MODE" => {
                self.embedding.mode = value.to_string();
            }
            "EMBEDDING__MODEL" => {
                self.embedding.model = value.to_string();
            }
            _ => {
                tracing::debug!("Unknown env config key: {}", path);
            }
        }
        Ok(())
    }

    /// Get the default configuration file path
    pub fn default_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| YinxError::Config("Cannot determine config directory".to_string()))?;

        Ok(config_dir.join("yinx").join("config.toml"))
    }

    /// Get the default data directory
    pub fn default_data_dir() -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| YinxError::Config("Cannot determine home directory".to_string()))?;

        Ok(home_dir.join(".yinx"))
    }
}

impl Default for Config {
    fn default() -> Self {
        let data_dir = PathBuf::from("~/.yinx");
        let config_dir = PathBuf::from("~/.config/yinx");

        Self {
            meta: MetaConfig {
                schema_version: "1.0.0".to_string(),
                created_at: current_timestamp(),
                last_modified: current_timestamp(),
            },
            storage: StorageConfig {
                data_dir: data_dir.clone(),
                max_blob_size: "10MB".to_string(),
            },
            capture: CaptureConfig {
                buffer_size: 10000,
                flush_interval: "5s".to_string(),
            },
            daemon: DaemonConfig {
                socket_path: data_dir.join("daemon.sock"),
                pid_file: data_dir.join("daemon.pid"),
                log_file: data_dir.join("logs").join("daemon.log"),
                max_connections: 10,
            },
            patterns: PatternsConfig {
                entities_file: config_dir.join("entities.toml"),
                tools_file: config_dir.join("tools.toml"),
                filters_file: config_dir.join("filters.toml"),
            },
            embedding: EmbeddingConfig {
                model: "all-MiniLM-L6-v2".to_string(),
                mode: "offline".to_string(),
                batch_size: 32,
            },
            llm: LlmConfig {
                enabled: false,
                provider: "groq".to_string(),
                api_key_env: "GROQ_API_KEY".to_string(),
                model: "llama-3.1-70b".to_string(),
                temperature: 0.1,
            },
            indexing: IndexingConfig {
                vector_dim: 384,
                hnsw_ef_construction: 200,
                hnsw_m: 16,
            },
            profiles: HashMap::new(),
        }
    }
}
