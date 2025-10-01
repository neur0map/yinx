/// Embedding provider trait and FastEmbed implementation
use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EmbeddingError {
    #[error("Model initialization failed: {0}")]
    InitializationError(String),

    #[error("Embedding generation failed: {0}")]
    GenerationError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

/// Trait for embedding providers
///
/// Allows abstraction over different embedding backends (FastEmbed, Candle, etc.)
pub trait EmbeddingProvider: Send + Sync {
    /// Generate embedding for a single text
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;

    /// Generate embeddings for multiple texts (batched for efficiency)
    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError>;

    /// Get the embedding dimension
    fn dimension(&self) -> usize;

    /// Get the model name
    fn model_name(&self) -> &str;
}

/// FastEmbed provider for local embedding generation
///
/// Uses all-MiniLM-L6-v2 model (384 dimensions) by default.
/// Optimized for offline operation with no API calls.
pub struct FastEmbedProvider {
    model: Arc<TextEmbedding>,
    model_name: String,
    dimension: usize,
}

impl FastEmbedProvider {
    /// Create a new FastEmbed provider with the specified model
    ///
    /// **Important**: Models are downloaded on-demand to `~/.cache/huggingface/`
    /// on first use. The smallest model (all-MiniLM-L6-v2) is ~90MB.
    /// Larger models:
    /// - all-MiniLM-L6-v2: 90MB (384 dims) - recommended for most use cases
    /// - bge-small-en-v1.5: 130MB (384 dims) - better accuracy
    /// - bge-base-en-v1.5: 440MB (768 dims) - highest accuracy
    pub fn new(model_name: &str) -> Result<Self, EmbeddingError> {
        // Map model name to FastEmbed enum
        let embedding_model = match model_name {
            "all-MiniLM-L6-v2" | "all-minilm-l6-v2" => EmbeddingModel::AllMiniLML6V2,
            "bge-small-en-v1.5" => EmbeddingModel::BGESmallENV15,
            "bge-base-en-v1.5" => EmbeddingModel::BGEBaseENV15,
            _ => {
                return Err(EmbeddingError::InitializationError(format!(
                    "Unsupported model: {}. Supported: all-MiniLM-L6-v2, bge-small-en-v1.5, bge-base-en-v1.5",
                    model_name
                )));
            }
        };

        // Get dimension from model
        let dimension = match embedding_model {
            EmbeddingModel::AllMiniLML6V2 => 384,
            EmbeddingModel::BGESmallENV15 => 384,
            EmbeddingModel::BGEBaseENV15 => 768,
            _ => 384, // fallback
        };

        // Log model size info
        let model_size_mb = match embedding_model {
            EmbeddingModel::AllMiniLML6V2 => 90,
            EmbeddingModel::BGESmallENV15 => 130,
            EmbeddingModel::BGEBaseENV15 => 440,
            _ => 90,
        };

        tracing::info!(
            "Initializing embedding model: {} ({}D, ~{}MB download if not cached)",
            model_name,
            dimension,
            model_size_mb
        );

        // Initialize model - will download to ~/.cache/huggingface/ if not present
        let init_options = InitOptions::new(embedding_model).with_show_download_progress(true);

        let model = TextEmbedding::try_new(init_options)
            .map_err(|e| EmbeddingError::InitializationError(e.to_string()))?;

        Ok(Self {
            model: Arc::new(model),
            model_name: model_name.to_string(),
            dimension,
        })
    }

    /// Create provider with default model (all-MiniLM-L6-v2)
    pub fn with_default_model() -> Result<Self, EmbeddingError> {
        Self::new("all-MiniLM-L6-v2")
    }
}

impl EmbeddingProvider for FastEmbedProvider {
    fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::InvalidInput("Empty text".to_string()));
        }

        let embeddings = self
            .model
            .embed(vec![text.to_string()], None)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;

        if embeddings.is_empty() {
            return Err(EmbeddingError::GenerationError(
                "No embeddings generated".to_string(),
            ));
        }

        let embedding = embeddings[0].clone();

        // Verify dimension
        if embedding.len() != self.dimension {
            return Err(EmbeddingError::DimensionMismatch {
                expected: self.dimension,
                actual: embedding.len(),
            });
        }

        Ok(embedding)
    }

    fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        // Filter out empty texts
        let valid_texts: Vec<String> = texts.iter().filter(|t| !t.is_empty()).cloned().collect();

        if valid_texts.is_empty() {
            return Err(EmbeddingError::InvalidInput(
                "All texts are empty".to_string(),
            ));
        }

        let embeddings = self
            .model
            .embed(valid_texts, None)
            .map_err(|e| EmbeddingError::GenerationError(e.to_string()))?;

        // Verify all dimensions
        for embedding in &embeddings {
            if embedding.len() != self.dimension {
                return Err(EmbeddingError::DimensionMismatch {
                    expected: self.dimension,
                    actual: embedding.len(),
                });
            }
        }

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    fn test_provider_creation() {
        let provider = FastEmbedProvider::with_default_model();
        assert!(provider.is_ok());

        let provider = provider.unwrap();
        assert_eq!(provider.dimension(), 384);
        assert_eq!(provider.model_name(), "all-MiniLM-L6-v2");
    }

    #[test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    fn test_single_embedding() {
        let provider = FastEmbedProvider::with_default_model().unwrap();
        let text = "This is a test sentence for embedding.";

        let embedding = provider.embed(text);
        assert!(embedding.is_ok());

        let embedding = embedding.unwrap();
        assert_eq!(embedding.len(), 384);

        // Check that embedding is normalized (roughly unit length)
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.1);
    }

    #[test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    fn test_batch_embedding() {
        let provider = FastEmbedProvider::with_default_model().unwrap();
        let texts = vec![
            "First test sentence.".to_string(),
            "Second test sentence.".to_string(),
            "Third test sentence.".to_string(),
        ];

        let embeddings = provider.embed_batch(&texts);
        assert!(embeddings.is_ok());

        let embeddings = embeddings.unwrap();
        assert_eq!(embeddings.len(), 3);

        for embedding in embeddings {
            assert_eq!(embedding.len(), 384);
        }
    }

    #[test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    fn test_empty_text() {
        let provider = FastEmbedProvider::with_default_model().unwrap();
        let result = provider.embed("");
        assert!(result.is_err());
    }

    #[test]
    #[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
    fn test_semantic_similarity() {
        let provider = FastEmbedProvider::with_default_model().unwrap();

        let text1 = "The cat sits on the mat.";
        let text2 = "A feline rests on the rug.";
        let text3 = "Python programming language.";

        let emb1 = provider.embed(text1).unwrap();
        let emb2 = provider.embed(text2).unwrap();
        let emb3 = provider.embed(text3).unwrap();

        // Calculate cosine similarity
        let sim_1_2 = cosine_similarity(&emb1, &emb2);
        let sim_1_3 = cosine_similarity(&emb1, &emb3);

        // Similar sentences should have higher similarity
        assert!(sim_1_2 > sim_1_3);
        assert!(sim_1_2 > 0.5);
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (mag_a * mag_b)
    }
}
