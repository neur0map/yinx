//! Pattern registry for configuration-driven entity extraction and tool detection
//!
//! This module provides:
//! - Pre-compiled regex patterns loaded from configuration files
//! - Entity extraction patterns (IPs, ports, CVEs, credentials, etc.)
//! - Tool detection patterns (nmap, gobuster, hydra, etc.)
//! - Filter normalization patterns (for tier 1-3 filtering)

use crate::error::{Result, YinxError};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Entity pattern configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityConfig {
    #[serde(rename = "type")]
    pub type_name: String,
    pub pattern: String,
    pub confidence: f32,
    pub context_window: usize,
    #[serde(default)]
    pub redact: bool,
    #[serde(default)]
    pub description: String,
}

/// Entity patterns configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitiesConfig {
    pub entity: Vec<EntityConfig>,
}

/// Tool detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub name: String,
    pub command_patterns: Vec<String>,
    pub entity_hints: Vec<String>,
    pub output_patterns: Vec<OutputPatternConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPatternConfig {
    pub pattern: String,
    pub section: String,
}

/// Tools configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub tool: Vec<ToolConfig>,
}

/// Normalization pattern for tier 1 filtering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizationPattern {
    pub name: String,
    pub pattern: String,
    pub replacement: String,
    #[serde(default)]
    pub priority: u8,
}

/// Technical pattern for tier 2 scoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechnicalPattern {
    pub name: String,
    pub pattern: String,
    pub weight: f32,
}

/// Filtering configuration file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FiltersConfig {
    pub tier1: Tier1Config,
    pub tier2: Tier2Config,
    pub tier3: Tier3Config,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier1Config {
    pub max_occurrences: u32,
    pub normalization_patterns: Vec<NormalizationPattern>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier2Config {
    pub entropy_weight: f32,
    pub uniqueness_weight: f32,
    pub technical_weight: f32,
    pub change_weight: f32,
    pub score_threshold_percentile: f32,
    pub technical_patterns: Vec<TechnicalPattern>,
    pub max_technical_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tier3Config {
    pub cluster_min_size: usize,
    pub max_cluster_size: usize,
    pub representative_strategy: String,
    pub cluster_patterns: Vec<NormalizationPattern>,
    pub preserve_metadata: Vec<String>,
}

/// Compiled entity pattern with pre-compiled regex
#[derive(Debug, Clone)]
pub struct CompiledEntityPattern {
    pub type_name: String,
    pub regex: Regex,
    pub confidence: f32,
    pub context_window: usize,
    pub redact: bool,
    pub description: String,
}

/// Compiled tool matcher with pre-compiled regexes
#[derive(Debug, Clone)]
pub struct CompiledToolMatcher {
    pub name: String,
    pub command_patterns: Vec<Regex>,
    pub entity_hints: Vec<String>,
    pub output_patterns: Vec<(Regex, String)>,
}

/// Compiled normalization pattern
#[derive(Debug, Clone)]
pub struct CompiledNormalizationPattern {
    pub name: String,
    pub regex: Regex,
    pub replacement: String,
    pub priority: u8,
}

/// Compiled technical pattern for scoring
#[derive(Debug, Clone)]
pub struct CompiledTechnicalPattern {
    pub name: String,
    pub regex: Regex,
    pub weight: f32,
}

/// Pattern registry with all pre-compiled patterns
#[derive(Clone)]
pub struct PatternRegistry {
    /// Entity extraction patterns
    pub entities: Vec<CompiledEntityPattern>,
    /// Entity lookup by type name
    pub entities_by_type: HashMap<String, usize>,
    /// Tool detection matchers
    pub tools: Vec<CompiledToolMatcher>,
    /// Tool lookup by name
    pub tools_by_name: HashMap<String, usize>,
    /// Tier 1 normalization patterns
    pub tier1_normalization: Vec<CompiledNormalizationPattern>,
    /// Tier 2 technical patterns
    pub tier2_technical: Vec<CompiledTechnicalPattern>,
    /// Tier 3 cluster patterns
    pub tier3_cluster: Vec<CompiledNormalizationPattern>,
    /// Tier 1 configuration
    pub tier1_config: Tier1Config,
    /// Tier 2 configuration
    pub tier2_config: Tier2Config,
    /// Tier 3 configuration
    pub tier3_config: Tier3Config,
}

impl PatternRegistry {
    /// Load pattern registry from configuration files
    pub fn from_config_files(
        entities_path: &Path,
        tools_path: &Path,
        filters_path: &Path,
    ) -> Result<Self> {
        // Load entities config
        let entities_toml = std::fs::read_to_string(entities_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to read entities config: {:?}", entities_path),
        })?;
        let entities_config: EntitiesConfig = toml::from_str(&entities_toml)?;

        // Load tools config
        let tools_toml = std::fs::read_to_string(tools_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to read tools config: {:?}", tools_path),
        })?;
        let tools_config: ToolsConfig = toml::from_str(&tools_toml)?;

        // Load filters config
        let filters_toml = std::fs::read_to_string(filters_path).map_err(|e| YinxError::Io {
            source: e,
            context: format!("Failed to read filters config: {:?}", filters_path),
        })?;
        let filters_config: FiltersConfig = toml::from_str(&filters_toml)?;

        Self::from_configs(entities_config, tools_config, filters_config)
    }

    /// Build pattern registry from parsed configurations
    pub fn from_configs(
        entities_config: EntitiesConfig,
        tools_config: ToolsConfig,
        filters_config: FiltersConfig,
    ) -> Result<Self> {
        // Compile entity patterns
        let mut entities = Vec::new();
        let mut entities_by_type = HashMap::new();

        for (idx, entity_cfg) in entities_config.entity.iter().enumerate() {
            let regex = Regex::new(&entity_cfg.pattern).map_err(|e| {
                YinxError::Config(format!(
                    "Invalid regex for entity '{}': {}",
                    entity_cfg.type_name, e
                ))
            })?;

            entities.push(CompiledEntityPattern {
                type_name: entity_cfg.type_name.clone(),
                regex,
                confidence: entity_cfg.confidence,
                context_window: entity_cfg.context_window,
                redact: entity_cfg.redact,
                description: entity_cfg.description.clone(),
            });

            entities_by_type.insert(entity_cfg.type_name.clone(), idx);
        }

        // Compile tool matchers
        let mut tools = Vec::new();
        let mut tools_by_name = HashMap::new();

        for (idx, tool_cfg) in tools_config.tool.iter().enumerate() {
            let command_patterns: Vec<Regex> = tool_cfg
                .command_patterns
                .iter()
                .map(|p| {
                    Regex::new(p).map_err(|e| {
                        YinxError::Config(format!(
                            "Invalid command pattern for tool '{}': {}",
                            tool_cfg.name, e
                        ))
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            let output_patterns: Vec<(Regex, String)> = tool_cfg
                .output_patterns
                .iter()
                .map(|op| {
                    Regex::new(&op.pattern)
                        .map(|r| (r, op.section.clone()))
                        .map_err(|e| {
                            YinxError::Config(format!(
                                "Invalid output pattern for tool '{}': {}",
                                tool_cfg.name, e
                            ))
                        })
                })
                .collect::<Result<Vec<_>>>()?;

            tools.push(CompiledToolMatcher {
                name: tool_cfg.name.clone(),
                command_patterns,
                entity_hints: tool_cfg.entity_hints.clone(),
                output_patterns,
            });

            tools_by_name.insert(tool_cfg.name.clone(), idx);
        }

        // Compile tier 1 normalization patterns
        let mut tier1_normalization: Vec<CompiledNormalizationPattern> = filters_config
            .tier1
            .normalization_patterns
            .iter()
            .map(|np| {
                Regex::new(&np.pattern)
                    .map(|r| CompiledNormalizationPattern {
                        name: np.name.clone(),
                        regex: r,
                        replacement: np.replacement.clone(),
                        priority: np.priority,
                    })
                    .map_err(|e| {
                        YinxError::Config(format!(
                            "Invalid tier1 normalization pattern '{}': {}",
                            np.name, e
                        ))
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        // Sort by priority
        tier1_normalization.sort_by_key(|p| p.priority);

        // Compile tier 2 technical patterns
        let tier2_technical: Vec<CompiledTechnicalPattern> = filters_config
            .tier2
            .technical_patterns
            .iter()
            .map(|tp| {
                Regex::new(&tp.pattern)
                    .map(|r| CompiledTechnicalPattern {
                        name: tp.name.clone(),
                        regex: r,
                        weight: tp.weight,
                    })
                    .map_err(|e| {
                        YinxError::Config(format!(
                            "Invalid tier2 technical pattern '{}': {}",
                            tp.name, e
                        ))
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        // Compile tier 3 cluster patterns
        let mut tier3_cluster: Vec<CompiledNormalizationPattern> = filters_config
            .tier3
            .cluster_patterns
            .iter()
            .map(|cp| {
                Regex::new(&cp.pattern)
                    .map(|r| CompiledNormalizationPattern {
                        name: cp.name.clone(),
                        regex: r,
                        replacement: cp.replacement.clone(),
                        priority: 0,
                    })
                    .map_err(|e| {
                        YinxError::Config(format!(
                            "Invalid tier3 cluster pattern '{}': {}",
                            cp.name, e
                        ))
                    })
            })
            .collect::<Result<Vec<_>>>()?;

        tier3_cluster.sort_by_key(|p| p.priority);

        Ok(Self {
            entities,
            entities_by_type,
            tools,
            tools_by_name,
            tier1_normalization,
            tier2_technical,
            tier3_cluster,
            tier1_config: filters_config.tier1,
            tier2_config: filters_config.tier2,
            tier3_config: filters_config.tier3,
        })
    }

    /// Detect tool from command string
    pub fn detect_tool(&self, command: &str) -> Option<&CompiledToolMatcher> {
        self.tools
            .iter()
            .find(|tool| tool.command_patterns.iter().any(|p| p.is_match(command)))
    }

    /// Extract all entities from text
    pub fn extract_entities(&self, text: &str) -> Vec<ExtractedEntity> {
        self.entities
            .iter()
            .flat_map(|pattern| {
                pattern.regex.find_iter(text).map(|m| ExtractedEntity {
                    type_name: pattern.type_name.clone(),
                    value: m.as_str().to_string(),
                    start: m.start(),
                    end: m.end(),
                    context: Self::get_context(text, m.start(), m.end(), pattern.context_window),
                    confidence: pattern.confidence,
                    redact: pattern.redact,
                })
            })
            .collect()
    }

    /// Get context around a match
    fn get_context(text: &str, start: usize, end: usize, window: usize) -> String {
        let context_start = start.saturating_sub(window);
        let context_end = (end + window).min(text.len());
        text[context_start..context_end].to_string()
    }

    /// Apply tier 1 normalization
    pub fn normalize_tier1(&self, line: &str) -> String {
        let mut result = line.to_string();
        for pattern in &self.tier1_normalization {
            result = pattern
                .regex
                .replace_all(&result, &pattern.replacement)
                .to_string();
        }
        result
    }

    /// Calculate tier 2 technical score
    pub fn calculate_technical_score(&self, line: &str, max_score: f32) -> f32 {
        let weighted_sum: f32 = self
            .tier2_technical
            .iter()
            .map(|p| p.regex.find_iter(line).count() as f32 * p.weight)
            .sum();

        (weighted_sum / max_score).min(1.0)
    }

    /// Apply tier 3 normalization
    pub fn normalize_tier3(&self, line: &str) -> String {
        let mut result = line.to_string();
        for pattern in &self.tier3_cluster {
            result = pattern
                .regex
                .replace_all(&result, &pattern.replacement)
                .to_string();
        }
        result
    }
}

/// Extracted entity from text
#[derive(Debug, Clone)]
pub struct ExtractedEntity {
    pub type_name: String,
    pub value: String,
    pub start: usize,
    pub end: usize,
    pub context: String,
    pub confidence: f32,
    pub redact: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_pattern_compilation() {
        let config = EntitiesConfig {
            entity: vec![EntityConfig {
                type_name: "ip_address".to_string(),
                pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                confidence: 0.95,
                context_window: 50,
                redact: false,
                description: "IPv4 address".to_string(),
            }],
        };

        let tools_config = ToolsConfig { tool: vec![] };
        let filters_config = FiltersConfig {
            tier1: Tier1Config {
                max_occurrences: 3,
                normalization_patterns: vec![],
            },
            tier2: Tier2Config {
                entropy_weight: 0.3,
                uniqueness_weight: 0.3,
                technical_weight: 0.2,
                change_weight: 0.2,
                score_threshold_percentile: 0.8,
                technical_patterns: vec![],
                max_technical_score: 10.0,
            },
            tier3: Tier3Config {
                cluster_min_size: 2,
                max_cluster_size: 1000,
                representative_strategy: "highest_entropy".to_string(),
                cluster_patterns: vec![],
                preserve_metadata: vec![],
            },
        };

        let registry = PatternRegistry::from_configs(config, tools_config, filters_config).unwrap();
        assert_eq!(registry.entities.len(), 1);
    }

    #[test]
    fn test_entity_extraction() {
        let config = EntitiesConfig {
            entity: vec![EntityConfig {
                type_name: "ip_address".to_string(),
                pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                confidence: 0.95,
                context_window: 10,
                redact: false,
                description: "IPv4".to_string(),
            }],
        };

        let tools_config = ToolsConfig { tool: vec![] };
        let filters_config = FiltersConfig {
            tier1: Tier1Config {
                max_occurrences: 3,
                normalization_patterns: vec![],
            },
            tier2: Tier2Config {
                entropy_weight: 0.3,
                uniqueness_weight: 0.3,
                technical_weight: 0.2,
                change_weight: 0.2,
                score_threshold_percentile: 0.8,
                technical_patterns: vec![],
                max_technical_score: 10.0,
            },
            tier3: Tier3Config {
                cluster_min_size: 2,
                max_cluster_size: 1000,
                representative_strategy: "highest_entropy".to_string(),
                cluster_patterns: vec![],
                preserve_metadata: vec![],
            },
        };

        let registry = PatternRegistry::from_configs(config, tools_config, filters_config).unwrap();

        let text = "Found host at 192.168.1.1 and 10.0.0.1";
        let entities = registry.extract_entities(text);

        assert_eq!(entities.len(), 2);
        assert_eq!(entities[0].value, "192.168.1.1");
        assert_eq!(entities[1].value, "10.0.0.1");
    }
}
