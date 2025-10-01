mod batch;
mod keyword_index;
/// Phase 6: Embedding & Indexing
///
/// This module provides local embedding generation and hybrid search capabilities.
/// Architecture:
/// - EmbeddingProvider trait for abstraction
/// - FastEmbedProvider for local embedding (all-MiniLM-L6-v2, 384-dim)
/// - HNSW for vector similarity search
/// - Tantivy for keyword search
/// - Batch processing for efficiency
mod provider;
mod vector_index;

pub use batch::{BatchItem, BatchProcessor, BatchResult};
pub use keyword_index::{KeywordIndex, KeywordIndexError, KeywordSearchResult};
pub use provider::{EmbeddingError, EmbeddingProvider, FastEmbedProvider};
pub use vector_index::{SearchResult, VectorIndex, VectorIndexError};

use serde::{Deserialize, Serialize};

/// Configuration for embedding generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Model name (e.g., "all-MiniLM-L6-v2")
    pub model: String,
    /// Embedding dimension (384 for MiniLM)
    pub dimension: usize,
    /// Batch size for processing
    pub batch_size: usize,
    /// Operating mode: "offline" or "online"
    pub mode: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "all-MiniLM-L6-v2".to_string(),
            dimension: 384,
            batch_size: 32,
            mode: "offline".to_string(),
        }
    }
}

/// Configuration for HNSW vector index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    /// Vector dimension (must match embedding dimension)
    pub vector_dim: usize,
    /// HNSW construction parameter (higher = better recall, slower build)
    pub hnsw_ef_construction: usize,
    /// HNSW M parameter (number of connections per layer)
    pub hnsw_m: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            vector_dim: 384,
            hnsw_ef_construction: 200,
            hnsw_m: 16,
        }
    }
}
