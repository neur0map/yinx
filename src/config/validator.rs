use crate::config::Config;
use crate::error::{Result, ValidationError, YinxError};

/// Configuration validator
pub struct ConfigValidator;

impl ConfigValidator {
    /// Validate the configuration
    pub fn validate(config: &Config) -> Result<()> {
        let mut errors = Vec::new();

        // Validate schema version
        Self::validate_schema_version(config, &mut errors);

        // Validate storage settings
        Self::validate_storage(config, &mut errors);

        // Validate capture settings
        Self::validate_capture(config, &mut errors);

        // Validate pattern file paths
        Self::validate_patterns(config, &mut errors);

        // Validate embedding settings
        Self::validate_embedding(config, &mut errors);

        // Validate LLM settings
        Self::validate_llm(config, &mut errors);

        // Validate indexing settings
        Self::validate_indexing(config, &mut errors);

        if errors.is_empty() {
            Ok(())
        } else {
            Err(YinxError::ConfigValidation { errors })
        }
    }

    fn validate_schema_version(config: &Config, errors: &mut Vec<ValidationError>) {
        let version = &config.meta.schema_version;
        if version != "1.0.0" {
            errors.push(ValidationError::new(
                "_meta.schema_version",
                format!("Unsupported schema version: {}", version),
            ));
        }
    }

    fn validate_storage(config: &Config, errors: &mut Vec<ValidationError>) {
        // Validate max_blob_size format
        let size_str = &config.storage.max_blob_size;
        if !Self::is_valid_size_string(size_str) {
            errors.push(ValidationError::new(
                "storage.max_blob_size",
                format!("Invalid size format: {}", size_str),
            ));
        }
    }

    fn validate_capture(config: &Config, errors: &mut Vec<ValidationError>) {
        // Validate buffer size
        if config.capture.buffer_size == 0 {
            errors.push(ValidationError::new(
                "capture.buffer_size",
                "Buffer size must be greater than 0",
            ));
        }

        // Validate batch size
        if config.capture.batch_size == 0 {
            errors.push(ValidationError::new(
                "capture.batch_size",
                "Batch size must be greater than 0",
            ));
        }

        // Validate flush interval format
        let interval = &config.capture.flush_interval;
        if !Self::is_valid_duration_string(interval) {
            errors.push(ValidationError::new(
                "capture.flush_interval",
                format!("Invalid duration format: {}", interval),
            ));
        }
    }

    fn validate_patterns(config: &Config, errors: &mut Vec<ValidationError>) {
        // Note: Pattern file existence is not checked here because:
        // 1. Paths may contain ~ which needs expansion
        // 2. Files may not exist yet (created by `yinx config init`)
        // 3. PatternRegistry loading will handle missing file errors

        // Just validate paths are not empty
        if config.patterns.entities_file.as_os_str().is_empty() {
            errors.push(ValidationError::new(
                "patterns.entities_file",
                "Entities file path cannot be empty",
            ));
        }

        if config.patterns.tools_file.as_os_str().is_empty() {
            errors.push(ValidationError::new(
                "patterns.tools_file",
                "Tools file path cannot be empty",
            ));
        }

        if config.patterns.filters_file.as_os_str().is_empty() {
            errors.push(ValidationError::new(
                "patterns.filters_file",
                "Filters file path cannot be empty",
            ));
        }
    }

    fn validate_embedding(config: &Config, errors: &mut Vec<ValidationError>) {
        // Validate mode
        let mode = &config.embedding.mode;
        if mode != "offline" && mode != "online" {
            errors.push(ValidationError::new(
                "embedding.mode",
                format!("Mode must be 'offline' or 'online', got '{}'", mode),
            ));
        }

        // Validate batch size
        if config.embedding.batch_size == 0 {
            errors.push(ValidationError::new(
                "embedding.batch_size",
                "Batch size must be greater than 0",
            ));
        }

        // Validate model name is not empty
        if config.embedding.model.is_empty() {
            errors.push(ValidationError::new(
                "embedding.model",
                "Model name cannot be empty",
            ));
        }
    }

    fn validate_llm(config: &Config, errors: &mut Vec<ValidationError>) {
        // If LLM is enabled, validate API key environment variable is set
        if config.llm.enabled {
            let env_var = &config.llm.api_key_env;
            if let Ok(key) = std::env::var(env_var) {
                if key.is_empty() {
                    errors.push(ValidationError::new(
                        "llm.api_key_env",
                        format!("Environment variable {} is empty", env_var),
                    ));
                }
            } else {
                errors.push(ValidationError::new(
                    "llm.api_key_env",
                    format!("Environment variable {} is not set", env_var),
                ));
            }
        }

        // Validate temperature range
        let temp = config.llm.temperature;
        if !(0.0..=2.0).contains(&temp) {
            errors.push(ValidationError::new(
                "llm.temperature",
                format!("Temperature must be between 0.0 and 2.0, got {}", temp),
            ));
        }

        // Validate provider
        let provider = &config.llm.provider;
        let valid_providers = ["groq", "openai", "anthropic", "ollama"];
        if !valid_providers.contains(&provider.as_str()) {
            errors.push(ValidationError::new(
                "llm.provider",
                format!(
                    "Provider must be one of {:?}, got '{}'",
                    valid_providers, provider
                ),
            ));
        }
    }

    fn validate_indexing(config: &Config, errors: &mut Vec<ValidationError>) {
        // Validate vector_dim
        if config.indexing.vector_dim == 0 {
            errors.push(ValidationError::new(
                "indexing.vector_dim",
                "Vector dimension must be greater than 0",
            ));
        }

        // Validate HNSW parameters
        if config.indexing.hnsw_ef_construction == 0 {
            errors.push(ValidationError::new(
                "indexing.hnsw_ef_construction",
                "HNSW ef_construction must be greater than 0",
            ));
        }

        if config.indexing.hnsw_m == 0 {
            errors.push(ValidationError::new(
                "indexing.hnsw_m",
                "HNSW M must be greater than 0",
            ));
        }
    }

    fn is_valid_size_string(s: &str) -> bool {
        // Simple validation for size strings like "10MB", "1GB"
        let s = s.to_uppercase();
        s.ends_with("KB")
            || s.ends_with("MB")
            || s.ends_with("GB")
            || s.ends_with("B")
            || s.chars().all(|c| c.is_ascii_digit())
    }

    fn is_valid_duration_string(s: &str) -> bool {
        // Simple validation for duration strings like "5s", "10m", "1h"
        s.ends_with('s')
            || s.ends_with('m')
            || s.ends_with('h')
            || s.chars().all(|c| c.is_ascii_digit())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_valid_config() {
        let config = Config::default();
        assert!(ConfigValidator::validate(&config).is_ok());
    }

    #[test]
    fn test_empty_pattern_path() {
        let mut config = Config::default();
        config.patterns.entities_file = PathBuf::new();
        assert!(ConfigValidator::validate(&config).is_err());
    }

    #[test]
    fn test_invalid_mode() {
        let mut config = Config::default();
        config.embedding.mode = "invalid".to_string();
        assert!(ConfigValidator::validate(&config).is_err());
    }
}
