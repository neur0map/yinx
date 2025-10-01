//! Provenance tracking and scored chunk structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Provenance information tracking the source of a chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provenance {
    /// Capture ID from database
    pub capture_id: i64,

    /// Content hash (BLAKE3) of the original blob
    pub blob_hash: String,

    /// Command that generated this output
    pub command: String,

    /// Timestamp of the capture
    pub timestamp: DateTime<Utc>,

    /// Tool that generated the output
    pub tool: String,
}

/// Metadata about a chunk (from filtering pipeline)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    /// Cluster size (number of similar lines represented)
    pub cluster_size: usize,

    /// Pattern used for clustering
    pub pattern: String,

    /// Statistical scores from filtering
    pub scores: Value,

    /// Extracted entities from this chunk
    pub entities: Vec<String>,
}

/// A chunk with relevance score and full provenance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredChunk {
    /// Chunk ID from database
    pub chunk_id: i64,

    /// Representative text for this chunk
    pub text: String,

    /// Relevance score (0.0 to 1.0, higher is better)
    pub score: f32,

    /// Chunk metadata
    pub metadata: ChunkMetadata,

    /// Provenance information
    pub provenance: Provenance,
}

impl ScoredChunk {
    /// Create a new scored chunk
    pub fn new(
        chunk_id: i64,
        text: String,
        score: f32,
        metadata: ChunkMetadata,
        provenance: Provenance,
    ) -> Self {
        Self {
            chunk_id,
            text,
            score,
            metadata,
            provenance,
        }
    }

    /// Get a short preview of the text (first N characters)
    pub fn preview(&self, max_chars: usize) -> String {
        if self.text.len() <= max_chars {
            self.text.clone()
        } else {
            format!("{}...", &self.text[..max_chars])
        }
    }
}
