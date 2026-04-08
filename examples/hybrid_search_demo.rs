// examples/hybrid_search_demo.rs
use bund_blobstore::common::GrokLogParser;
use bund_blobstore::common::log_ingestor::{LogIngestionConfig, LogIngestor};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use bund_blobstore::timeline::TelemetryValue;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     Hybrid Search Demo - Using Existing Functionality           ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    if let Err(e) = run_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
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

    // Create DataDistributionManager with RoundRobin strategy
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?,
    ));

    println!("✓ DataDistributionManager initialized\n");

    println!("📄 Step 2: Creating and storing chunked documents\n");

    // Create a large technical document and split into chunks
    let chunks = create_chunked_document();
    println!("Created {} document chunks", chunks.len());

    // Store chunks in DataDistributionManager
    for (i, chunk) in chunks.iter().enumerate() {
        let key = format!("doc_chunk_{:04}", i);
        let data = chunk.as_bytes();

        manager
            .write()
            .put(&key, data, None)
            .map_err(|e| format!("Failed to store chunk: {}", e))?;

        if i < 3 {
            println!("  Stored {}: {}...", key, &chunk[..chunk.len().min(80)]);
        }
    }

    println!(
        "\n✓ Stored {} chunks in DataDistributionManager",
        chunks.len()
    );

    // Display distribution stats
    let stats = manager.read().get_stats();
    println!("\n📊 Distribution Statistics:");
    println!("  - Total records: {}", stats.total_records);

    println!("\n🔍 Step 3: Full-Text Search Queries\n");

    // Perform FTS queries on the chunks
    let queries = vec!["vector database", "full text search", "hybrid search"];

    for query in queries {
        println!("【Query】 '{}'", query);
        let results = search_chunks(&chunks, query);
        println!("Found {} results:", results.len());
        for (i, (chunk, score)) in results.iter().enumerate().take(3) {
            println!(
                "  {}. Score: {:.3} - {}...",
                i + 1,
                score,
                &chunk[..chunk.len().min(100)]
            );
        }
        println!();
    }

    println!("\n🔍 Step 4: Vector Similarity Search\n");

    // Perform vector similarity search
    let vector_query = "How to optimize search performance?";
    println!("Vector Query: '{}'", vector_query);
    let query_embedding = generate_embedding(vector_query);

    let similar_chunks = find_similar_chunks(&chunks, &query_embedding, 3);
    println!("Top 3 semantically similar chunks:");
    for (i, (chunk, similarity)) in similar_chunks.iter().enumerate() {
        println!("  {}. Similarity: {:.3}", i + 1, similarity);
        println!("     {}...", &chunk[..chunk.len().min(100)]);
    }

    println!("\n🔀 Step 5: Hybrid Search (FTS + Vector)\n");

    // Hybrid search combining both methods
    let hybrid_query = "database optimization techniques";
    println!("Hybrid Query: '{}'", hybrid_query);

    let fts_results = search_chunks(&chunks, hybrid_query);
    let query_emb = generate_embedding(hybrid_query);
    let vector_results = find_similar_chunks(&chunks, &query_emb, 5);

    let hybrid_results = combine_results(&fts_results, &vector_results);
    println!("\nHybrid Search Results (FTS + Vector combined):");
    for (i, (chunk, score)) in hybrid_results.iter().enumerate().take(5) {
        println!("  {}. Hybrid Score: {:.3}", i + 1, score);
        println!("     {}...", &chunk[..chunk.len().min(100)]);
    }

    println!("\n✅ Demo completed successfully!");

    // Cleanup
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    Ok(())
}

fn create_chunked_document() -> Vec<String> {
    let mut chunks = Vec::new();

    // Create comprehensive technical content chunks
    let content = vec![
        r#"Vector databases are specialized database systems designed to store and query high-dimensional vector embeddings. These embeddings represent unstructured data such as text, images, audio, and video in a numerical format that captures semantic meaning. Unlike traditional databases that rely on exact matches, vector databases excel at similarity search."#,
        r#"Full-text search (FTS) focuses on exact keyword matching and lexical analysis. It uses inverted indexes to quickly find documents containing specific words or phrases. FTS excels at precise keyword queries but struggles with synonyms, misspellings, and semantic understanding."#,
        r#"Hybrid search combines both techniques to leverage their respective strengths. Common hybrid approaches include Score Fusion which combines relevance scores, Reciprocal Rank Fusion (RRF) which merges rankings, and Cascaded Search which uses one method to filter then the other to re-rank."#,
        r#"Document chunking is critical for Retrieval-Augmented Generation (RAG) systems. Fixed-size chunking splits text into equal-sized chunks. Semantic chunking uses embeddings to find natural breakpoints. Recursive chunking starts with larger chunks and recursively splits oversized ones."#,
        r#"Performance optimization for vector search includes pre-filtering metadata before search, post-filtering after search, quantization to reduce precision, and partitioning vectors based on metadata. Query optimization includes batch processing, caching, and approximate search."#,
        r#"RAG implementation best practices include document preprocessing and cleaning, intelligent chunking with overlap, embedding generation for each chunk, metadata enrichment, and multi-modal indexing combining text and vectors."#,
        r#"Approximate Nearest Neighbor (ANN) algorithms include HNSW (Hierarchical Navigable Small World), IVF (Inverted File Index), and PQ (Product Quantization). These algorithms trade off between search speed and accuracy."#,
        r#"Embedding models like BERT, Sentence-BERT, Universal Sentence Encoder (USE), and FastText capture semantic meaning. These models transform text into vector representations that preserve contextual relationships."#,
        r#"Real-world applications of hybrid search include e-commerce product discovery, enterprise document search, customer support ticket matching, and content recommendation systems."#,
        r#"TF-IDF (Term Frequency-Inverse Document Frequency) and BM25 are ranking functions used in full-text search. They evaluate how relevant a document is to a given search query based on term frequency and document frequency."#,
    ];

    for text in content {
        // Further split long chunks if needed
        if text.len() > 500 {
            let words: Vec<&str> = text.split_whitespace().collect();
            let mut current_chunk = String::new();
            for word in words {
                if current_chunk.len() + word.len() + 1 > 500 {
                    chunks.push(current_chunk.clone());
                    current_chunk.clear();
                }
                if !current_chunk.is_empty() {
                    current_chunk.push(' ');
                }
                current_chunk.push_str(word);
            }
            if !current_chunk.is_empty() {
                chunks.push(current_chunk);
            }
        } else {
            chunks.push(text.to_string());
        }
    }

    chunks
}

fn search_chunks(chunks: &[String], query: &str) -> Vec<(String, f32)> {
    let query_words: Vec<&str> = query.split_whitespace().collect();
    let mut results = Vec::new();

    for chunk in chunks {
        let mut score = 0.0;
        let chunk_lower = chunk.to_lowercase();

        for word in &query_words {
            let word_lower = word.to_lowercase();
            if chunk_lower.contains(&word_lower) {
                // Simple TF scoring
                let count = chunk_lower.matches(&word_lower).count() as f32;
                score += count / chunk.len() as f32;
            }
        }

        // Normalize by query length
        score /= query_words.len() as f32;

        if score > 0.0 {
            results.push((chunk.clone(), score));
        }
    }

    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results
}

fn generate_embedding(text: &str) -> Vec<f32> {
    // Simple but effective embedding using word hashing
    let mut embedding = vec![0.0f32; 64];
    let words: Vec<&str> = text.split_whitespace().collect();

    for word in words {
        let hash = simple_hash(word);
        let idx = (hash % 64) as usize;
        embedding[idx] += 1.0;
    }

    // Normalize
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

fn find_similar_chunks(
    chunks: &[String],
    query_embedding: &[f32],
    top_k: usize,
) -> Vec<(String, f32)> {
    let mut similarities: Vec<(String, f32)> = chunks
        .iter()
        .map(|chunk| {
            let chunk_embedding = generate_embedding(chunk);
            let similarity = cosine_similarity(query_embedding, &chunk_embedding);
            (chunk.clone(), similarity)
        })
        .collect();

    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    similarities.truncate(top_k);
    similarities
}

fn combine_results(
    fts_results: &[(String, f32)],
    vector_results: &[(String, f32)],
) -> Vec<(String, f32)> {
    use std::collections::HashMap;

    let mut scores: HashMap<String, f32> = HashMap::new();
    let k = 60.0;

    // Add FTS results with RRF scores
    for (rank, (chunk, _)) in fts_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + (rank + 1) as f32);
        *scores.entry(chunk.clone()).or_insert(0.0) += rrf_score;
    }

    // Add vector results with RRF scores
    for (rank, (chunk, _)) in vector_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + (rank + 1) as f32);
        *scores.entry(chunk.clone()).or_insert(0.0) += rrf_score;
    }

    // Sort by hybrid score
    let mut results: Vec<(String, f32)> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results
}
