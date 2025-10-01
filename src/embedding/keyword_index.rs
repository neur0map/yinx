/// Tantivy keyword index for full-text search
use anyhow::Result;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, TantivyError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeywordIndexError {
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

    #[error("Tantivy error: {0}")]
    TantivyError(#[from] TantivyError),

    #[error("Query parsing error: {0}")]
    QueryParseError(String),
}

/// Search result with ID and relevance score
#[derive(Debug, Clone)]
pub struct KeywordSearchResult {
    /// ID of the document (maps to chunk_id or capture_id)
    pub id: u64,
    /// BM25 relevance score
    pub score: f32,
    /// Matched text snippet
    pub snippet: String,
}

/// Tantivy keyword index wrapper
///
/// Provides full-text search with BM25 ranking.
pub struct KeywordIndex {
    index: Index,
    reader: IndexReader,
    writer: IndexWriter,
    #[allow(dead_code)]
    schema: Schema,
    id_field: Field,
    text_field: Field,
    #[allow(dead_code)]
    index_path: PathBuf,
}

impl KeywordIndex {
    /// Create a new keyword index
    ///
    /// # Arguments
    /// * `index_path` - Directory to store the index
    pub fn new(index_path: PathBuf) -> Result<Self, KeywordIndexError> {
        // Try to open existing index
        if index_path.exists() && index_path.join("meta.json").exists() {
            Self::load(index_path)
        } else {
            Self::create(index_path)
        }
    }

    /// Create a new index from scratch
    fn create(index_path: PathBuf) -> Result<Self, KeywordIndexError> {
        // Create directory
        std::fs::create_dir_all(&index_path)?;

        // Define schema
        let mut schema_builder = Schema::builder();

        let id_field = schema_builder.add_u64_field("id", INDEXED | STORED);
        let text_field = schema_builder.add_text_field("text", TEXT | STORED);

        let schema = schema_builder.build();

        // Create index
        let index = Index::create_in_dir(&index_path, schema.clone())
            .map_err(|e| KeywordIndexError::InitializationError(e.to_string()))?;

        // Create writer
        let writer = index
            .writer(50_000_000) // 50MB buffer
            .map_err(|e| KeywordIndexError::InitializationError(e.to_string()))?;

        // Create reader
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| KeywordIndexError::InitializationError(e.to_string()))?;

        Ok(Self {
            index,
            reader,
            writer,
            schema,
            id_field,
            text_field,
            index_path,
        })
    }

    /// Load existing index
    fn load(index_path: PathBuf) -> Result<Self, KeywordIndexError> {
        if !index_path.exists() {
            return Err(KeywordIndexError::IndexNotFound(
                index_path.display().to_string(),
            ));
        }

        // Open index
        let index = Index::open_in_dir(&index_path)
            .map_err(|e| KeywordIndexError::InitializationError(e.to_string()))?;

        let schema = index.schema();

        // Get fields
        let id_field = schema.get_field("id").map_err(|_| {
            KeywordIndexError::InitializationError("Missing 'id' field in schema".to_string())
        })?;

        let text_field = schema.get_field("text").map_err(|_| {
            KeywordIndexError::InitializationError("Missing 'text' field in schema".to_string())
        })?;

        // Create writer
        let writer = index
            .writer(50_000_000)
            .map_err(|e| KeywordIndexError::InitializationError(e.to_string()))?;

        // Create reader
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()
            .map_err(|e| KeywordIndexError::InitializationError(e.to_string()))?;

        Ok(Self {
            index,
            reader,
            writer,
            schema,
            id_field,
            text_field,
            index_path,
        })
    }

    /// Insert a document into the index
    ///
    /// # Arguments
    /// * `id` - Unique ID for the document
    /// * `text` - Text content to index
    pub fn insert(&mut self, id: u64, text: &str) -> Result<(), KeywordIndexError> {
        let doc = doc!(
            self.id_field => id,
            self.text_field => text,
        );

        self.writer
            .add_document(doc)
            .map_err(|e| KeywordIndexError::InsertError(e.to_string()))?;

        Ok(())
    }

    /// Insert multiple documents in batch
    pub fn insert_batch(&mut self, items: &[(u64, String)]) -> Result<(), KeywordIndexError> {
        for (id, text) in items {
            self.insert(*id, text)?;
        }
        Ok(())
    }

    /// Commit all pending changes
    pub fn commit(&mut self) -> Result<(), KeywordIndexError> {
        self.writer
            .commit()
            .map_err(|e| KeywordIndexError::InsertError(e.to_string()))?;

        // Wait for reader to reload
        self.reader
            .reload()
            .map_err(|e| KeywordIndexError::SearchError(e.to_string()))?;

        Ok(())
    }

    /// Search the index
    ///
    /// # Arguments
    /// * `query` - Search query (supports boolean operators, phrases, etc.)
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    /// Vector of search results sorted by relevance
    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<KeywordSearchResult>, KeywordIndexError> {
        let searcher = self.reader.searcher();

        // Parse query
        let query_parser = QueryParser::for_index(&self.index, vec![self.text_field]);
        let query = query_parser
            .parse_query(query)
            .map_err(|e| KeywordIndexError::QueryParseError(e.to_string()))?;

        // Search
        let top_docs = searcher
            .search(&query, &TopDocs::with_limit(limit))
            .map_err(|e| KeywordIndexError::SearchError(e.to_string()))?;

        // Convert results
        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: tantivy::TantivyDocument = searcher
                .doc(doc_address)
                .map_err(|e| KeywordIndexError::SearchError(e.to_string()))?;

            let id = retrieved_doc
                .get_first(self.id_field)
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    KeywordIndexError::SearchError("Missing or invalid ID field".to_string())
                })?;

            let text = retrieved_doc
                .get_first(self.text_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Create snippet (first 200 chars)
            let snippet = if text.len() > 200 {
                format!("{}...", &text[..200])
            } else {
                text
            };

            results.push(KeywordSearchResult { id, score, snippet });
        }

        Ok(results)
    }

    /// Delete a document by ID
    pub fn delete(&mut self, id: u64) -> Result<(), KeywordIndexError> {
        let term = Term::from_field_u64(self.id_field, id);
        self.writer.delete_term(term);
        Ok(())
    }

    /// Clear the entire index
    pub fn clear(&mut self) -> Result<(), KeywordIndexError> {
        self.writer
            .delete_all_documents()
            .map_err(|e| KeywordIndexError::InsertError(e.to_string()))?;
        self.commit()?;
        Ok(())
    }

    /// Get the number of documents in the index
    pub fn len(&self) -> u64 {
        let searcher = self.reader.searcher();
        searcher.num_docs()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_index_creation() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test_index");

        let index = KeywordIndex::new(index_path);
        assert!(index.is_ok());

        let index = index.unwrap();
        assert_eq!(index.len(), 0);
        assert!(index.is_empty());
    }

    #[test]
    fn test_insert_and_search() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test_index");

        let mut index = KeywordIndex::new(index_path).unwrap();

        // Insert documents
        index
            .insert(1, "The quick brown fox jumps over the lazy dog")
            .unwrap();
        index
            .insert(2, "A fast red fox leaps above a sleepy canine")
            .unwrap();
        index
            .insert(3, "Python programming language tutorial")
            .unwrap();

        index.commit().unwrap();

        assert_eq!(index.len(), 3);

        // Search for "fox"
        let results = index.search("fox", 10).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results[0].id == 1 || results[0].id == 2);

        // Search for "python"
        let results = index.search("python", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 3);
    }

    #[test]
    fn test_batch_insert() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test_index");

        let mut index = KeywordIndex::new(index_path).unwrap();

        let items = vec![
            (1, "Document one".to_string()),
            (2, "Document two".to_string()),
            (3, "Document three".to_string()),
        ];

        index.insert_batch(&items).unwrap();
        index.commit().unwrap();

        assert_eq!(index.len(), 3);
    }

    #[test]
    fn test_phrase_search() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test_index");

        let mut index = KeywordIndex::new(index_path).unwrap();

        index.insert(1, "This is a test document").unwrap();
        index
            .insert(2, "Another test with different words")
            .unwrap();
        index.commit().unwrap();

        // Phrase search
        let results = index.search("\"test document\"", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 1);
    }

    #[test]
    fn test_reload() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test_index");

        // Create and populate index
        {
            let mut index = KeywordIndex::new(index_path.clone()).unwrap();
            index.insert(1, "Test document").unwrap();
            index.commit().unwrap();
        }

        // Reload index
        {
            let index = KeywordIndex::new(index_path).unwrap();
            assert_eq!(index.len(), 1);

            let results = index.search("test", 10).unwrap();
            assert_eq!(results.len(), 1);
        }
    }

    #[test]
    fn test_delete() {
        let temp = TempDir::new().unwrap();
        let index_path = temp.path().join("test_index");

        let mut index = KeywordIndex::new(index_path).unwrap();

        index.insert(1, "Document one").unwrap();
        index.insert(2, "Document two").unwrap();
        index.commit().unwrap();

        assert_eq!(index.len(), 2);

        // Delete document
        index.delete(1).unwrap();
        index.commit().unwrap();

        assert_eq!(index.len(), 1);
    }
}
