// examples/hybrid_search_demo.rs
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     Complete Hybrid Search Demo - DataDistributionManager       ║");
    println!("║     Document Storage + Embeddings + FTS + Vector Search         ║");
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

    println!("📄 Step 2: Creating and chunking large document\n");

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

    println!("\n💾 Step 3: Storing documents and embeddings in DataDistributionManager\n");

    // Store chunks and compute embeddings - NO in-memory storage!
    let mut document_ids = Vec::new();

    for (i, chunk) in chunks.iter().enumerate() {
        let doc_id = format!("doc_{:04}", i);
        document_ids.push(doc_id.clone());

        // Compute embedding for the chunk
        let embedding = compute_embedding(chunk);

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

        // Store complete document (content + embedding + metadata) in DataDistributionManager
        let doc_data =
            serde_json::to_vec(&doc).map_err(|e| format!("Failed to serialize: {}", e))?;
        manager
            .write()
            .put(&doc_id, &doc_data, None)
            .map_err(|e| format!("Failed to store document: {}", e))?;

        // Also store embedding separately for quick vector search (optional optimization)
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
        "\n✓ Stored {} documents with embeddings in DataDistributionManager",
        chunks.len()
    );

    // Display storage statistics - using available fields
    let stats = manager.read().get_stats();
    println!("\n📊 Storage Statistics:");
    println!("  - Total records stored: {}", stats.total_records);
    println!("  - Documents: {}", chunks.len());
    println!("  - Embedding records: {}", chunks.len());
    println!("  - Shard distribution: {:?}", stats.shard_distribution);
    println!("  - Load balance score: {:.3}", stats.load_balance_score);
    println!();

    println!("🔎 Step 4: Full-Text Search (Retrieving from DataDistributionManager)\n");

    // Perform FTS queries - reading from DataDistributionManager
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

    println!("\n🔍 Step 5: Vector Similarity Search (Semantic from DataDistributionManager)\n");

    // Perform vector search queries - reading embeddings from DataDistributionManager
    let vector_queries = vec![
        "semantic understanding of text",
        "approximate nearest neighbor algorithms",
        "retrieval augmented generation techniques",
    ];

    for query in vector_queries {
        println!("【Vector Query】 '{}'", query);
        let query_embedding = compute_embedding(query);
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

    println!("\n🔀 Step 6: Hybrid Search (FTS + Vector from DataDistributionManager)\n");

    // Perform hybrid searches
    let hybrid_queries = vec![
        "database indexing and search optimization",
        "document retrieval ranking methods",
        "embedding generation techniques",
    ];

    for query in hybrid_queries {
        println!("【Hybrid Query】 '{}'", query);

        // Get FTS results from DataDistributionManager
        let fts_results = full_text_search(&manager, &document_ids, query)?;

        // Get vector results from DataDistributionManager
        let query_embedding = compute_embedding(query);
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

    println!("\n📈 Step 7: Verifying Data Persistence\n");

    // Demonstrate retrieval from DataDistributionManager - no in-memory cache!
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
                "  ✓ Embedding verified in separate storage ({} bytes)",
                emb_data.len()
            );
        }
    }

    println!("\n✅ Demo completed successfully!");
    println!("\n📊 Summary of DataDistributionManager Usage:");
    println!("  - All documents stored persistently on disk");
    println!("  - Embeddings computed and stored with documents");
    println!("  - FTS queries read documents from storage");
    println!("  - Vector search reads embeddings from storage");
    println!("  - No in-memory caching - everything from DataDistributionManager");
    println!("  - Data persists across queries");

    // Cleanup
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    Ok(())
}

fn create_large_document() -> String {
    let mut doc = String::new();

    doc.push_str(r#"
# Vector Databases: Architecture and Implementation

## Introduction to Vector Databases

Vector databases are specialized database systems designed to store and query high-dimensional vector embeddings.
These embeddings represent unstructured data such as text, images, audio, and video in a numerical format that captures
semantic meaning. Unlike traditional databases that rely on exact matches, vector databases excel at similarity search,
finding items that are conceptually similar to a query vector.

## ANN Algorithms

### HNSW (Hierarchical Navigable Small World)

HNSW builds a multi-layer graph structure where each layer is a subset of the previous layer. Search starts at the top layer
and navigates through the graph to find nearest neighbors. This provides excellent search quality with logarithmic complexity.

### IVF (Inverted File Index)

IVF partitions the vector space into clusters using k-means. During search, only vectors in the most relevant clusters are examined,
significantly reducing the number of distance calculations needed.

## Full-Text Search Fundamentals

Full-text search (FTS) focuses on exact keyword matching and lexical analysis. It uses inverted indexes to quickly find documents
containing specific words or phrases.

### TF-IDF Scoring

Term Frequency-Inverse Document Frequency (TF-IDF) evaluates how relevant a document is to a query.

### BM25

BM25 is an advanced ranking function that improves upon TF-IDF by introducing term frequency saturation and document length
normalization.

## Vector Search Techniques

### Cosine Similarity

Cosine similarity measures the cosine of the angle between two vectors, ranging from -1 to 1.

### Euclidean Distance

Euclidean distance measures the straight-line distance between two points in vector space.

## Hybrid Search Approaches

### Score Fusion

Combine relevance scores from both FTS and vector search using weighted averaging.

### Reciprocal Rank Fusion (RRF)

Merge rankings from multiple search methods: RRF_score = Σ 1/(k + rank)

### Cascaded Search

Use one method to filter candidates, then re-rank with the other.

## RAG Implementation

### Indexing Phase

1. Document preprocessing and cleaning
2. Intelligent chunking with overlap
3. Embedding generation for each chunk
4. Metadata enrichment

### Retrieval Phase

1. Query understanding and expansion
2. Hybrid search execution
3. Re-ranking for relevance

## Performance Optimization

### Indexing Optimization

- Pre-filtering: Apply metadata filters before vector search
- Post-filtering: Apply filters after vector search
- Quantization: Reduce vector precision to save memory

### Query Optimization

- Batch Queries: Process multiple queries together
- Caching: Cache frequent query results
- Approximate Search: Use ANN for speed
"#);

    doc
}

fn chunk_document(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut position = 0;
    let text_len = text.len();

    while position < text_len {
        let end = (position + chunk_size).min(text_len);
        let mut chunk = text[position..end].to_string();

        // Try to break at paragraph boundaries
        if end < text_len {
            if let Some(last_para) = chunk.rfind("\n\n") {
                if last_para > chunk.len() / 2 {
                    chunk = chunk[..=last_para].to_string();
                }
            }
            // Try to break at sentence boundaries
            else if let Some(last_period) = chunk.rfind('.') {
                if last_period > chunk.len() / 2 {
                    chunk = chunk[..=last_period + 1].to_string();
                }
            }
        }

        if !chunk.trim().is_empty() && chunk.len() > 50 {
            chunks.push(chunk);
        }

        position += chunk_size - overlap;
    }

    chunks
}

fn compute_embedding(text: &str) -> Vec<f32> {
    // Create a 128-dim embedding using TF-IDF style features
    let mut embedding = vec![0.0f32; 128];
    let words: Vec<&str> = text.split_whitespace().collect();

    // Count word frequencies
    let mut word_freq: HashMap<String, usize> = HashMap::new();
    for word in words {
        let word_lower = word.to_lowercase();
        *word_freq.entry(word_lower).or_insert(0) += 1;
    }

    // Generate embedding from word features
    for (word, freq) in word_freq.iter() {
        let hash = simple_hash(word);
        let idx = (hash % 128) as usize;
        let tf = 1.0 + (*freq as f32).log10();
        embedding[idx] += tf;
    }

    // Apply normalization
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut embedding {
            *x /= norm;
        }
    }

    embedding
}

fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 0;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

fn full_text_search(
    manager: &Arc<RwLock<DataDistributionManager>>,
    document_ids: &[String],
    query: &str,
) -> Result<Vec<SearchResult>, String> {
    let query_terms: Vec<&str> = query.split_whitespace().collect();
    let mut results = Vec::new();

    // Read each document from DataDistributionManager
    for doc_id in document_ids {
        let doc_data = manager
            .read()
            .get(doc_id)
            .map_err(|e| format!("Failed to read document {}: {}", doc_id, e))?;

        if let Some(data) = doc_data {
            let doc: StoredDocument = serde_json::from_slice(&data)
                .map_err(|e| format!("Failed to deserialize document {}: {}", doc_id, e))?;

            let doc_lower = doc.content.to_lowercase();
            let mut score = 0.0;

            for term in &query_terms {
                let term_lower = term.to_lowercase();
                let count = doc_lower.matches(&term_lower).count() as f32;
                if count > 0.0 {
                    // TF score with log normalization
                    let tf = 1.0 + count.log10();
                    // Simple IDF simulation
                    let idf = 1.0 / (1.0 + (count / doc.content.len() as f32));
                    score += tf * idf;
                }
            }

            // Normalize by query length
            if !query_terms.is_empty() {
                score /= query_terms.len() as f32;
            }

            if score > 0.0 {
                results.push(SearchResult {
                    document_id: doc.id.clone(),
                    content: doc.content,
                    fts_score: score,
                    vector_score: 0.0,
                    hybrid_score: 0.0,
                });
            }
        }
    }

    results.sort_by(|a, b| b.fts_score.partial_cmp(&a.fts_score).unwrap());
    results.truncate(10);
    Ok(results)
}

fn vector_search(
    manager: &Arc<RwLock<DataDistributionManager>>,
    document_ids: &[String],
    query_embedding: &[f32],
    top_k: usize,
) -> Result<Vec<SearchResult>, String> {
    let mut results = Vec::new();

    // Read each document and its embedding from DataDistributionManager
    for doc_id in document_ids {
        // Read the embedding from storage
        let embedding_key = format!("emb_{}", doc_id);
        let embedding_data = manager
            .read()
            .get(&embedding_key)
            .map_err(|e| format!("Failed to read embedding for {}: {}", doc_id, e))?;

        if let Some(emb_data) = embedding_data {
            // Convert bytes back to Vec<f32>
            let embedding: Vec<f32> = emb_data
                .chunks(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();

            let similarity = cosine_similarity(query_embedding, &embedding);

            // Also need the document content for display
            let doc_data = manager
                .read()
                .get(doc_id)
                .map_err(|e| format!("Failed to read document {}: {}", doc_id, e))?;

            if let Some(data) = doc_data {
                let doc: StoredDocument = serde_json::from_slice(&data)
                    .map_err(|e| format!("Failed to deserialize document {}: {}", doc_id, e))?;

                results.push(SearchResult {
                    document_id: doc_id.clone(),
                    content: doc.content,
                    fts_score: 0.0,
                    vector_score: similarity,
                    hybrid_score: 0.0,
                });
            }
        }
    }

    results.sort_by(|a, b| b.vector_score.partial_cmp(&a.vector_score).unwrap());
    results.truncate(top_k);
    Ok(results)
}

fn combine_results(
    fts_results: &[SearchResult],
    vector_results: &[SearchResult],
) -> Vec<SearchResult> {
    use std::collections::HashMap;

    let mut combined: HashMap<String, SearchResult> = HashMap::new();
    let k = 60.0;

    // Add FTS results with RRF scores
    for (rank, result) in fts_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + (rank + 1) as f32);
        if let Some(existing) = combined.get_mut(&result.document_id) {
            existing.hybrid_score += rrf_score;
            existing.fts_score = result.fts_score;
        } else {
            combined.insert(
                result.document_id.clone(),
                SearchResult {
                    document_id: result.document_id.clone(),
                    content: result.content.clone(),
                    fts_score: result.fts_score,
                    vector_score: 0.0,
                    hybrid_score: rrf_score,
                },
            );
        }
    }

    // Add vector results with RRF scores
    for (rank, result) in vector_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + (rank + 1) as f32);
        if let Some(existing) = combined.get_mut(&result.document_id) {
            existing.hybrid_score += rrf_score;
            existing.vector_score = result.vector_score;
        } else {
            combined.insert(
                result.document_id.clone(),
                SearchResult {
                    document_id: result.document_id.clone(),
                    content: result.content.clone(),
                    fts_score: 0.0,
                    vector_score: result.vector_score,
                    hybrid_score: rrf_score,
                },
            );
        }
    }

    let mut results: Vec<SearchResult> = combined.into_values().collect();
    results.sort_by(|a, b| b.hybrid_score.partial_cmp(&a.hybrid_score).unwrap());
    results
}
