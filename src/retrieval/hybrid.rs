//! Hybrid search combining semantic and keyword search

use crate::config::RetrievalConfig;
use crate::embedding::{EmbeddingProvider, KeywordIndex, VectorIndex};
use crate::retrieval::{
    deduplicate_chunks, reciprocal_rank_fusion, ChunkMetadata, FusionConfig, Provenance, Reranker,
    ScoredChunk, SearchQuery,
};
use crate::storage::Database;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("Embedding generation failed: {0}")]
    EmbeddingError(String),

    #[error("Vector search failed: {0}")]
    VectorSearchError(String),

    #[error("Keyword search failed: {0}")]
    KeywordSearchError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Reranking failed: {0}")]
    RerankingError(String),

    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}

/// Hybrid searcher combining semantic and keyword search
pub struct HybridSearcher {
    embedding_provider: Arc<dyn EmbeddingProvider>,
    vector_index: Arc<RwLock<VectorIndex>>,
    keyword_index: Arc<RwLock<KeywordIndex>>,
    database: Arc<Database>,
    reranker: Option<Arc<Reranker>>,
    config: RetrievalConfig,
}

impl HybridSearcher {
    /// Create a new hybrid searcher
    pub fn new(
        embedding_provider: Arc<dyn EmbeddingProvider>,
        vector_index: Arc<RwLock<VectorIndex>>,
        keyword_index: Arc<RwLock<KeywordIndex>>,
        database: Arc<Database>,
        config: RetrievalConfig,
    ) -> Result<Self, SearchError> {
        // Initialize reranker if enabled
        let reranker = if config.enable_reranking {
            let r = Reranker::new(&config.reranker_model)
                .map_err(|e| SearchError::RerankingError(e.to_string()))?;
            Some(Arc::new(r))
        } else {
            None
        };

        Ok(Self {
            embedding_provider,
            vector_index,
            keyword_index,
            database,
            reranker,
            config,
        })
    }

    /// Perform hybrid search
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<ScoredChunk>, SearchError> {
        if query.text.is_empty() {
            return Err(SearchError::InvalidQuery(
                "Query text cannot be empty".to_string(),
            ));
        }

        let search_limit = query.limit * self.config.search_multiplier;

        // Step 1: Parallel semantic + keyword search
        let (semantic_results, keyword_results) = tokio::join!(
            self.semantic_search(&query.text, search_limit),
            self.keyword_search(&query.text, search_limit)
        );

        let semantic_results = semantic_results?;
        let keyword_results = keyword_results?;

        // Step 2: Reciprocal Rank Fusion
        let fusion_config = FusionConfig::new(
            self.config.rrf_k,
            self.config.semantic_weight,
            self.config.keyword_weight,
        )
        .map_err(|e| SearchError::InvalidQuery(e.to_string()))?;

        let fused_results =
            reciprocal_rank_fusion(semantic_results, keyword_results, &fusion_config);

        // Step 3: Hydrate chunks from database
        let mut candidates = self.hydrate_chunks(fused_results).await?;

        // Step 4: Apply filters if specified
        if let Some(session_id) = &query.session_id {
            candidates.retain(|c| c.provenance.capture_id.to_string() == *session_id);
        }

        if let Some(tool) = &query.tool_filter {
            candidates.retain(|c| c.provenance.tool == *tool);
        }

        // Step 5: Apply similarity threshold
        if self.config.min_similarity_threshold > 0.0 {
            candidates.retain(|c| c.score >= self.config.min_similarity_threshold);
        }

        // Step 6: Rerank if enabled
        let results = if self.reranker.is_some() && candidates.len() > 1 {
            self.rerank_chunks(&query.text, candidates, query.limit)
                .await?
        } else {
            // Just truncate to limit
            candidates.truncate(query.limit);
            candidates
        };

        // Step 7: Deduplicate by chunk_id
        let final_results = deduplicate_chunks(results);

        Ok(final_results)
    }

    /// Semantic search using vector index
    async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(i64, f32)>, SearchError> {
        // Generate query embedding
        let query_embedding = self
            .embedding_provider
            .embed(query)
            .map_err(|e| SearchError::EmbeddingError(e.to_string()))?;

        // Search vector index
        let vector_index = self.vector_index.read().await;
        let results = vector_index
            .search(&query_embedding, limit, self.config.hnsw_ef_search)
            .map_err(|e| SearchError::VectorSearchError(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|r| (r.id as i64, r.score))
            .collect())
    }

    /// Keyword search using tantivy index
    async fn keyword_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(i64, f32)>, SearchError> {
        let keyword_index = self.keyword_index.read().await;
        let results = keyword_index
            .search(query, limit)
            .map_err(|e| SearchError::KeywordSearchError(e.to_string()))?;

        Ok(results
            .into_iter()
            .map(|r| (r.id as i64, r.score))
            .collect())
    }

    /// Hydrate chunks from database with full metadata and provenance
    async fn hydrate_chunks(
        &self,
        chunk_ids: Vec<(i64, f32)>,
    ) -> Result<Vec<ScoredChunk>, SearchError> {
        if chunk_ids.is_empty() {
            return Ok(Vec::new());
        }

        // Extract IDs and scores
        let ids: Vec<i64> = chunk_ids.iter().map(|(id, _)| *id).collect();
        let score_map: std::collections::HashMap<i64, f32> = chunk_ids.into_iter().collect();

        // Fetch chunks from database
        let chunk_records = self
            .database
            .get_chunks(&ids)
            .map_err(|e| SearchError::DatabaseError(format!("Failed to get chunks: {}", e)))?;

        // Hydrate each chunk
        let mut scored_chunks = Vec::new();
        for chunk_record in chunk_records {
            let score = score_map.get(&chunk_record.id).copied().unwrap_or(0.0);

            // Fetch capture for provenance
            let capture = self
                .database
                .get_capture(chunk_record.capture_id)
                .map_err(|e| SearchError::DatabaseError(format!("Failed to get capture: {}", e)))?
                .ok_or_else(|| {
                    SearchError::DatabaseError(format!(
                        "Capture {} not found for chunk {}",
                        chunk_record.capture_id, chunk_record.id
                    ))
                })?;

            // Parse metadata
            let metadata: ChunkMetadata = if let Some(metadata_json) = &chunk_record.metadata {
                serde_json::from_str(metadata_json).unwrap_or_else(|_| ChunkMetadata {
                    cluster_size: chunk_record.cluster_size as usize,
                    pattern: String::new(),
                    scores: serde_json::json!({}),
                    entities: vec![],
                })
            } else {
                ChunkMetadata {
                    cluster_size: chunk_record.cluster_size as usize,
                    pattern: String::new(),
                    scores: serde_json::json!({}),
                    entities: vec![],
                }
            };

            // Build provenance
            let provenance = Provenance {
                capture_id: capture.id,
                blob_hash: capture.output_hash.clone(),
                command: capture.command.unwrap_or_else(|| String::from("(unknown)")),
                timestamp: chrono::DateTime::from_timestamp(capture.timestamp, 0)
                    .unwrap_or_else(chrono::Utc::now),
                tool: capture.tool.unwrap_or_else(|| String::from("unknown")),
            };

            // Create scored chunk
            scored_chunks.push(ScoredChunk::new(
                chunk_record.id,
                chunk_record.representative_text,
                score,
                metadata,
                provenance,
            ));
        }

        Ok(scored_chunks)
    }

    /// Rerank chunks using cross-encoder
    async fn rerank_chunks(
        &self,
        query: &str,
        mut candidates: Vec<ScoredChunk>,
        limit: usize,
    ) -> Result<Vec<ScoredChunk>, SearchError> {
        let reranker = self
            .reranker
            .as_ref()
            .ok_or_else(|| SearchError::RerankingError("Reranker not initialized".to_string()))?;

        // Limit candidates for reranking
        let max_rerank = self.config.rerank_candidates_limit.min(candidates.len());
        candidates.truncate(max_rerank);

        // Extract texts
        let texts: Vec<String> = candidates.iter().map(|c| c.text.clone()).collect();

        // Rerank
        let reranked_indices = reranker
            .rerank(query, &texts, limit)
            .map_err(|e| SearchError::RerankingError(e.to_string()))?;

        // Reorder chunks and update scores
        let reranked_chunks: Vec<ScoredChunk> = reranked_indices
            .into_iter()
            .map(|(idx, new_score)| {
                let mut chunk = candidates[idx].clone();
                chunk.score = new_score;
                chunk
            })
            .collect();

        Ok(reranked_chunks)
    }
}

#[cfg(test)]
mod tests {

    // TODO: Add integration tests
}
