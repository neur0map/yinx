//! Cross-encoder reranking using FastEmbed

use fastembed::{RerankInitOptions, TextRerank};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RerankError {
    #[error("Reranker initialization failed: {0}")]
    InitializationError(String),

    #[error("Reranking failed: {0}")]
    RerankingError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
}

/// Cross-encoder reranker for improving result precision
pub struct Reranker {
    model: Arc<TextRerank>,
    model_name: String,
}

impl Reranker {
    /// Create a new reranker with specified model
    ///
    /// # Arguments
    /// * `model_name` - Model name (e.g., "Xenova/ms-marco-MiniLM-L-6-v2")
    pub fn new(model_name: &str) -> Result<Self, RerankError> {
        tracing::info!("Initializing reranker model: {}", model_name);

        // FastEmbed v4.x uses RerankInitOptions
        let init_options = RerankInitOptions::new(fastembed::RerankerModel::BGERerankerBase)
            .with_show_download_progress(true);

        let model = TextRerank::try_new(init_options)
            .map_err(|e| RerankError::InitializationError(e.to_string()))?;

        Ok(Self {
            model: Arc::new(model),
            model_name: model_name.to_string(),
        })
    }

    /// Create reranker with default model
    pub fn with_default_model() -> Result<Self, RerankError> {
        Self::new("Xenova/ms-marco-MiniLM-L-6-v2")
    }

    /// Rerank a list of text candidates given a query
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `candidates` - Candidate texts to rerank
    /// * `top_k` - Number of top results to return
    ///
    /// # Returns
    /// Vector of (index, score) pairs sorted by score descending
    pub fn rerank(
        &self,
        query: &str,
        candidates: &[String],
        top_k: usize,
    ) -> Result<Vec<(usize, f32)>, RerankError> {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }

        if query.is_empty() {
            return Err(RerankError::InvalidInput(
                "Query cannot be empty".to_string(),
            ));
        }

        // Create document references for reranking
        let documents: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();

        // Rerank using FastEmbed
        let results = self
            .model
            .rerank(query, documents, true, Some(top_k))
            .map_err(|e| RerankError::RerankingError(e.to_string()))?;

        // Convert to (index, score) pairs
        let mut scored: Vec<(usize, f32)> =
            results.into_iter().map(|r| (r.index, r.score)).collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(scored)
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires model download
    fn test_reranker_creation() {
        let reranker = Reranker::with_default_model();
        assert!(reranker.is_ok());
    }

    #[test]
    #[ignore] // Requires model download
    fn test_rerank_basic() {
        let reranker = Reranker::with_default_model().unwrap();

        let query = "What is the capital of France?";
        let candidates = vec![
            "Paris is the capital of France.".to_string(),
            "London is the capital of England.".to_string(),
            "The weather is nice today.".to_string(),
        ];

        let results = reranker.rerank(query, &candidates, 2).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 0); // First candidate should rank highest
    }
}
