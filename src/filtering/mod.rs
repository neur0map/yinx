// Three-tier filtering pipeline for capture output
//
// Tier 1: Hash-based deduplication (100K → 10K lines, 90% reduction)
// Tier 2: Statistical scoring (10K → 2K lines, 80% reduction)
// Tier 3: Semantic clustering (2K → 100 clusters, 95% reduction)

mod tier1;
mod tier2;
mod tier3;
mod types;
mod utils;

pub use tier1::{Tier1Filter, Tier1Stats};
pub use tier2::Tier2Filter;
pub use tier3::{RepresentativeStrategy, Tier3Filter};
pub use types::{Cluster, FilterDecision, FilterStats, ScoreComponents, ScoredLine};

use crate::error::Result;
use crate::patterns::PatternRegistry;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Main filtering pipeline orchestrator
/// Manages session-scoped Tier1 filters and coordinates all three tiers
pub struct FilterPipeline {
    patterns: Arc<PatternRegistry>,

    /// Session-scoped Tier1 filters (stateful deduplication)
    /// Key: session_id, Value: Tier1Filter wrapped in Mutex for interior mutability
    tier1_filters: Arc<Mutex<HashMap<String, Arc<Mutex<Tier1Filter>>>>>,
}

impl FilterPipeline {
    /// Create new filter pipeline
    ///
    /// # Arguments
    /// * `patterns` - Pattern registry with tier configurations
    pub fn new(patterns: Arc<PatternRegistry>) -> Self {
        Self {
            patterns,
            tier1_filters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Process capture output through three-tier pipeline
    ///
    /// # Arguments
    /// * `session_id` - Session identifier for stateful filtering
    /// * `output` - Raw capture output text
    ///
    /// # Returns
    /// Tuple of (clusters, statistics)
    pub fn process_capture(
        &self,
        session_id: &str,
        output: &str,
    ) -> Result<(Vec<Cluster>, FilterStats)> {
        let start = Instant::now();

        // Split output into lines
        let lines: Vec<String> = output.lines().map(|s| s.to_string()).collect();
        let input_count = lines.len();

        // Tier 1: Hash-based deduplication (stateful per session)
        let tier1_filter = self.get_or_create_tier1_filter(session_id);
        let tier1_output = {
            let mut filter = tier1_filter.lock().unwrap();
            filter.filter_lines(lines.into_iter())
        };
        let tier1_count = tier1_output.len();

        // Tier 2: Statistical scoring (stateless)
        let tier2_filter = Tier2Filter::new(self.patterns.clone());
        let tier2_output = tier2_filter.filter_lines(tier1_output);
        let tier2_count = tier2_output.len();

        // Extract lines from scored results
        let tier2_lines: Vec<String> = tier2_output.into_iter().map(|s| s.line).collect();

        // Tier 3: Semantic clustering (stateless)
        let tier3_filter = Tier3Filter::new(self.patterns.clone());
        let clusters = tier3_filter.cluster_lines(tier2_lines);
        let cluster_count = clusters.len();

        let stats = FilterStats {
            input_lines: input_count,
            tier1_output: tier1_count,
            tier2_output: tier2_count,
            tier3_clusters: cluster_count,
            processing_time_ms: start.elapsed().as_millis() as u64,
        };

        Ok((clusters, stats))
    }

    /// Get or create Tier1 filter for session
    fn get_or_create_tier1_filter(&self, session_id: &str) -> Arc<Mutex<Tier1Filter>> {
        let mut filters = self.tier1_filters.lock().unwrap();

        filters
            .entry(session_id.to_string())
            .or_insert_with(|| {
                let max_occurrences = self.patterns.tier1_config.max_occurrences;
                let filter = Tier1Filter::new(self.patterns.clone(), max_occurrences);
                Arc::new(Mutex::new(filter))
            })
            .clone()
    }

    /// Clear session filter state (called when session ends)
    ///
    /// # Arguments
    /// * `session_id` - Session identifier to clear
    pub fn clear_session(&self, session_id: &str) {
        let mut filters = self.tier1_filters.lock().unwrap();
        filters.remove(session_id);
    }

    /// Get number of active sessions being tracked
    pub fn active_sessions(&self) -> usize {
        let filters = self.tier1_filters.lock().unwrap();
        filters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{
        EntitiesConfig, FiltersConfig, NormalizationPattern, TechnicalPattern, Tier1Config,
        Tier2Config, Tier3Config, ToolsConfig,
    };

    fn create_test_patterns() -> Arc<PatternRegistry> {
        let entities = EntitiesConfig { entity: vec![] };
        let tools = ToolsConfig { tool: vec![] };

        let filters = FiltersConfig {
            tier1: Tier1Config {
                max_occurrences: 3,
                normalization_patterns: vec![NormalizationPattern {
                    name: "ip_address".to_string(),
                    pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                    replacement: "__IP__".to_string(),
                    priority: 1,
                }],
            },
            tier2: Tier2Config {
                entropy_weight: 0.25,
                uniqueness_weight: 0.25,
                technical_weight: 0.25,
                change_weight: 0.25,
                score_threshold_percentile: 0.5, // Keep top 50% for testing
                max_technical_score: 10.0,
                technical_patterns: vec![TechnicalPattern {
                    name: "cve".to_string(),
                    pattern: r"CVE-\d{4}-\d{4,}".to_string(),
                    weight: 2.0,
                }],
            },
            tier3: Tier3Config {
                cluster_min_size: 2,
                max_cluster_size: 1000,
                representative_strategy: "highest_entropy".to_string(),
                cluster_patterns: vec![NormalizationPattern {
                    name: "numbers".to_string(),
                    pattern: r"\d+".to_string(),
                    replacement: "__NUM__".to_string(),
                    priority: 1,
                }],
                preserve_metadata: vec![],
            },
        };

        Arc::new(
            PatternRegistry::from_configs(entities, tools, filters)
                .expect("Failed to create test patterns"),
        )
    }

    #[test]
    fn test_pipeline_basic() {
        let patterns = create_test_patterns();
        let pipeline = FilterPipeline::new(patterns);

        let output = "Line 1\nLine 2\nLine 3\n";
        let (clusters, stats) = pipeline.process_capture("test-session", output).unwrap();

        assert_eq!(stats.input_lines, 3);
        assert!(stats.tier1_output <= 3);
        assert!(!clusters.is_empty());
    }

    #[test]
    fn test_pipeline_deduplication_across_captures() {
        let patterns = create_test_patterns();
        let pipeline = FilterPipeline::new(patterns);

        // First capture
        let output1 = "Repeated line\nRepeated line\nRepeated line\n";
        let (_, stats1) = pipeline.process_capture("session1", output1).unwrap();

        // Should keep all 3 (max_occurrences = 3)
        assert_eq!(stats1.tier1_output, 3);

        // Second capture in same session
        let output2 = "Repeated line\nRepeated line\nNew line\n";
        let (_, stats2) = pipeline.process_capture("session1", output2).unwrap();

        // "Repeated line" already seen 3 times, should be filtered
        // Only "New line" should pass Tier 1
        assert_eq!(stats2.tier1_output, 1);
    }

    #[test]
    fn test_pipeline_session_isolation() {
        let patterns = create_test_patterns();
        let pipeline = FilterPipeline::new(patterns);

        let output = "Test line\nTest line\nTest line\n";

        // Session 1
        let (_, stats1) = pipeline.process_capture("session1", output).unwrap();
        assert_eq!(stats1.tier1_output, 3);

        // Session 2 - should have independent deduplication
        let (_, stats2) = pipeline.process_capture("session2", output).unwrap();
        assert_eq!(stats2.tier1_output, 3);
    }

    #[test]
    fn test_pipeline_session_cleanup() {
        let patterns = create_test_patterns();
        let pipeline = FilterPipeline::new(patterns);

        pipeline.process_capture("session1", "Test\n").unwrap();
        pipeline.process_capture("session2", "Test\n").unwrap();

        assert_eq!(pipeline.active_sessions(), 2);

        pipeline.clear_session("session1");
        assert_eq!(pipeline.active_sessions(), 1);

        pipeline.clear_session("session2");
        assert_eq!(pipeline.active_sessions(), 0);
    }

    #[test]
    fn test_pipeline_empty_output() {
        let patterns = create_test_patterns();
        let pipeline = FilterPipeline::new(patterns);

        let (clusters, stats) = pipeline.process_capture("test", "").unwrap();

        assert_eq!(stats.input_lines, 0);
        assert_eq!(stats.tier1_output, 0);
        assert_eq!(stats.tier2_output, 0);
        assert_eq!(stats.tier3_clusters, 0);
        assert!(clusters.is_empty());
    }

    #[test]
    fn test_pipeline_single_line() {
        let patterns = create_test_patterns();
        let pipeline = FilterPipeline::new(patterns);

        let (clusters, stats) = pipeline.process_capture("test", "Single line\n").unwrap();

        assert_eq!(stats.input_lines, 1);
        assert_eq!(stats.tier1_output, 1);
        // Tier 2 with 50% threshold on 1 line = 1 line passes
        assert_eq!(stats.tier2_output, 1);
        // Tier 3 with cluster_min_size=2, single line becomes singleton
        assert_eq!(clusters.len(), 1);
    }

    #[test]
    fn test_pipeline_performance() {
        let patterns = create_test_patterns();
        let pipeline = FilterPipeline::new(patterns);

        // Generate 1000 lines
        let lines: Vec<String> = (0..1000)
            .map(|i| format!("Test line {} with some content", i))
            .collect();
        let output = lines.join("\n");

        let (_, stats) = pipeline.process_capture("perf-test", &output).unwrap();

        // Should complete quickly
        assert!(stats.processing_time_ms < 100);

        // Should achieve significant reduction
        let reduction = 1.0 - (stats.tier3_clusters as f32 / stats.input_lines as f32);
        assert!(reduction > 0.5); // At least 50% reduction
    }
}
