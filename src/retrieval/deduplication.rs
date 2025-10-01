//! Result deduplication by chunk ID

use crate::retrieval::ScoredChunk;
use std::collections::HashSet;

/// Deduplicate chunks by chunk_id, keeping the highest-scored instance
///
/// # Arguments
/// * `chunks` - Scored chunks potentially with duplicates
///
/// # Returns
/// Deduplicated chunks, maintaining score order
pub fn deduplicate_chunks(chunks: Vec<ScoredChunk>) -> Vec<ScoredChunk> {
    let mut seen: HashSet<i64> = HashSet::new();

    chunks
        .into_iter()
        .filter(|chunk| seen.insert(chunk.chunk_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::{ChunkMetadata, Provenance};
    use chrono::Utc;

    #[test]
    fn test_deduplication() {
        let prov = Provenance {
            capture_id: 1,
            blob_hash: "abc123".to_string(),
            command: "test".to_string(),
            timestamp: Utc::now(),
            tool: "test".to_string(),
        };

        let meta = ChunkMetadata {
            cluster_size: 1,
            pattern: "test".to_string(),
            scores: serde_json::json!({}),
            entities: vec![],
        };

        let chunks = vec![
            ScoredChunk::new(1, "text1".to_string(), 0.9, meta.clone(), prov.clone()),
            ScoredChunk::new(2, "text2".to_string(), 0.8, meta.clone(), prov.clone()),
            ScoredChunk::new(1, "text1".to_string(), 0.7, meta.clone(), prov.clone()), // Duplicate
        ];

        let deduped = deduplicate_chunks(chunks);

        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].chunk_id, 1);
        assert_eq!(deduped[0].score, 0.9); // Keeps first (highest score)
    }
}
