//! Reciprocal Rank Fusion algorithm for combining search results

use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FusionError {
    #[error("Invalid weight configuration: weights must be positive")]
    InvalidWeights,

    #[error("Empty result sets provided")]
    EmptyResults,
}

/// Configuration for fusion algorithm
#[derive(Debug, Clone)]
pub struct FusionConfig {
    /// RRF K constant (typically 60)
    pub rrf_k: f32,

    /// Weight for semantic results
    pub semantic_weight: f32,

    /// Weight for keyword results
    pub keyword_weight: f32,
}

impl FusionConfig {
    pub fn new(rrf_k: f32, semantic_weight: f32, keyword_weight: f32) -> Result<Self, FusionError> {
        if semantic_weight <= 0.0 || keyword_weight <= 0.0 {
            return Err(FusionError::InvalidWeights);
        }

        Ok(Self {
            rrf_k,
            semantic_weight,
            keyword_weight,
        })
    }
}

/// Apply Reciprocal Rank Fusion to combine two ranked lists
///
/// RRF formula: score(id) = sum over all rankings of: weight / (k + rank)
///
/// # Arguments
/// * `semantic_results` - (id, original_score) pairs from semantic search
/// * `keyword_results` - (id, original_score) pairs from keyword search
/// * `config` - Fusion configuration
///
/// # Returns
/// Fused results as (id, fused_score) pairs, sorted by score descending
pub fn reciprocal_rank_fusion(
    semantic_results: Vec<(i64, f32)>,
    keyword_results: Vec<(i64, f32)>,
    config: &FusionConfig,
) -> Vec<(i64, f32)> {
    let mut scores: HashMap<i64, f32> = HashMap::new();

    // Process semantic results
    for (rank, (chunk_id, _original_score)) in semantic_results.iter().enumerate() {
        let rrf_score = config.semantic_weight / (config.rrf_k + (rank as f32) + 1.0);
        *scores.entry(*chunk_id).or_insert(0.0) += rrf_score;
    }

    // Process keyword results
    for (rank, (chunk_id, _original_score)) in keyword_results.iter().enumerate() {
        let rrf_score = config.keyword_weight / (config.rrf_k + (rank as f32) + 1.0);
        *scores.entry(*chunk_id).or_insert(0.0) += rrf_score;
    }

    // Sort by fused score descending
    let mut results: Vec<(i64, f32)> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_basic() {
        let semantic = vec![(1, 0.9), (2, 0.8), (3, 0.7)];
        let keyword = vec![(2, 0.95), (1, 0.85), (4, 0.75)];

        let config = FusionConfig::new(60.0, 1.0, 1.0).unwrap();
        let fused = reciprocal_rank_fusion(semantic, keyword, &config);

        // Results should be fused and sorted
        assert!(fused.len() >= 3);

        // ID 1 and 2 appear in both lists, should rank higher
        assert!(fused[0].0 == 1 || fused[0].0 == 2);
    }

    #[test]
    fn test_rrf_weighted() {
        let semantic = vec![(1, 0.9)];
        let keyword = vec![(2, 0.9)];

        // Prefer semantic
        let config = FusionConfig::new(60.0, 0.7, 0.3).unwrap();
        let fused = reciprocal_rank_fusion(semantic.clone(), keyword.clone(), &config);

        assert_eq!(fused[0].0, 1); // Semantic result should win
    }
}
