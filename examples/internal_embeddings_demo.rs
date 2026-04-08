// examples/hybrid_search_demo.rs
use bund_blobstore::common::embeddings::EmbeddingModel;
use bund_blobstore::common::embeddings::{EmbeddingGenerator, cosine_similarity};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     Complete Hybrid Search Demo - DataDistributionManager       ║");
    println!("║     Document Storage + FastEmbed + FTS + Vector Search          ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    if let Err(e) = run_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StoredDocument {
    id: String,
    content: String,
    embedding: Vec<f32>,
    metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct SearchResult {
    document_id: String,
    content: String,
    fts_score: f32,
    vector_score: f32,
    hybrid_score: f32,
}

fn run_demo() -> Result<(), String> {
    // Setup data directory
    let data_dir = PathBuf::from("./hybrid_demo_data");
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir)
            .map_err(|e| format!("Failed to remove old dir: {}", e))?;
    }
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    println!("📚 Step 1: Initializing DataDistributionManager\n");

    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?,
    ));

    println!("✓ DataDistributionManager initialized with RoundRobin strategy\n");

    println!("📚 Step 2: Initializing FastEmbed Generator\n");

    // Initialize fastembed generator
    let embedder = EmbeddingGenerator::new(EmbeddingModel::AllMiniLML6V2)
        .map_err(|e| format!("Failed to initialize embedder: {}", e))?;
    println!(
        "✓ FastEmbed initialized with model: {} ({} dimensions)",
        embedder.model().name(),
        embedder.dimension()
    );
    println!("  - Model: {}", embedder.model().name());
    println!("  - Dimension: {}\n", embedder.dimension());

    println!("📄 Step 3: Creating and chunking large document\n");

    // Create a large technical document
    let document_text = create_large_document();
    println!("Original document size: {} characters", document_text.len());

    // Chunk the document
    let chunks = chunk_document(&document_text, 600, 100);
    println!("✓ Document split into {} chunks\n", chunks.len());

    // Display first few chunks
    println!("First 3 chunks preview:");
    for (i, chunk) in chunks.iter().enumerate().take(3) {
        println!("  Chunk {}: {}...", i + 1, &chunk[..chunk.len().min(100)]);
    }

    println!("\n💾 Step 4: Storing documents and embeddings in DataDistributionManager\n");

    // Store chunks and compute embeddings using fastembed
    let mut document_ids = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let doc_id = format!("doc_{:04}", i);
        document_ids.push(doc_id.clone());

        // Compute embedding using fastembed
        let embedding = embedder.embed(chunk)?;

        // Create metadata
        let mut metadata = HashMap::new();
        metadata.insert("chunk_index".to_string(), i.to_string());
        metadata.insert("size".to_string(), chunk.len().to_string());
        metadata.insert(
            "timestamp".to_string(),
            chrono::Utc::now().timestamp().to_string(),
        );

        // Create document object
        let doc = StoredDocument {
            id: doc_id.clone(),
            content: chunk.clone(),
            embedding: embedding.clone(),
            metadata: metadata.clone(),
        };

        // Store complete document in DataDistributionManager
        let doc_data =
            serde_json::to_vec(&doc).map_err(|e| format!("Failed to serialize: {}", e))?;
        manager
            .write()
            .put(&doc_id, &doc_data, None)
            .map_err(|e| format!("Failed to store document: {}", e))?;

        // Also store embedding separately for quick vector search
        let embedding_key = format!("emb_{}", doc_id);
        let embedding_data: Vec<u8> = embedding.iter().flat_map(|&f| f.to_le_bytes()).collect();
        manager
            .write()
            .put(&embedding_key, &embedding_data, None)
            .map_err(|e| format!("Failed to store embedding: {}", e))?;

        if i < 3 {
            println!(
                "  ✓ Stored document {} with {}dim embedding",
                doc_id,
                embedding.len()
            );
            println!("     Content: {}...", &chunk[..chunk.len().min(80)]);
        }
    }

    println!(
        "\n✓ Stored {} documents with FastEmbed embeddings in DataDistributionManager",
        chunks.len()
    );

    // Display storage statistics
    let stats = manager.read().get_stats();
    println!("\n📊 Storage Statistics:");
    println!("  - Total records stored: {}", stats.total_records);
    println!("  - Documents: {}", chunks.len());
    println!("  - Embedding records: {}", chunks.len());
    println!("  - Shard distribution: {:?}", stats.shard_distribution);
    println!("  - Load balance score: {:.3}", stats.load_balance_score);
    println!();

    println!("🔎 Step 5: Full-Text Search (Retrieving from DataDistributionManager)\n");

    // Perform FTS queries
    let fts_queries = vec!["vector database", "hybrid search", "chunking strategies"];

    for query in fts_queries {
        println!("【FTS Query】 '{}'", query);
        let results = full_text_search(&manager, &document_ids, query)?;
        println!("Found {} relevant documents:", results.len());
        for (i, result) in results.iter().enumerate().take(3) {
            println!(
                "  {}. Document: {} (Score: {:.3})",
                i + 1,
                result.document_id,
                result.fts_score
            );
            println!(
                "     Content: {}...",
                &result.content[..result.content.len().min(150)]
            );
        }
        println!();
    }

    println!("\n🔍 Step 6: Vector Similarity Search (FastEmbed from DataDistributionManager)\n");

    // Perform vector search queries using fastembed
    let vector_queries = vec![
        "semantic understanding of text",
        "approximate nearest neighbor algorithms",
        "retrieval augmented generation techniques",
    ];

    for query in vector_queries {
        println!("【Vector Query】 '{}'", query);
        let query_embedding = embedder.embed(query)?;
        let results = vector_search(&manager, &document_ids, &query_embedding, 3)?;
        println!("Top 3 semantically similar documents:");
        for (i, result) in results.iter().enumerate() {
            println!(
                "  {}. Document: {} (Similarity: {:.3})",
                i + 1,
                result.document_id,
                result.vector_score
            );
            println!(
                "     Content: {}...",
                &result.content[..result.content.len().min(150)]
            );
        }
        println!();
    }

    println!("\n🔀 Step 7: Hybrid Search (FTS + Vector from DataDistributionManager)\n");

    // Perform hybrid searches
    let hybrid_queries = vec![
        "database indexing and search optimization",
        "document retrieval ranking methods",
        "embedding generation techniques",
    ];

    for query in hybrid_queries {
        println!("【Hybrid Query】 '{}'", query);

        // Get FTS results
        let fts_results = full_text_search(&manager, &document_ids, query)?;

        // Get vector results using fastembed
        let query_embedding = embedder.embed(query)?;
        let vector_results = vector_search(&manager, &document_ids, &query_embedding, 10)?;

        // Combine using Reciprocal Rank Fusion
        let hybrid_results = combine_results(&fts_results, &vector_results);

        println!("\nHybrid Search Results (FTS + Vector combined):");
        for (i, result) in hybrid_results.iter().enumerate().take(5) {
            println!(
                "  {}. Document: {} (Hybrid Score: {:.3})",
                i + 1,
                result.document_id,
                result.hybrid_score
            );
            println!(
                "     FTS: {:.3} | Vector: {:.3}",
                result.fts_score, result.vector_score
            );
            println!(
                "     Content: {}...",
                &result.content[..result.content.len().min(150)]
            );
        }
        println!();
    }

    println!("\n📈 Step 8: Verifying Data Persistence\n");

    // Demonstrate retrieval from DataDistributionManager
    let test_doc_id = &document_ids[0];
    let retrieved_data = manager
        .read()
        .get(test_doc_id)
        .map_err(|e| format!("Failed to retrieve: {}", e))?;

    if let Some(data) = retrieved_data {
        let retrieved_doc: StoredDocument =
            serde_json::from_slice(&data).map_err(|e| format!("Failed to deserialize: {}", e))?;
        println!(
            "✓ Successfully retrieved document '{}' from DataDistributionManager",
            retrieved_doc.id
        );
        println!("  Content length: {} chars", retrieved_doc.content.len());
        println!("  Embedding dimension: {}", retrieved_doc.embedding.len());
        println!("  Metadata: {:?}", retrieved_doc.metadata);

        // Verify embedding was stored correctly
        let embedding_key = format!("emb_{}", test_doc_id);
        let embedding_data = manager
            .read()
            .get(&embedding_key)
            .map_err(|e| format!("Failed to retrieve embedding: {}", e))?;
        if let Some(emb_data) = embedding_data {
            println!(
                "  ✓ FastEmbed embedding verified in separate storage ({} bytes)",
                emb_data.len()
            );
        }
    }

    println!("\n✅ Demo completed successfully!");
    println!("\n📊 Summary of DataDistributionManager + FastEmbed:");
    println!("  - All documents stored persistently on disk");
    println!(
        "  - FastEmbed embeddings ({} dims) computed and stored",
        embedder.dimension()
    );
    println!("  - FTS queries read documents from storage");
    println!("  - Vector search reads FastEmbed embeddings from storage");
    println!("  - No in-memory caching - everything from DataDistributionManager");
    println!("  - Data persists across queries");

    // Cleanup
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    Ok(())
}

// ... rest of the helper functions (create_large_document, chunk_document,
// full_text_search, vector_search, combine_results) remain the same as before ...
