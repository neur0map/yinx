// Tier 3: Semantic clustering filter
// Groups similar lines and selects representatives
use std::collections::HashMap;
use std::sync::Arc;

use crate::filtering::types::Cluster;
use crate::filtering::utils;
use crate::patterns::PatternRegistry;

/// Semantic clustering filter
/// Groups similar lines based on normalized patterns and selects representatives
pub struct Tier3Filter {
    patterns: Arc<PatternRegistry>,
}

/// Representative selection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepresentativeStrategy {
    /// Select first item in cluster
    First,
    /// Select longest item
    Longest,
    /// Select item with highest entropy
    HighestEntropy,
}

impl RepresentativeStrategy {
    /// Parse strategy from configuration string
    pub fn parse_strategy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "first" => Self::First,
            "longest" => Self::Longest,
            "highest_entropy" => Self::HighestEntropy,
            _ => Self::HighestEntropy, // Default
        }
    }
}

impl Tier3Filter {
    /// Create Tier 3 filter from pattern registry
    pub fn new(patterns: Arc<PatternRegistry>) -> Self {
        Self { patterns }
    }

    /// Cluster lines and select representatives
    ///
    /// # Algorithm
    /// 1. Group lines by normalized pattern
    /// 2. Handle small clusters (below min_size): keep as singletons
    /// 3. Handle large clusters (above max_size): split into chunks
    /// 4. For normal clusters: select representative based on strategy
    ///
    /// # Arguments
    /// * `lines` - Vector of lines to cluster
    ///
    /// # Returns
    /// Vector of clusters with selected representatives
    pub fn cluster_lines(&self, lines: Vec<String>) -> Vec<Cluster> {
        if lines.is_empty() {
            return Vec::new();
        }

        let config = &self.patterns.tier3_config;
        let cluster_min_size = config.cluster_min_size;
        let max_cluster_size = config.max_cluster_size;
        let strategy = RepresentativeStrategy::parse_strategy(&config.representative_strategy);

        // Phase 1: Group by normalized pattern
        let mut clusters: HashMap<String, Vec<String>> = HashMap::new();

        for line in lines {
            let pattern = self.patterns.normalize_tier3(&line);
            clusters.entry(pattern).or_default().push(line);
        }

        // Phase 2: Process clusters and select representatives
        let mut result: Vec<Cluster> = Vec::new();

        for (pattern, members) in clusters {
            let size = members.len();

            // Handle small clusters (keep all as separate items)
            if size < cluster_min_size {
                for member in members {
                    result.push(Cluster {
                        pattern: pattern.clone(),
                        representative: member.clone(),
                        members: vec![member],
                        size: 1,
                        metadata: serde_json::json!({ "singleton": true }),
                    });
                }
                continue;
            }

            // Handle large clusters (split if needed)
            if size > max_cluster_size {
                for chunk in members.chunks(max_cluster_size) {
                    let representative = self.select_representative(chunk, strategy);
                    result.push(Cluster {
                        pattern: pattern.clone(),
                        representative,
                        members: chunk.to_vec(),
                        size: chunk.len(),
                        metadata: serde_json::json!({ "split": true }),
                    });
                }
                continue;
            }

            // Normal clustering
            let representative = self.select_representative(&members, strategy);
            result.push(Cluster {
                pattern: pattern.clone(),
                representative,
                members,
                size,
                metadata: serde_json::json!({
                    "count": size,
                }),
            });
        }

        result
    }

    /// Select representative from cluster members based on strategy
    fn select_representative(
        &self,
        members: &[String],
        strategy: RepresentativeStrategy,
    ) -> String {
        match strategy {
            RepresentativeStrategy::First => members[0].clone(),

            RepresentativeStrategy::Longest => {
                members.iter().max_by_key(|s| s.len()).unwrap().clone()
            }

            RepresentativeStrategy::HighestEntropy => members
                .iter()
                .map(|s| (s, utils::shannon_entropy(s)))
                .max_by(|(_, e1), (_, e2)| e1.partial_cmp(e2).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap()
                .0
                .clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{
        EntitiesConfig, FiltersConfig, NormalizationPattern, Tier1Config, Tier2Config, Tier3Config,
        ToolsConfig,
    };

    fn create_test_patterns() -> Arc<PatternRegistry> {
        let entities = EntitiesConfig { entity: vec![] };
        let tools = ToolsConfig { tool: vec![] };

        let filters = FiltersConfig {
            tier1: Tier1Config {
                max_occurrences: 3,
                normalization_patterns: vec![],
            },
            tier2: Tier2Config {
                entropy_weight: 0.25,
                uniqueness_weight: 0.25,
                technical_weight: 0.25,
                change_weight: 0.25,
                score_threshold_percentile: 0.8,
                max_technical_score: 10.0,
                technical_patterns: vec![],
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
    fn test_tier3_clustering() {
        let patterns = create_test_patterns();
        let filter = Tier3Filter::new(patterns);

        let lines = vec![
            "Port 80 open".to_string(),
            "Port 443 open".to_string(),
            "Port 8080 open".to_string(),
            "Different line entirely".to_string(),
        ];

        let clusters = filter.cluster_lines(lines);

        // Should cluster port lines together (numbers normalized to __NUM__)
        // Pattern "Port __NUM__ open" should have 3 members
        let port_cluster = clusters
            .iter()
            .find(|c| c.pattern.contains("Port") && c.size == 3);

        assert!(port_cluster.is_some(), "Port lines should be clustered");
    }

    #[test]
    fn test_tier3_representative_strategies() {
        let patterns = create_test_patterns();
        let filter = Tier3Filter::new(patterns);

        let members = vec![
            "short".to_string(),
            "medium length".to_string(),
            "very long line with more text".to_string(),
        ];

        // Test longest strategy
        let rep = filter.select_representative(&members, RepresentativeStrategy::Longest);
        assert_eq!(rep, "very long line with more text");

        // Test first strategy
        let rep = filter.select_representative(&members, RepresentativeStrategy::First);
        assert_eq!(rep, "short");

        // Test highest entropy strategy
        let rep = filter.select_representative(&members, RepresentativeStrategy::HighestEntropy);
        // Should select the most varied line
        assert!(rep.len() > 5);
    }

    #[test]
    fn test_tier3_cluster_size_limits() {
        let entities = EntitiesConfig { entity: vec![] };
        let tools = ToolsConfig { tool: vec![] };

        let filters = FiltersConfig {
            tier1: Tier1Config {
                max_occurrences: 3,
                normalization_patterns: vec![],
            },
            tier2: Tier2Config {
                entropy_weight: 0.25,
                uniqueness_weight: 0.25,
                technical_weight: 0.25,
                change_weight: 0.25,
                score_threshold_percentile: 0.8,
                max_technical_score: 10.0,
                technical_patterns: vec![],
            },
            tier3: Tier3Config {
                cluster_min_size: 3, // Increase min size
                max_cluster_size: 1000,
                representative_strategy: "first".to_string(),
                cluster_patterns: vec![],
                preserve_metadata: vec![],
            },
        };

        let patterns = Arc::new(
            PatternRegistry::from_configs(entities, tools, filters)
                .expect("Failed to create test patterns"),
        );

        let filter = Tier3Filter::new(patterns);

        let lines = vec![
            "line1".to_string(),
            "line1".to_string(), // Only 2, below min
            "line2".to_string(),
            "line2".to_string(),
            "line2".to_string(), // 3, meets min
        ];

        let clusters = filter.cluster_lines(lines);

        // line1 members should be kept as singletons (below min_size)
        let singletons: Vec<_> = clusters
            .iter()
            .filter(|c| c.metadata.get("singleton").is_some())
            .collect();

        assert_eq!(singletons.len(), 2, "Should have 2 singletons");

        // line2 should be clustered
        let line2_cluster = clusters.iter().find(|c| c.size == 3);
        assert!(line2_cluster.is_some(), "Should have cluster of size 3");
    }

    #[test]
    fn test_tier3_empty_input() {
        let patterns = create_test_patterns();
        let filter = Tier3Filter::new(patterns);

        let clusters = filter.cluster_lines(vec![]);
        assert_eq!(clusters.len(), 0);
    }

    #[test]
    fn test_tier3_single_line() {
        let patterns = create_test_patterns();
        let filter = Tier3Filter::new(patterns);

        let lines = vec!["single line".to_string()];
        let clusters = filter.cluster_lines(lines);

        // Single line below min_size should be singleton
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].size, 1);
        assert_eq!(clusters[0].representative, "single line");
    }

    #[test]
    fn test_representative_strategy_parsing() {
        assert_eq!(
            RepresentativeStrategy::parse_strategy("first"),
            RepresentativeStrategy::First
        );
        assert_eq!(
            RepresentativeStrategy::parse_strategy("longest"),
            RepresentativeStrategy::Longest
        );
        assert_eq!(
            RepresentativeStrategy::parse_strategy("highest_entropy"),
            RepresentativeStrategy::HighestEntropy
        );
        assert_eq!(
            RepresentativeStrategy::parse_strategy("invalid"),
            RepresentativeStrategy::HighestEntropy
        ); // Default
    }
}
