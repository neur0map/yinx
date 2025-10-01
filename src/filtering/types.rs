// Shared types for filtering pipeline
use serde::{Deserialize, Serialize};

/// Decision from Tier 1 hash-based deduplication filter
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterDecision {
    /// Keep this line (not yet exceeded max occurrences)
    Keep,
    /// Discard this line (exceeded max occurrences)
    Discard,
}

/// Line with statistical score from Tier 2 filter
#[derive(Debug, Clone)]
pub struct ScoredLine {
    /// The actual line content
    pub line: String,
    /// Total composite score
    pub score: f32,
    /// Individual score components for debugging/tuning
    pub components: ScoreComponents,
}

/// Breakdown of score components for transparency
#[derive(Debug, Clone)]
pub struct ScoreComponents {
    /// Shannon entropy score (weighted)
    pub entropy: f32,
    /// Inverse frequency score (weighted)
    pub uniqueness: f32,
    /// Technical pattern density score (weighted)
    pub technical: f32,
    /// Change from previous line score (weighted)
    pub change: f32,
}

impl ScoreComponents {
    /// Calculate total score by summing all components
    pub fn total(&self) -> f32 {
        self.entropy + self.uniqueness + self.technical + self.change
    }
}

/// Cluster output from Tier 3 semantic clustering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    /// Normalized pattern that defines this cluster
    pub pattern: String,

    /// Selected representative line (for embedding/display)
    pub representative: String,

    /// All member lines in this cluster
    pub members: Vec<String>,

    /// Cluster size
    pub size: usize,

    /// Additional metadata (timestamps, entropy, etc.)
    pub metadata: serde_json::Value,
}

/// Statistics from filtering operation
#[derive(Debug, Clone, Default)]
pub struct FilterStats {
    /// Number of input lines
    pub input_lines: usize,
    /// Number of lines after Tier 1 deduplication
    pub tier1_output: usize,
    /// Number of lines after Tier 2 scoring
    pub tier2_output: usize,
    /// Number of clusters from Tier 3
    pub tier3_clusters: usize,
    /// Total processing time in milliseconds
    pub processing_time_ms: u64,
}
