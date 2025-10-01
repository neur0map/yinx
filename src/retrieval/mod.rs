//! Phase 7: Hybrid Retrieval & Reranking
//!
//! This module implements hybrid search combining semantic and keyword search,
//! with Reciprocal Rank Fusion and optional cross-encoder reranking.

mod deduplication;
mod fusion;
mod hybrid;
mod provenance;
mod reranker;

pub use deduplication::deduplicate_chunks;
pub use fusion::{reciprocal_rank_fusion, FusionConfig};
pub use hybrid::{HybridSearcher, SearchError};
pub use provenance::{ChunkMetadata, Provenance, ScoredChunk};
pub use reranker::{RerankError, Reranker};

use serde::{Deserialize, Serialize};

/// Search query with optional filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Query text
    pub text: String,

    /// Maximum number of results
    pub limit: usize,

    /// Optional session filter
    pub session_id: Option<String>,

    /// Optional tool filter
    pub tool_filter: Option<String>,

    /// Optional time range filter
    pub time_range: Option<(i64, i64)>,
}

impl SearchQuery {
    pub fn new(text: impl Into<String>, limit: usize) -> Self {
        Self {
            text: text.into(),
            limit,
            session_id: None,
            tool_filter: None,
            time_range: None,
        }
    }
}
