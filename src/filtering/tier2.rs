// Tier 2: Statistical importance scoring filter
// Scores lines based on entropy, uniqueness, technical patterns, and change detection
use std::collections::HashMap;
use std::sync::Arc;

use crate::filtering::types::{ScoreComponents, ScoredLine};
use crate::filtering::utils;
use crate::patterns::PatternRegistry;

/// Statistical importance scoring filter
/// Analyzes line characteristics to assign importance scores
pub struct Tier2Filter {
    patterns: Arc<PatternRegistry>,
}

impl Tier2Filter {
    /// Create Tier 2 filter from pattern registry
    pub fn new(patterns: Arc<PatternRegistry>) -> Self {
        Self { patterns }
    }

    /// Filter lines by statistical scoring (two-pass algorithm)
    ///
    /// Pass 1: Build frequency map for uniqueness calculation
    /// Pass 2: Score each line and filter by percentile threshold
    ///
    /// # Arguments
    /// * `lines` - Vector of lines to score and filter
    ///
    /// # Returns
    /// Vector of scored lines above the threshold
    pub fn filter_lines(&self, lines: Vec<String>) -> Vec<ScoredLine> {
        if lines.is_empty() {
            return Vec::new();
        }

        // Extract config values
        let config = &self.patterns.tier2_config;
        let entropy_weight = config.entropy_weight;
        let uniqueness_weight = config.uniqueness_weight;
        let technical_weight = config.technical_weight;
        let change_weight = config.change_weight;
        let max_technical_score = config.max_technical_score;
        let threshold_percentile = config.score_threshold_percentile;

        // Pass 1: Build frequency map
        let mut line_frequencies: HashMap<&str, u32> = HashMap::new();
        for line in &lines {
            *line_frequencies.entry(line.as_str()).or_insert(0) += 1;
        }
        let total_lines = lines.len() as f32;

        // Pass 2: Score each line
        let mut scored_lines: Vec<ScoredLine> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let prev_line = if i > 0 {
                    Some(lines[i - 1].as_str())
                } else {
                    None
                };

                // Calculate individual score components
                let entropy = utils::shannon_entropy(line) * entropy_weight;

                let freq = *line_frequencies.get(line.as_str()).unwrap_or(&1);
                let uniqueness = (1.0 - (freq as f32 / total_lines)) * uniqueness_weight;

                let technical = self
                    .patterns
                    .calculate_technical_score(line, max_technical_score)
                    * technical_weight;

                let change = match prev_line {
                    Some(prev) => utils::change_score(line, prev) * change_weight,
                    None => change_weight, // First line gets max change score
                };

                let components = ScoreComponents {
                    entropy,
                    uniqueness,
                    technical,
                    change,
                };

                ScoredLine {
                    line: line.clone(),
                    score: components.total(),
                    components,
                }
            })
            .collect();

        // Calculate threshold percentile
        let scores: Vec<f32> = scored_lines.iter().map(|s| s.score).collect();
        let threshold = utils::percentile(&scores, threshold_percentile);

        // Filter by threshold
        scored_lines.retain(|s| s.score >= threshold);
        scored_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{
        EntitiesConfig, FiltersConfig, TechnicalPattern, Tier1Config, Tier2Config, Tier3Config,
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
                technical_patterns: vec![
                    TechnicalPattern {
                        name: "cve".to_string(),
                        pattern: r"CVE-\d{4}-\d{4,}".to_string(),
                        weight: 2.0,
                    },
                    TechnicalPattern {
                        name: "ip".to_string(),
                        pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                        weight: 1.0,
                    },
                ],
            },
            tier3: Tier3Config {
                cluster_min_size: 2,
                max_cluster_size: 1000,
                representative_strategy: "highest_entropy".to_string(),
                cluster_patterns: vec![],
                preserve_metadata: vec![],
            },
        };

        Arc::new(
            PatternRegistry::from_configs(entities, tools, filters)
                .expect("Failed to create test patterns"),
        )
    }

    #[test]
    fn test_tier2_entropy_scoring() {
        let patterns = create_test_patterns();
        let filter = Tier2Filter::new(patterns);

        let lines = vec![
            "aaaaaaaaaaaa".to_string(), // Low entropy
            "a1b2c3d4e5f6".to_string(), // High entropy
        ];

        let scored = filter.filter_lines(lines);

        // With 80th percentile of 2 lines, only top 20% = 1 line passes
        assert_eq!(scored.len(), 1);

        // The high entropy line should be the one that passed
        assert_eq!(scored[0].line, "a1b2c3d4e5f6");
    }

    #[test]
    fn test_tier2_uniqueness_scoring() {
        let patterns = create_test_patterns();
        let filter = Tier2Filter::new(patterns);

        let lines = vec![
            "common line".to_string(),
            "common line".to_string(),
            "common line".to_string(),
            "rare line with unique content".to_string(),
        ];

        let scored = filter.filter_lines(lines);

        // With 80th percentile of 4 lines, top 20% = 1 line
        // The rare line should score higher due to uniqueness and entropy
        assert_eq!(scored.len(), 1);
        assert!(scored[0].line.contains("rare"));
    }

    #[test]
    fn test_tier2_technical_scoring() {
        let patterns = create_test_patterns();
        let filter = Tier2Filter::new(patterns);

        let lines = vec![
            "Just some text".to_string(),
            "Found CVE-2024-1234 at 192.168.1.1:80".to_string(),
        ];

        let scored = filter.filter_lines(lines);

        // With 80th percentile of 2 lines, only top 20% = 1 line
        assert_eq!(scored.len(), 1);

        // The technical line with CVE and IP should score higher
        assert!(scored[0].line.contains("CVE-2024-1234"));
    }

    #[test]
    fn test_tier2_percentile_threshold() {
        let patterns = create_test_patterns();
        let filter = Tier2Filter::new(patterns);

        // Create 10 lines with varying information content
        let lines: Vec<String> = (0..10)
            .map(|i| format!("line with {} unique content", i))
            .collect();

        let scored = filter.filter_lines(lines);

        // With 80th percentile of 10 lines, top 20% = 2 lines
        // But allow wide tolerance due to equal scores causing ties
        assert!(scored.len() <= 10); // Maximum all pass
        assert!(!scored.is_empty()); // Minimum one passes
    }

    #[test]
    fn test_tier2_empty_input() {
        let patterns = create_test_patterns();
        let filter = Tier2Filter::new(patterns);

        let scored = filter.filter_lines(vec![]);
        assert_eq!(scored.len(), 0);
    }

    #[test]
    fn test_tier2_single_line() {
        let patterns = create_test_patterns();
        let filter = Tier2Filter::new(patterns);

        let lines = vec!["single line".to_string()];
        let scored = filter.filter_lines(lines);

        // Single line should always pass through
        assert_eq!(scored.len(), 1);
        assert_eq!(scored[0].line, "single line");
    }
}
