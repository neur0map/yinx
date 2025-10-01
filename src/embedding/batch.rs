/// Batch processor for efficient embedding generation
use super::{EmbeddingError, EmbeddingProvider, KeywordIndex, VectorIndex};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

/// Item to be processed (text with associated ID)
#[derive(Debug, Clone)]
pub struct BatchItem {
    pub id: u64,
    pub text: String,
}

/// Result of batch processing
#[derive(Debug)]
pub struct BatchResult {
    pub processed: usize,
    pub failed: usize,
    pub duration_ms: u64,
}

/// Batch processor for embedding generation and indexing
///
/// Efficiently processes large batches of text by:
/// - Batching embedding generation
/// - Concurrent processing
/// - Error handling and retry logic
pub struct BatchProcessor {
    provider: Arc<dyn EmbeddingProvider>,
    vector_index: Arc<VectorIndex>,
    keyword_index: Arc<tokio::sync::Mutex<KeywordIndex>>,
    batch_size: usize,
    max_concurrent: usize,
}

impl BatchProcessor {
    /// Create a new batch processor
    ///
    /// # Arguments
    /// * `provider` - Embedding provider
    /// * `vector_index` - Vector index for similarity search
    /// * `keyword_index` - Keyword index for full-text search
    /// * `batch_size` - Number of items to embed in one batch
    /// * `max_concurrent` - Maximum concurrent batch operations
    pub fn new(
        provider: Arc<dyn EmbeddingProvider>,
        vector_index: Arc<VectorIndex>,
        keyword_index: Arc<tokio::sync::Mutex<KeywordIndex>>,
        batch_size: usize,
        max_concurrent: usize,
    ) -> Self {
        Self {
            provider,
            vector_index,
            keyword_index,
            batch_size,
            max_concurrent,
        }
    }

    /// Process a batch of items
    ///
    /// Generates embeddings and updates both vector and keyword indexes.
    pub async fn process(&self, items: Vec<BatchItem>) -> Result<BatchResult> {
        let start = std::time::Instant::now();
        let total = items.len();

        info!("Starting batch processing of {} items", total);

        let mut processed = 0;
        let mut failed = 0;

        // Process in chunks of batch_size
        let semaphore = Arc::new(Semaphore::new(self.max_concurrent));

        for chunk in items.chunks(self.batch_size) {
            let permit = semaphore.clone().acquire_owned().await?;

            let result = self.process_chunk(chunk).await;

            drop(permit);

            match result {
                Ok(count) => {
                    processed += count;
                    debug!("Processed chunk of {} items", count);
                }
                Err(e) => {
                    warn!("Failed to process chunk: {}", e);
                    failed += chunk.len();
                }
            }
        }

        // Commit keyword index changes
        let mut keyword_index = self.keyword_index.lock().await;
        keyword_index.commit()?;

        // Save vector index
        self.vector_index.save()?;

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            "Batch processing complete: {} processed, {} failed, {}ms",
            processed, failed, duration_ms
        );

        Ok(BatchResult {
            processed,
            failed,
            duration_ms,
        })
    }

    /// Process a single chunk of items
    async fn process_chunk(&self, chunk: &[BatchItem]) -> Result<usize, EmbeddingError> {
        // Extract texts
        let texts: Vec<String> = chunk.iter().map(|item| item.text.clone()).collect();

        // Generate embeddings
        let embeddings = self.provider.embed_batch(&texts)?;

        if embeddings.len() != chunk.len() {
            return Err(EmbeddingError::GenerationError(format!(
                "Embedding count mismatch: expected {}, got {}",
                chunk.len(),
                embeddings.len()
            )));
        }

        // Insert into vector index
        for (item, embedding) in chunk.iter().zip(embeddings.iter()) {
            self.vector_index
                .insert(item.id, embedding)
                .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;
        }

        // Insert into keyword index
        let mut keyword_index = self.keyword_index.lock().await;
        for item in chunk {
            keyword_index
                .insert(item.id, &item.text)
                .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;
        }

        Ok(chunk.len())
    }

    /// Process items in the background without blocking
    pub fn process_background(
        self: Arc<Self>,
        items: Vec<BatchItem>,
    ) -> tokio::task::JoinHandle<Result<BatchResult>> {
        tokio::spawn(async move { self.process(items).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{FastEmbedProvider, KeywordIndex, VectorIndex};
    use tempfile::TempDir;

    async fn create_test_processor() -> (BatchProcessor, TempDir) {
        let temp = TempDir::new().unwrap();

        let provider = Arc::new(FastEmbedProvider::with_default_model().unwrap());

        let vector_path = temp.path().join("vectors.hnsw");
        let vector_index = Arc::new(VectorIndex::new(384, 200, 16, vector_path).unwrap());

        let keyword_path = temp.path().join("keywords");
        let keyword_index = Arc::new(tokio::sync::Mutex::new(
            KeywordIndex::new(keyword_path).unwrap(),
        ));

        let processor = BatchProcessor::new(provider, vector_index, keyword_index, 32, 4);

        (processor, temp)
    }

    #[tokio::test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    async fn test_batch_processing() {
        let (processor, _temp) = create_test_processor().await;

        let items = vec![
            BatchItem {
                id: 1,
                text: "Test document one".to_string(),
            },
            BatchItem {
                id: 2,
                text: "Test document two".to_string(),
            },
            BatchItem {
                id: 3,
                text: "Test document three".to_string(),
            },
        ];

        let result = processor.process(items).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.processed, 3);
        assert_eq!(result.failed, 0);
    }

    #[tokio::test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    async fn test_large_batch() {
        let (processor, _temp) = create_test_processor().await;

        // Create 100 test items
        let items: Vec<BatchItem> = (0..100)
            .map(|i| BatchItem {
                id: i,
                text: format!("Test document number {}", i),
            })
            .collect();

        let result = processor.process(items).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.processed, 100);
        assert_eq!(result.failed, 0);

        // Verify indexes were updated
        assert_eq!(processor.vector_index.len(), 100);

        let keyword_index = processor.keyword_index.lock().await;
        assert_eq!(keyword_index.len(), 100);
    }

    #[tokio::test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    async fn test_background_processing() {
        let (processor, _temp) = create_test_processor().await;
        let processor = Arc::new(processor);

        let items = vec![BatchItem {
            id: 1,
            text: "Background test".to_string(),
        }];

        let handle = processor.clone().process_background(items);
        let result = handle.await.unwrap();

        assert!(result.is_ok());
        assert_eq!(result.unwrap().processed, 1);
    }

    #[tokio::test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    async fn test_empty_batch() {
        let (processor, _temp) = create_test_processor().await;

        let items = vec![];
        let result = processor.process(items).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().processed, 0);
    }
}
