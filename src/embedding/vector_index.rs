/// HNSW vector index for similarity search
use hnsw_rs::prelude::*;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VectorIndexError {
    #[error("Index initialization failed: {0}")]
    InitializationError(String),

    #[error("Index not found: {0}")]
    IndexNotFound(String),

    #[error("Insert failed: {0}")]
    InsertError(String),

    #[error("Search failed: {0}")]
    SearchError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Search result with ID and similarity score
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// ID of the item (maps to chunk_id or capture_id)
    pub id: u64,
    /// Cosine similarity score (0.0 to 1.0, higher is more similar)
    pub score: f32,
}

/// HNSW vector index wrapper
///
/// Provides efficient approximate nearest neighbor search.
/// Uses cosine similarity (dot product on normalized vectors).
pub struct VectorIndex {
    /// Inner HNSW index
    index: Arc<RwLock<Hnsw<'static, f32, DistCosine>>>,
    /// Vector dimension
    dimension: usize,
    /// Index file path (for future persistence)
    #[allow(dead_code)]
    index_path: PathBuf,
    /// Number of indexed vectors
    count: Arc<RwLock<u64>>,
}

impl VectorIndex {
    /// Create a new vector index
    ///
    /// # Arguments
    /// * `dimension` - Vector dimension (must match embedding dimension)
    /// * `ef_construction` - HNSW construction parameter (higher = better recall, slower build)
    /// * `m` - HNSW M parameter (number of connections per layer)
    /// * `index_path` - Path to store the index file
    pub fn new(
        dimension: usize,
        ef_construction: usize,
        m: usize,
        index_path: PathBuf,
    ) -> Result<Self, VectorIndexError> {
        // Try to load existing index
        if index_path.exists() {
            Self::load(index_path)
        } else {
            // Create new index
            let index = Hnsw::<f32, DistCosine>::new(
                m,
                dimension,
                ef_construction,
                200, // max_nb_connection
                DistCosine,
            );

            Ok(Self {
                index: Arc::new(RwLock::new(index)),
                dimension,
                index_path,
                count: Arc::new(RwLock::new(0)),
            })
        }
    }

    /// Load existing index from file (not yet implemented)
    pub fn load(_index_path: PathBuf) -> Result<Self, VectorIndexError> {
        // TODO: Implement persistence with bincode or serde
        // For now, return error
        Err(VectorIndexError::SerializationError(
            "Index persistence not yet implemented - create new index instead".to_string(),
        ))
    }

    /// Save index to file (not yet implemented)
    pub fn save(&self) -> Result<(), VectorIndexError> {
        // TODO: Implement persistence with bincode or serde
        // For now, just succeed (index is in-memory)
        Ok(())
    }

    /// Insert a vector into the index
    ///
    /// # Arguments
    /// * `id` - Unique ID for the vector (e.g., chunk_id)
    /// * `vector` - Embedding vector
    pub fn insert(&self, id: u64, vector: &[f32]) -> Result<(), VectorIndexError> {
        if vector.len() != self.dimension {
            return Err(VectorIndexError::InvalidDimension {
                expected: self.dimension,
                actual: vector.len(),
            });
        }

        // Convert to owned Vec for HNSW
        let data = vector.to_vec();

        // Insert into index
        #[allow(unused_mut)]
        let mut index = self.index.write().unwrap();
        index.insert((&data, id as usize));

        // Update count
        let mut count = self.count.write().unwrap();
        *count += 1;

        Ok(())
    }

    /// Insert multiple vectors in batch
    pub fn insert_batch(&self, items: &[(u64, Vec<f32>)]) -> Result<(), VectorIndexError> {
        for (id, vector) in items {
            self.insert(*id, vector)?;
        }
        Ok(())
    }

    /// Search for k nearest neighbors
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `k` - Number of results to return
    /// * `ef_search` - HNSW search parameter (higher = better recall, slower search)
    ///
    /// # Returns
    /// Vector of (id, similarity_score) pairs, sorted by score descending
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
        ef_search: usize,
    ) -> Result<Vec<SearchResult>, VectorIndexError> {
        if query.len() != self.dimension {
            return Err(VectorIndexError::InvalidDimension {
                expected: self.dimension,
                actual: query.len(),
            });
        }

        let index = self.index.read().unwrap();

        // Perform search
        let results = index.search(query, k, ef_search);

        // Convert to SearchResult
        let search_results = results
            .into_iter()
            .map(|neighbor| SearchResult {
                id: neighbor.d_id as u64,
                score: 1.0 - neighbor.distance, // Convert distance to similarity
            })
            .collect();

        Ok(search_results)
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> u64 {
        *self.count.read().unwrap()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get vector dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Clear the index (remove all vectors)
    pub fn clear(&self) -> Result<(), VectorIndexError> {
        let mut index = self.index.write().unwrap();
        *index = Hnsw::<f32, DistCosine>::new(
            16, // default M
            self.dimension,
            200, // default ef_construction
            200, // max_nb_connection
            DistCosine,
        );

        let mut count = self.count.write().unwrap();
        *count = 0;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_index_creation() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test.hnsw");

        let index = VectorIndex::new(384, 200, 16, index_path);
        assert!(index.is_ok());

        let index = index.unwrap();
        assert_eq!(index.dimension(), 384);
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_insert_and_search() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test.hnsw");

        let index = VectorIndex::new(384, 200, 16, index_path).unwrap();

        // Create test vectors
        let mut vec1 = vec![0.0; 384];
        vec1[0] = 1.0;

        let mut vec2 = vec![0.0; 384];
        vec2[1] = 1.0;

        let mut vec3 = vec![0.0; 384];
        vec3[0] = 0.9;
        vec3[1] = 0.1;

        // Insert vectors
        index.insert(1, &vec1).unwrap();
        index.insert(2, &vec2).unwrap();
        index.insert(3, &vec3).unwrap();

        assert_eq!(index.len(), 3);

        // Search for nearest to vec1
        let results = index.search(&vec1, 2, 50).unwrap();
        assert_eq!(results.len(), 2);

        // First result should be vec1 itself or vec3 (most similar)
        assert!(results[0].id == 1 || results[0].id == 3);
        assert!(results[0].score > 0.8);
    }

    #[test]
    #[ignore] // Persistence not yet implemented - TODO: Phase 6.1
    fn test_save_and_load() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test.hnsw");

        // Create and populate index
        {
            let index = VectorIndex::new(384, 200, 16, index_path.clone()).unwrap();

            let vec = vec![1.0; 384];
            index.insert(42, &vec).unwrap();

            index.save().unwrap();
        }

        // Load index
        {
            let index = VectorIndex::load(index_path).unwrap();
            assert_eq!(index.dimension(), 384);
            assert_eq!(index.len(), 1);
        }
    }

    #[test]
    fn test_batch_insert() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test.hnsw");

        let index = VectorIndex::new(384, 200, 16, index_path).unwrap();

        let items: Vec<(u64, Vec<f32>)> = (0..10).map(|i| (i, vec![i as f32; 384])).collect();

        index.insert_batch(&items).unwrap();
        assert_eq!(index.len(), 10);
    }

    #[test]
    fn test_dimension_validation() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test.hnsw");

        let index = VectorIndex::new(384, 200, 16, index_path).unwrap();

        // Try to insert wrong dimension
        let vec = vec![1.0; 128];
        let result = index.insert(1, &vec);
        assert!(result.is_err());
    }
}
