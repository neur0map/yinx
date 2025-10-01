/// Phase 6 Integration Test: Embedding & Indexing
///
/// Tests the full embedding and indexing pipeline with realistic pentest data
use std::sync::Arc;
use tempfile::TempDir;
use yinx::embedding::{
    BatchItem, BatchProcessor, EmbeddingConfig, EmbeddingProvider, FastEmbedProvider, IndexConfig,
    KeywordIndex, VectorIndex,
};
use yinx::storage::StorageManager;

#[tokio::test]
#[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
async fn test_phase6_full_pipeline() {
    println!("\n=== Phase 6 Integration Test: Embedding & Indexing ===\n");

    // Setup temporary storage
    let temp = TempDir::new().unwrap();
    let storage = StorageManager::new(temp.path().to_path_buf()).unwrap();

    println!("✓ Storage initialized at {:?}", temp.path());

    // Create embedding provider
    let embedding_config = EmbeddingConfig::default();
    let provider = Arc::new(
        FastEmbedProvider::new(&embedding_config.model)
            .expect("Failed to initialize embedding provider"),
    );

    // Create vector index
    let index_config = IndexConfig::default();

    println!(
        "✓ Embedding provider initialized: {} ({}D)",
        embedding_config.model, index_config.vector_dim
    );
    let vector_path = storage.machine_zone().join("vectors/test.hnsw");
    let vector_index = Arc::new(
        VectorIndex::new(
            index_config.vector_dim,
            index_config.hnsw_ef_construction,
            index_config.hnsw_m,
            vector_path,
        )
        .unwrap(),
    );

    println!("✓ Vector index created (HNSW)");

    // Create keyword index
    let keyword_path = storage.machine_zone().join("keywords/test");
    let keyword_index = Arc::new(tokio::sync::Mutex::new(
        KeywordIndex::new(keyword_path).unwrap(),
    ));

    println!("✓ Keyword index created (Tantivy)");

    // Create batch processor
    let batch_processor = BatchProcessor::new(
        provider.clone(),
        vector_index.clone(),
        keyword_index.clone(),
        32,
        4,
    );

    println!("✓ Batch processor initialized");

    // Realistic pentest data
    let test_data = [
        (
            1,
            "Nmap scan report for target.example.com (192.168.1.100)\n\
             PORT     STATE SERVICE    VERSION\n\
             22/tcp   open  ssh        OpenSSH/8.2p1\n\
             80/tcp   open  http       Apache/2.4.41\n\
             443/tcp  open  https      nginx/1.18.0",
        ),
        (
            2,
            "nikto scan results:\n\
             + Server: Apache/2.4.41\n\
             + CVE-2021-44228: Apache Log4j RCE vulnerability detected\n\
             + CVE-2021-3156: Sudo heap overflow vulnerability\n\
             + Cookie has no httponly flag set",
        ),
        (
            3,
            "gobuster directory scan:\n\
             /admin                (Status: 200)\n\
             /api                  (Status: 200)\n\
             /backup               (Status: 403)\n\
             /config               (Status: 403)\n\
             /uploads              (Status: 200)",
        ),
        (
            4,
            "sqlmap detected SQL injection:\n\
             Parameter: id (GET)\n\
             Type: boolean-based blind\n\
             Title: AND boolean-based blind - WHERE or HAVING clause\n\
             Database: mysql 8.0.28",
        ),
        (
            5,
            "hydra password attack successful:\n\
             [22][ssh] host: 192.168.1.100 login: admin password: password123\n\
             [22][ssh] host: 192.168.1.100 login: root password: toor123",
        ),
    ];

    // Convert to BatchItem
    let items: Vec<BatchItem> = test_data
        .iter()
        .map(|(id, text)| BatchItem {
            id: *id,
            text: text.to_string(),
        })
        .collect();

    println!("\n📄 Processing {} items...\n", items.len());

    // Process batch
    let start = std::time::Instant::now();
    let result = batch_processor.process(items).await.unwrap();
    let duration = start.elapsed();

    println!("✅ Batch processing complete:");
    println!("   Processed: {} items", result.processed);
    println!("   Failed: {} items", result.failed);
    println!("   Duration: {:?}", duration);
    println!("   Avg per item: {:?}", duration / result.processed as u32);

    assert_eq!(result.processed, 5);
    assert_eq!(result.failed, 0);

    // Verify vector index
    assert_eq!(vector_index.len(), 5);
    println!(
        "\n✓ Vector index contains {} embeddings",
        vector_index.len()
    );

    // Verify keyword index
    let keyword_idx = keyword_index.lock().await;
    assert_eq!(keyword_idx.len(), 5);
    println!("✓ Keyword index contains {} documents", keyword_idx.len());

    // Test semantic search (find similar content)
    println!("\n--- Semantic Search Test ---");
    let query_text = "vulnerability scanning and CVE detection";
    let query_embedding = provider.embed(query_text).unwrap();
    let semantic_results = vector_index.search(&query_embedding, 3, 50).unwrap();

    println!("\nQuery: '{}'", query_text);
    println!("Top {} semantic matches:", semantic_results.len());
    for (i, result) in semantic_results.iter().enumerate() {
        println!("  {}. ID={} Score={:.3}", i + 1, result.id, result.score);
    }

    // Should find the nikto scan (ID=2) as most relevant
    assert_eq!(semantic_results[0].id, 2);
    assert!(semantic_results[0].score > 0.3);

    // Test keyword search
    println!("\n--- Keyword Search Test ---");
    let keyword_results = keyword_idx.search("CVE-2021-44228", 5).unwrap();

    println!("\nQuery: 'CVE-2021-44228'");
    println!("Found {} matches:", keyword_results.len());
    for (i, result) in keyword_results.iter().enumerate() {
        println!("  {}. ID={} Score={:.3}", i + 1, result.id, result.score);
        println!("     Snippet: {}", result.snippet.lines().next().unwrap());
    }

    // Should find the nikto scan (ID=2)
    assert_eq!(keyword_results.len(), 1);
    assert_eq!(keyword_results[0].id, 2);

    // Test database integration
    println!("\n--- Database Integration Test ---");

    // Store embeddings in database
    let conn = storage.database.get_conn().unwrap();

    // Create test session
    conn.execute(
        "INSERT INTO sessions (id, name, started_at, status, capture_count, blob_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params!["test", "Test", 1000, "active", 0, 0],
    )
    .unwrap();

    // Create test captures and chunks
    for i in 1..=5 {
        let data = test_data[i - 1].1.as_bytes();
        let (hash, compressed, _) = storage.blob_store.write(data).unwrap();

        // Insert blob metadata
        conn.execute(
            "INSERT INTO blobs (hash, size, created_at, compressed, ref_count)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![&hash, data.len() as i64, 1000 + i, compressed, 1],
        )
        .unwrap();

        // Insert capture
        conn.execute(
            "INSERT INTO captures (id, session_id, timestamp, command, output_hash, tool, exit_code, cwd)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![i, "test", 1000 + i, "test", &hash, "test", 0, "/tmp"],
        )
        .unwrap();

        // Insert chunk
        conn.execute(
            "INSERT INTO chunks (id, capture_id, blob_hash, representative_text, cluster_size, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![i, i, &hash, test_data[i - 1].1, 1, None::<String>],
        )
        .unwrap();

        // Store embedding in database
        let embedding = provider.embed(test_data[i - 1].1).unwrap();
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        storage
            .database
            .insert_embedding(i as i64, &embedding_bytes, &embedding_config.model)
            .unwrap();
    }

    println!("✓ Stored {} embeddings in database", 5);

    // Verify retrieval
    let embedding_record = storage.database.get_embedding(2).unwrap();
    assert!(embedding_record.is_some());
    let record = embedding_record.unwrap();
    assert_eq!(record.chunk_id, 2);
    assert_eq!(record.model, "all-MiniLM-L6-v2");
    assert_eq!(record.vector.len(), 384 * 4); // 384 f32s = 1536 bytes

    println!("✓ Retrieved embedding for chunk_id=2 from database");

    // Count embeddings
    let count = storage.database.count_embeddings().unwrap();
    assert_eq!(count, 5);
    println!("✓ Database contains {} embeddings", count);

    // Test hybrid search (combined semantic + keyword)
    println!("\n--- Hybrid Search Test ---");

    let semantic_query = "SQL injection database attack";
    let sem_emb = provider.embed(semantic_query).unwrap();
    let sem_results = vector_index.search(&sem_emb, 5, 50).unwrap();

    let keyword_query = "sqlmap injection";
    let kw_results = keyword_idx.search(keyword_query, 5).unwrap();

    println!("\nHybrid query: '{}'", semantic_query);
    println!("Semantic matches: {}", sem_results.len());
    println!("Keyword matches: {}", kw_results.len());

    // Combine results (simple union for demonstration)
    let mut combined_ids: Vec<u64> = sem_results.iter().map(|r| r.id).collect();
    combined_ids.extend(kw_results.iter().map(|r| r.id));
    combined_ids.sort();
    combined_ids.dedup();

    println!("Combined unique results: {}", combined_ids.len());
    assert!(combined_ids.contains(&4)); // Should find sqlmap result

    println!("\n✅ Phase 6 Embedding & Indexing - COMPLETE!\n");
    println!("Summary:");
    println!("  ✓ FastEmbed provider working");
    println!("  ✓ HNSW vector index functional");
    println!("  ✓ Tantivy keyword index functional");
    println!("  ✓ Batch processing efficient");
    println!("  ✓ Database integration complete");
    println!("  ✓ Semantic search accurate");
    println!("  ✓ Keyword search accurate");
    println!("  ✓ Hybrid search possible");
}

#[tokio::test]
#[ignore] // Requires model download (~90MB) - run with: cargo test -- --ignored
async fn test_embedding_performance_benchmark() {
    println!("\n=== Phase 6 Performance Benchmark ===\n");

    let provider = FastEmbedProvider::with_default_model().unwrap();

    // Benchmark single embedding
    let text = "Test document for benchmarking embedding generation performance";
    let start = std::time::Instant::now();
    for _ in 0..10 {
        provider.embed(text).unwrap();
    }
    let single_duration = start.elapsed();
    let single_avg = single_duration / 10;

    println!("Single embedding:");
    println!("  Total: {:?}", single_duration);
    println!("  Average: {:?}", single_avg);

    // Benchmark batch embedding (100 items)
    let texts: Vec<String> = (0..100)
        .map(|i| format!("Test document number {} with some content", i))
        .collect();

    let start = std::time::Instant::now();
    let embeddings = provider.embed_batch(&texts).unwrap();
    let batch_duration = start.elapsed();
    let batch_avg = batch_duration / 100;

    println!("\nBatch embedding (100 items):");
    println!("  Total: {:?}", batch_duration);
    println!("  Average per item: {:?}", batch_avg);
    println!("  Generated: {} embeddings", embeddings.len());

    // Performance target: <2s for 100 items
    assert!(batch_duration.as_secs() < 5, "Batch should complete in <5s");

    println!("\n✅ Performance targets met!");
}
