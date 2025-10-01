//! Metadata enrichment for captures and chunks
//!
//! Provides structured metadata extraction and enrichment

use crate::entities::{CorrelationGraph, Entity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Capture-level metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureMetadata {
    /// Tool detected from command
    pub tool: Option<String>,
    /// Entity types found in output
    pub entity_types: Vec<String>,
    /// Number of entities extracted
    pub entity_count: usize,
    /// Hosts discovered in this capture
    pub hosts: Vec<String>,
    /// Vulnerabilities found in this capture
    pub vulnerabilities: Vec<String>,
    /// Whether sensitive data was found
    pub has_sensitive_data: bool,
}

impl CaptureMetadata {
    /// Create metadata from entities
    pub fn from_entities(entities: &[Entity], tool: Option<String>) -> Self {
        let mut entity_types: Vec<String> =
            entities.iter().map(|e| e.entity_type.clone()).collect();
        entity_types.sort();
        entity_types.dedup();

        let hosts: Vec<String> = entities
            .iter()
            .filter(|e| e.entity_type == "ip_address" || e.entity_type == "hostname")
            .map(|e| e.value.clone())
            .collect();

        let vulnerabilities: Vec<String> = entities
            .iter()
            .filter(|e| e.entity_type == "cve")
            .map(|e| e.value.clone())
            .collect();

        let has_sensitive_data = entities.iter().any(|e| e.should_redact);

        Self {
            tool,
            entity_types,
            entity_count: entities.len(),
            hosts,
            vulnerabilities,
            has_sensitive_data,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Create from JSON string
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }
}

/// Chunk-level metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Entities found in this chunk
    pub entities: Vec<Entity>,
    /// Relevance score from filtering
    pub relevance_score: f32,
    /// Tier that selected this chunk (1, 2, or 3)
    pub selected_by_tier: u8,
    /// Cluster information (if from tier 3)
    pub cluster_info: Option<ClusterInfo>,
}

/// Cluster information for tier 3 chunks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterInfo {
    /// Number of lines in cluster
    pub cluster_size: usize,
    /// Representative selection strategy used
    pub strategy: String,
    /// Pattern used for clustering
    pub pattern: String,
}

impl ChunkMetadata {
    /// Create chunk metadata
    pub fn new(
        entities: Vec<Entity>,
        relevance_score: f32,
        selected_by_tier: u8,
        cluster_info: Option<ClusterInfo>,
    ) -> Self {
        Self {
            entities,
            relevance_score,
            selected_by_tier,
            cluster_info,
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Create from JSON string
    pub fn from_json(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// Get entity count
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Check if has sensitive data
    pub fn has_sensitive_data(&self) -> bool {
        self.entities.iter().any(|e| e.should_redact)
    }
}

/// Metadata enricher for processing entities and building context
pub struct MetadataEnricher {
    /// Correlation graph for relationship tracking
    graph: CorrelationGraph,
}

impl MetadataEnricher {
    /// Create new metadata enricher
    pub fn new() -> Self {
        Self {
            graph: CorrelationGraph::new(),
        }
    }

    /// Create metadata enricher with existing graph
    pub fn with_graph(graph: CorrelationGraph) -> Self {
        Self { graph }
    }

    /// Enrich capture with entities and update correlation graph
    pub fn enrich_capture(
        &mut self,
        entities: Vec<Entity>,
        tool: Option<String>,
        timestamp: i64,
    ) -> CaptureMetadata {
        // Update correlation graph
        self.graph.process_entities(&entities, timestamp);

        // Create metadata
        CaptureMetadata::from_entities(&entities, tool)
    }

    /// Create chunk metadata with entities
    pub fn create_chunk_metadata(
        &self,
        _chunk_text: &str,
        entities: Vec<Entity>,
        relevance_score: f32,
        selected_by_tier: u8,
        cluster_info: Option<ClusterInfo>,
    ) -> ChunkMetadata {
        ChunkMetadata::new(entities, relevance_score, selected_by_tier, cluster_info)
    }

    /// Get reference to correlation graph
    pub fn graph(&self) -> &CorrelationGraph {
        &self.graph
    }

    /// Get mutable reference to correlation graph
    pub fn graph_mut(&mut self) -> &mut CorrelationGraph {
        &mut self.graph
    }

    /// Export graph statistics
    pub fn export_stats(&self) -> serde_json::Value {
        let stats = self.graph.stats();
        serde_json::json!({
            "hosts": stats.host_count,
            "services": stats.service_count,
            "vulnerabilities": stats.vulnerability_count,
            "total_ports": stats.total_ports,
            "total_credentials": stats.total_credentials,
        })
    }

    /// Get all hosts from graph
    pub fn get_all_hosts(&self) -> Vec<HashMap<String, serde_json::Value>> {
        self.graph
            .get_all_hosts()
            .iter()
            .map(|host| {
                let mut map = HashMap::new();
                map.insert("identifier".to_string(), serde_json::json!(host.identifier));
                map.insert(
                    "ports".to_string(),
                    serde_json::json!(host.ports.iter().collect::<Vec<_>>()),
                );
                map.insert(
                    "vulnerabilities".to_string(),
                    serde_json::json!(host.vulnerabilities.iter().collect::<Vec<_>>()),
                );
                map.insert("first_seen".to_string(), serde_json::json!(host.first_seen));
                map.insert("last_seen".to_string(), serde_json::json!(host.last_seen));
                map
            })
            .collect()
    }
}

impl Default for MetadataEnricher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_entity(entity_type: &str, value: &str, should_redact: bool) -> Entity {
        Entity {
            entity_type: entity_type.to_string(),
            value: value.to_string(),
            context: format!("Context for {}", value),
            confidence: 0.9,
            should_redact,
        }
    }

    #[test]
    fn test_capture_metadata_basic() {
        let entities = vec![
            create_test_entity("ip_address", "192.168.1.1", false),
            create_test_entity("cve", "CVE-2021-44228", false),
        ];

        let metadata = CaptureMetadata::from_entities(&entities, Some("nmap".to_string()));

        assert_eq!(metadata.tool, Some("nmap".to_string()));
        assert_eq!(metadata.entity_count, 2);
        assert_eq!(metadata.hosts.len(), 1);
        assert_eq!(metadata.vulnerabilities.len(), 1);
        assert!(!metadata.has_sensitive_data);
    }

    #[test]
    fn test_capture_metadata_sensitive() {
        let entities = vec![
            create_test_entity("ip_address", "192.168.1.1", false),
            create_test_entity("credential_password", "admin:pass", true),
        ];

        let metadata = CaptureMetadata::from_entities(&entities, None);

        assert!(metadata.has_sensitive_data);
        assert_eq!(metadata.entity_types.len(), 2);
    }

    #[test]
    fn test_capture_metadata_serialization() {
        let entities = vec![create_test_entity("ip_address", "192.168.1.1", false)];

        let metadata = CaptureMetadata::from_entities(&entities, Some("nmap".to_string()));
        let json = metadata.to_json().unwrap();
        let deserialized = CaptureMetadata::from_json(&json).unwrap();

        assert_eq!(metadata.entity_count, deserialized.entity_count);
        assert_eq!(metadata.tool, deserialized.tool);
    }

    #[test]
    fn test_chunk_metadata_basic() {
        let entities = vec![create_test_entity("ip_address", "192.168.1.1", false)];

        let metadata = ChunkMetadata::new(entities, 0.95, 2, None);

        assert_eq!(metadata.relevance_score, 0.95);
        assert_eq!(metadata.selected_by_tier, 2);
        assert_eq!(metadata.entity_count(), 1);
        assert!(!metadata.has_sensitive_data());
    }

    #[test]
    fn test_chunk_metadata_with_cluster() {
        let entities = vec![create_test_entity("ip_address", "192.168.1.1", false)];

        let cluster_info = ClusterInfo {
            cluster_size: 10,
            strategy: "highest_entropy".to_string(),
            pattern: "ip_pattern".to_string(),
        };

        let metadata = ChunkMetadata::new(entities, 0.8, 3, Some(cluster_info));

        assert_eq!(metadata.selected_by_tier, 3);
        assert!(metadata.cluster_info.is_some());
        assert_eq!(metadata.cluster_info.as_ref().unwrap().cluster_size, 10);
    }

    #[test]
    fn test_chunk_metadata_serialization() {
        let entities = vec![create_test_entity("cve", "CVE-2021-44228", false)];

        let metadata = ChunkMetadata::new(entities, 0.9, 1, None);
        let json = metadata.to_json().unwrap();
        let deserialized = ChunkMetadata::from_json(&json).unwrap();

        assert_eq!(metadata.relevance_score, deserialized.relevance_score);
        assert_eq!(metadata.selected_by_tier, deserialized.selected_by_tier);
    }

    #[test]
    fn test_metadata_enricher_basic() {
        let mut enricher = MetadataEnricher::new();

        let entities = vec![
            create_test_entity("ip_address", "192.168.1.1", false),
            create_test_entity("port", "22/tcp", false),
        ];

        let metadata = enricher.enrich_capture(entities, Some("nmap".to_string()), 1000);

        assert_eq!(metadata.entity_count, 2);
        assert_eq!(metadata.hosts.len(), 1);

        // Verify graph was updated
        let host = enricher.graph().get_host("192.168.1.1").unwrap();
        assert_eq!(host.ports.len(), 1);
    }

    #[test]
    fn test_metadata_enricher_correlation() {
        let mut enricher = MetadataEnricher::new();

        // First capture
        let entities1 = vec![
            create_test_entity("ip_address", "192.168.1.1", false),
            create_test_entity("cve", "CVE-2021-44228", false),
        ];
        enricher.enrich_capture(entities1, Some("nmap".to_string()), 1000);

        // Second capture - same vulnerability, different host
        let entities2 = vec![
            create_test_entity("ip_address", "192.168.1.2", false),
            create_test_entity("cve", "CVE-2021-44228", false),
        ];
        enricher.enrich_capture(entities2, Some("nmap".to_string()), 2000);

        // Verify correlation
        let affected = enricher.graph().get_vulnerable_hosts("CVE-2021-44228");
        assert_eq!(affected.len(), 2);
    }

    #[test]
    fn test_metadata_enricher_stats() {
        let mut enricher = MetadataEnricher::new();

        let entities = vec![
            create_test_entity("ip_address", "192.168.1.1", false),
            create_test_entity("port", "22/tcp", false),
            create_test_entity("cve", "CVE-2021-44228", false),
        ];

        enricher.enrich_capture(entities, Some("nmap".to_string()), 1000);

        let stats = enricher.export_stats();
        assert_eq!(stats["hosts"], 1);
        assert_eq!(stats["vulnerabilities"], 1);
    }

    #[test]
    fn test_create_chunk_metadata() {
        let enricher = MetadataEnricher::new();
        let entities = vec![create_test_entity("ip_address", "192.168.1.1", false)];

        let metadata =
            enricher.create_chunk_metadata("Host 192.168.1.1 is up", entities, 0.95, 2, None);

        assert_eq!(metadata.relevance_score, 0.95);
        assert_eq!(metadata.entity_count(), 1);
    }

    #[test]
    fn test_get_all_hosts() {
        let mut enricher = MetadataEnricher::new();

        for i in 1..=3 {
            let entities = vec![create_test_entity(
                "ip_address",
                &format!("192.168.1.{}", i),
                false,
            )];
            enricher.enrich_capture(entities, None, 1000 + i as i64);
        }

        let hosts = enricher.get_all_hosts();
        assert_eq!(hosts.len(), 3);
    }
}
