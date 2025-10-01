// Tier 1: Hash-based deduplication filter
// Normalizes content using patterns and tracks occurrence counts
use ahash::{HashMap, HashMapExt};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::filtering::types::FilterDecision;
use crate::patterns::PatternRegistry;

/// Hash-based deduplication filter
/// Maintains state across captures within a session to track pattern occurrences
pub struct Tier1Filter {
    /// Reference to pattern registry for normalization
    patterns: Arc<PatternRegistry>,

    /// Maximum occurrences before discarding (from config)
    max_occurrences: u32,

    /// Pattern hash -> occurrence count
    /// Key: hash of normalized line
    /// Value: number of times seen in this session
    pattern_counts: HashMap<u64, u32>,
}

impl Tier1Filter {
    /// Create new Tier 1 filter with configuration
    ///
    /// # Arguments
    /// * `patterns` - Pattern registry for normalization
    /// * `max_occurrences` - Maximum times a pattern can occur before being discarded
    pub fn new(patterns: Arc<PatternRegistry>, max_occurrences: u32) -> Self {
        Self {
            patterns,
            max_occurrences,
            pattern_counts: HashMap::new(),
        }
    }

    /// Process a single line through the deduplication filter
    /// This method is stateful - updates internal occurrence counts
    ///
    /// # Arguments
    /// * `line` - The line to process
    ///
    /// # Returns
    /// FilterDecision::Keep if line should be kept, Discard otherwise
    pub fn process_line(&mut self, line: &str) -> FilterDecision {
        // Normalize using patterns from config (replace IPs, timestamps, etc.)
        let normalized = self.patterns.normalize_tier1(line);

        // Hash normalized pattern using fast non-crypto hash
        let hash = self.hash_pattern(&normalized);

        // Update occurrence count
        let count = self.pattern_counts.entry(hash).or_insert(0);
        *count += 1;

        // Make decision based on count
        if *count <= self.max_occurrences {
            FilterDecision::Keep
        } else {
            FilterDecision::Discard
        }
    }

    /// Process batch of lines (streaming filter)
    ///
    /// # Arguments
    /// * `lines` - Iterator of lines to filter
    ///
    /// # Returns
    /// Vector of lines that passed the deduplication filter
    pub fn filter_lines(&mut self, lines: impl Iterator<Item = String>) -> Vec<String> {
        lines
            .filter(|line| self.process_line(line) == FilterDecision::Keep)
            .collect()
    }

    /// Hash a normalized pattern using fast non-crypto hash (AHash)
    fn hash_pattern(&self, pattern: &str) -> u64 {
        use ahash::AHasher;
        let mut hasher = AHasher::default();
        pattern.hash(&mut hasher);
        hasher.finish()
    }

    /// Clear state (called when session ends)
    pub fn reset(&mut self) {
        self.pattern_counts.clear();
    }

    /// Get statistics about current filter state
    pub fn stats(&self) -> Tier1Stats {
        Tier1Stats {
            unique_patterns: self.pattern_counts.len(),
            total_occurrences: self.pattern_counts.values().sum(),
        }
    }
}

/// Statistics from Tier 1 filter
#[derive(Debug, Clone)]
pub struct Tier1Stats {
    /// Number of unique patterns seen
    pub unique_patterns: usize,
    /// Total number of occurrences across all patterns
    pub total_occurrences: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::PatternRegistry;

    fn create_test_patterns() -> Arc<PatternRegistry> {
        // Create minimal test patterns
        use crate::patterns::{
            EntitiesConfig, FiltersConfig, NormalizationPattern, Tier1Config, Tier2Config,
            Tier3Config, ToolsConfig,
        };

        let entities = EntitiesConfig { entity: vec![] };
        let tools = ToolsConfig { tool: vec![] };

        // Create basic tier1 normalization pattern for IPs
        let tier1_config = Tier1Config {
            max_occurrences: 3,
            normalization_patterns: vec![NormalizationPattern {
                name: "ip_address".to_string(),
                pattern: r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b".to_string(),
                replacement: "__IP__".to_string(),
                priority: 1,
            }],
        };

        let filters = FiltersConfig {
            tier1: tier1_config,
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
    fn test_tier1_deduplication_basic() {
        let patterns = create_test_patterns();
        let mut filter = Tier1Filter::new(patterns, 3);

        // Same line should be kept 3 times, then discarded
        assert_eq!(filter.process_line("test line"), FilterDecision::Keep);
        assert_eq!(filter.process_line("test line"), FilterDecision::Keep);
        assert_eq!(filter.process_line("test line"), FilterDecision::Keep);
        assert_eq!(filter.process_line("test line"), FilterDecision::Discard);
    }

    #[test]
    fn test_tier1_normalization() {
        let patterns = create_test_patterns();
        let mut filter = Tier1Filter::new(patterns, 2);

        // Different IPs should normalize to same pattern
        assert_eq!(
            filter.process_line("Host: 192.168.1.1"),
            FilterDecision::Keep
        );
        assert_eq!(filter.process_line("Host: 10.0.0.1"), FilterDecision::Keep);
        // Third occurrence should be discarded
        assert_eq!(
            filter.process_line("Host: 172.16.0.1"),
            FilterDecision::Discard
        );
    }

    #[test]
    fn test_tier1_stateful_tracking() {
        let patterns = create_test_patterns();
        let mut filter = Tier1Filter::new(patterns, 2);

        filter.process_line("line1");
        filter.process_line("line2");

        let stats = filter.stats();
        assert_eq!(stats.unique_patterns, 2);
        assert_eq!(stats.total_occurrences, 2);
    }

    #[test]
    fn test_tier1_reset() {
        let patterns = create_test_patterns();
        let mut filter = Tier1Filter::new(patterns, 2);

        filter.process_line("test");
        filter.process_line("test");

        filter.reset();

        let stats = filter.stats();
        assert_eq!(stats.unique_patterns, 0);
        assert_eq!(stats.total_occurrences, 0);
    }

    #[test]
    fn test_tier1_filter_lines() {
        let patterns = create_test_patterns();
        let mut filter = Tier1Filter::new(patterns, 2);

        let lines = vec![
            "line1".to_string(),
            "line1".to_string(),
            "line1".to_string(), // Should be discarded
            "line2".to_string(),
        ];

        let filtered = filter.filter_lines(lines.into_iter());
        assert_eq!(filtered.len(), 3); // line1 (2x) + line2 (1x)
    }
}
