//! Entity extraction using PatternRegistry
//!
//! Provides configuration-driven entity extraction with ZERO hardcoded patterns

use crate::patterns::{ExtractedEntity, PatternRegistry};
use serde::{Deserialize, Serialize};

/// Extracted entity with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Entity type (ip_address, cve, credential_password, etc.)
    pub entity_type: String,
    /// Extracted value
    pub value: String,
    /// Context surrounding the entity
    pub context: String,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Whether this entity should be redacted in reports
    pub should_redact: bool,
}

impl From<ExtractedEntity> for Entity {
    fn from(extracted: ExtractedEntity) -> Self {
        Self {
            entity_type: extracted.type_name,
            value: extracted.value,
            context: extracted.context,
            confidence: extracted.confidence,
            should_redact: extracted.redact,
        }
    }
}

/// Entity extractor using PatternRegistry
///
/// Uses pre-compiled regex patterns from entities.toml
/// No hardcoded patterns - 100% configuration-driven
pub struct EntityExtractor {
    registry: PatternRegistry,
}

impl EntityExtractor {
    /// Create new entity extractor with pattern registry
    pub fn new(registry: PatternRegistry) -> Self {
        Self { registry }
    }

    /// Extract all entities from text
    ///
    /// Returns entities sorted by position in text
    pub fn extract(&self, text: &str) -> Vec<Entity> {
        self.registry
            .extract_entities(text)
            .into_iter()
            .map(Entity::from)
            .collect()
    }

    /// Extract entities by specific type
    ///
    /// Example types: "ip_address", "cve", "credential_password"
    pub fn extract_by_type(&self, text: &str, entity_type: &str) -> Vec<Entity> {
        self.extract(text)
            .into_iter()
            .filter(|e| e.entity_type == entity_type)
            .collect()
    }

    /// Extract entities with minimum confidence threshold
    pub fn extract_with_confidence(&self, text: &str, min_confidence: f32) -> Vec<Entity> {
        self.extract(text)
            .into_iter()
            .filter(|e| e.confidence >= min_confidence)
            .collect()
    }

    /// Extract only redactable entities (credentials, keys, etc.)
    pub fn extract_sensitive(&self, text: &str) -> Vec<Entity> {
        self.extract(text)
            .into_iter()
            .filter(|e| e.should_redact)
            .collect()
    }

    /// Get unique entity types found in text
    pub fn get_entity_types(&self, text: &str) -> Vec<String> {
        let mut types: Vec<String> = self
            .extract(text)
            .into_iter()
            .map(|e| e.entity_type)
            .collect();
        types.sort();
        types.dedup();
        types
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{
        EntitiesConfig, EntityConfig, FiltersConfig, Tier1Config, Tier2Config, Tier3Config,
        ToolsConfig,
    };

    fn create_test_extractor() -> EntityExtractor {
        let entities_config = EntitiesConfig {
            entity: vec![
                EntityConfig {
                    type_name: "ip_address".to_string(),
                    pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                    confidence: 0.95,
                    context_window: 50,
                    redact: false,
                    description: "IPv4 address".to_string(),
                },
                EntityConfig {
                    type_name: "cve".to_string(),
                    pattern: r"CVE-\d{4}-\d{4,}".to_string(),
                    confidence: 1.0,
                    context_window: 100,
                    redact: false,
                    description: "CVE vulnerability".to_string(),
                },
                EntityConfig {
                    type_name: "credential_password".to_string(),
                    pattern: r"(?i)(password|passwd|pwd)\s*[:=]\s*\S+".to_string(),
                    confidence: 0.7,
                    context_window: 80,
                    redact: true,
                    description: "Password credential".to_string(),
                },
            ],
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

        let registry =
            PatternRegistry::from_configs(entities_config, tools_config, filters_config).unwrap();
        EntityExtractor::new(registry)
    }

    #[test]
    fn test_extract_basic() {
        let extractor = create_test_extractor();
        let text = "Found host at 192.168.1.1 with CVE-2021-44228";
        let entities = extractor.extract(text);

        assert_eq!(entities.len(), 2);
        assert!(entities.iter().any(|e| e.value == "192.168.1.1"));
        assert!(entities.iter().any(|e| e.value == "CVE-2021-44228"));
    }

    #[test]
    fn test_extract_by_type() {
        let extractor = create_test_extractor();
        let text = "Host 192.168.1.1 has CVE-2021-44228 and 10.0.0.1 has CVE-2021-12345";

        let ips = extractor.extract_by_type(text, "ip_address");
        assert_eq!(ips.len(), 2);

        let cves = extractor.extract_by_type(text, "cve");
        assert_eq!(cves.len(), 2);
    }

    #[test]
    fn test_extract_with_confidence() {
        let extractor = create_test_extractor();
        let text = "Host at 192.168.1.1 with password=admin123";

        // High confidence only (>= 0.95)
        let high_conf = extractor.extract_with_confidence(text, 0.95);
        assert_eq!(high_conf.len(), 1); // Only IP
        assert_eq!(high_conf[0].entity_type, "ip_address");

        // Medium confidence (>= 0.7)
        let med_conf = extractor.extract_with_confidence(text, 0.7);
        assert_eq!(med_conf.len(), 2); // IP + password
    }

    #[test]
    fn test_extract_sensitive() {
        let extractor = create_test_extractor();
        let text = "Found 192.168.1.1 with password=secret123";

        let sensitive = extractor.extract_sensitive(text);
        assert_eq!(sensitive.len(), 1);
        assert!(sensitive[0].should_redact);
        assert_eq!(sensitive[0].entity_type, "credential_password");
    }

    #[test]
    fn test_get_entity_types() {
        let extractor = create_test_extractor();
        let text = "Host 192.168.1.1 has CVE-2021-44228 and password=admin";

        let types = extractor.get_entity_types(text);
        assert_eq!(types.len(), 3);
        assert!(types.contains(&"ip_address".to_string()));
        assert!(types.contains(&"cve".to_string()));
        assert!(types.contains(&"credential_password".to_string()));
    }

    #[test]
    fn test_multiple_same_type() {
        let extractor = create_test_extractor();
        let text = "Hosts: 192.168.1.1, 192.168.1.2, 10.0.0.1";

        let ips = extractor.extract_by_type(text, "ip_address");
        assert_eq!(ips.len(), 3);
    }

    #[test]
    fn test_context_extraction() {
        let extractor = create_test_extractor();
        let text = "Found vulnerability CVE-2021-44228 in Apache Log4j";

        let entities = extractor.extract(text);
        let cve = entities.iter().find(|e| e.entity_type == "cve").unwrap();

        // Context should include surrounding text
        assert!(cve.context.contains("CVE-2021-44228"));
        assert!(cve.context.len() > "CVE-2021-44228".len());
    }

    #[test]
    fn test_empty_text() {
        let extractor = create_test_extractor();
        let entities = extractor.extract("");
        assert_eq!(entities.len(), 0);
    }

    #[test]
    fn test_no_matches() {
        let extractor = create_test_extractor();
        let text = "No entities in this text";
        let entities = extractor.extract(text);
        assert_eq!(entities.len(), 0);
    }
}
