```markdown
# Text Documents with Embeddings - Complete Guide

## Overview

This guide provides comprehensive instructions for storing large text documents in the DataDistributionManager with embeddings and performing hybrid search (full-text + vector similarity). The approach combines traditional keyword search with semantic understanding for optimal retrieval results.

## Architecture

```
Large Document
     ↓
Document Chunking
     ↓
┌─────────────────────────────────────┐
│  For Each Chunk:                    │
│  ├── Generate Embedding (FastEmbed) │
│  ├── Store Content in DDM           │
│  ├── Store Embedding in DDM         │
│  └── Index for FTS                  │
└─────────────────────────────────────┘
     ↓
Hybrid Search
     ↓
┌─────────────────────────────────────┐
│  Query                             │
│  ├── Full-Text Search (TF-IDF)     │
│  ├── Vector Search (Cosine Sim)    │
│  └── Combine Results (RRF)         │
└─────────────────────────────────────┘
```

## Prerequisites

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["full"] }
fastembed = "5.13"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = "0.4"
```

## Complete Implementation

```rust
// examples/text_documents_embeddings.rs
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use bund_blobstore::common::embeddings::{EmbeddingGenerator, cosine_similarity};
use bund_blobstore::common::grok_integration::GrokLogParser;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use std::path::PathBuf;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredDocument {
    id: String,
    content: String,
    embedding: Vec<f32>,
    metadata: DocumentMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentMetadata {
    title: String,
    author: String,
    created_at: i64,
    chunk_index: usize,
    total_chunks: usize,
    tags: Vec<String>,
    category: String,
}

#[derive(Debug, Clone)]
struct SearchResult {
    chunk_id: String,
    content: String,
    metadata: DocumentMetadata,
    fts_score: f32,
    vector_score: f32,
    hybrid_score: f32,
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     Text Documents with Embeddings - Complete Guide             ║");
    println!("║     Hybrid Search (FTS + Vector Similarity)                     ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    if let Err(e) = run_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_demo() -> Result<(), String> {
    // Step 1: Initialize components
    println!("📚 Step 1: Initializing Components\n");
    
    let data_dir = PathBuf::from("./text_embeddings_demo");
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).map_err(|e| format!("Failed to remove old dir: {}", e))?;
    }
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    
    // Initialize DataDistributionManager
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?
    ));
    println!("✓ DataDistributionManager initialized");
    
    // Initialize embedding generator
    let embedder = EmbeddingGenerator::with_download_progress(true)
        .map_err(|e| format!("Failed to create embedder: {}", e))?;
    
    if !embedder.is_download_complete() {
        println!("⏳ Downloading embedding model...");
        embedder.wait_for_download(300).map_err(|e| format!("Download failed: {}", e))?;
    }
    println!("✓ Embedding generator ready (dimension: {})\n", embedder.dimension());
    
    // Step 2: Create and chunk large document
    println!("📄 Step 2: Creating Large Technical Document\n");
    
    let document = create_technical_document();
    println!("Original document size: {} characters", document.len());
    
    // Intelligent chunking
    let chunks = chunk_document(&document, 800, 100);
    println!("✓ Document split into {} chunks\n", chunks.len());
    
    // Display chunk preview
    println!("First 3 chunks preview:");
    for (i, chunk) in chunks.iter().enumerate().take(3) {
        println!("  Chunk {}: {}...", i + 1, &chunk[..chunk.len().min(100)]);
    }
    
    // Step 3: Generate embeddings and store
    println!("\n💾 Step 3: Generating Embeddings and Storing in DataDistributionManager\n");
    
    let mut stored_docs = Vec::new();
    let total_chunks = chunks.len();
    
    for (i, chunk_content) in chunks.iter().enumerate() {
        println!("Processing chunk {}/{}", i + 1, total_chunks);
        
        // Generate embedding using FastEmbed
        let embedding = embedder.embed(chunk_content)?;
        
        // Create metadata
        let metadata = DocumentMetadata {
            title: "Technical Document on Vector Databases".to_string(),
            author: "Bund BlobStore Team".to_string(),
            created_at: chrono::Utc::now().timestamp(),
            chunk_index: i,
            total_chunks,
            tags: vec![
                "vector-database".to_string(),
                "embeddings".to_string(),
                "hybrid-search".to_string(),
                "rag".to_string(),
            ],
            category: "Technical Documentation".to_string(),
        };
        
        // Create stored document
        let doc_id = format!("doc_chunk_{:04}", i);
        let stored_doc = StoredDocument {
            id: doc_id.clone(),
            content: chunk_content.clone(),
            embedding: embedding.clone(),
            metadata: metadata.clone(),
        };
        
        // Store in DataDistributionManager
        let doc_data = serde_json::to_vec(&stored_doc)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        manager.write().put(&doc_id, &doc_data, None)
            .map_err(|e| format!("Failed to store document: {}", e))?;
        
        // Store embedding separately for fast vector search
        let embedding_key = format!("emb_{}", doc_id);
        let embedding_data: Vec<u8> = embedding.iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();
        manager.write().put(&embedding_key, &embedding_data, None)
            .map_err(|e| format!("Failed to store embedding: {}", e))?;
        
        stored_docs.push(stored_doc);
        
        if (i + 1) % 5 == 0 {
            println!("  ✓ Stored {} chunks", i + 1);
        }
    }
    
    println!("\n✓ Stored {} chunks with embeddings\n", stored_docs.len());
    
    // Display storage statistics
    let stats = manager.read().get_stats();
    println!("📊 Storage Statistics:");
    println!("  - Total records: {}", stats.total_records);
    println!("  - Documents: {}", stored_docs.len());
    println!("  - Embeddings: {}", stored_docs.len());
    println!("  - Shards: {:?}", stats.shard_distribution);
    println!("  - Load balance: {:.3}\n", stats.load_balance_score);
    
    // Step 4: Perform Hybrid Search
    println!("🔍 Step 4: Hybrid Search Queries\n");
    
    let queries = vec![
        "How do vector databases work?",
        "What is hybrid search?",
        "Explain document chunking strategies",
        "How to implement RAG systems?",
    ];
    
    for query in queries {
        println!("{}", "═".repeat(80));
        println!("Query: \"{}\"", query);
        println!("{}", "═".repeat(80));
        
        let results = hybrid_search(&manager, &embedder, &stored_docs, query, 3)?;
        
        println!("\nTop 3 Results:");
        for (i, result) in results.iter().enumerate() {
            println!("\n  {}. Chunk: {} (Score: {:.3})", i + 1, result.chunk_id, result.hybrid_score);
            println!("     FTS: {:.3} | Vector: {:.3}", result.fts_score, result.vector_score);
            println!("     Title: {}", result.metadata.title);
            println!("     Tags: {:?}", result.metadata.tags);
            println!("     Content: {}...", &result.content[..result.content.len().min(200)]);
        }
        println!();
    }
    
    // Step 5: Advanced Search with Filters
    println!("🔍 Step 5: Filtered Hybrid Search\n");
    
    let query = "performance optimization";
    let tags_filter = vec!["vector-database".to_string(), "embeddings".to_string()];
    
    println!("Query: \"{}\"", query);
    println!("Filter: tags in {:?}", tags_filter);
    
    let results = filtered_hybrid_search(&manager, &embedder, &stored_docs, query, &tags_filter, 3)?;
    
    println!("\nTop 3 Results (with tag filter):");
    for (i, result) in results.iter().enumerate() {
        println!("  {}. Score: {:.3} - {}...", i + 1, result.hybrid_score, &result.content[..result.content.len().min(150)]);
    }
    
    // Step 6: Verify Data Persistence
    println!("\n📈 Step 6: Verifying Data Persistence\n");
    
    let test_chunk_id = &stored_docs[0].id;
    let retrieved_data = manager.read().get(test_chunk_id)
        .map_err(|e| format!("Failed to retrieve: {}", e))?;
    
    if let Some(data) = retrieved_data {
        let retrieved_doc: StoredDocument = serde_json::from_slice(&data)
            .map_err(|e| format!("Failed to deserialize: {}", e))?;
        println!("✓ Successfully retrieved document '{}'", retrieved_doc.id);
        println!("  Content length: {} chars", retrieved_doc.content.len());
        println!("  Embedding dimension: {}", retrieved_doc.embedding.len());
        println!("  Metadata: {:?}", retrieved_doc.metadata);
    }
    
    // Cleanup
    println!("\n🧹 Cleaning up...\n");
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;
    
    println!("✅ Demo completed successfully!");
    println!("\n📊 Summary:");
    println!("  - Chunks stored: {}", stored_docs.len());
    println!("  - Embedding dimension: {}", embedder.dimension());
    println!("  - Hybrid search combines FTS + Vector similarity");
    println!("  - Results ranked using Reciprocal Rank Fusion (RRF)");
    
    Ok(())
}

fn create_technical_document() -> String {
    let mut doc = String::new();
    
    doc.push_str(r#"
# Vector Databases: Complete Guide

## Introduction to Vector Databases

Vector databases are specialized database systems designed to store and query high-dimensional vector embeddings. 
These embeddings represent unstructured data such as text, images, audio, and video in a numerical format that captures 
semantic meaning. Unlike traditional databases that rely on exact matches, vector databases excel at similarity search, 
finding items that are conceptually similar to a query vector.

## How Vector Databases Work

Vector databases use Approximate Nearest Neighbor (ANN) algorithms to efficiently find similar vectors in high-dimensional space. 
Common algorithms include HNSW (Hierarchical Navigable Small World), IVF (Inverted File Index), and PQ (Product Quantization). 
These algorithms trade off between search speed and accuracy, allowing for sub-linear search times even with millions of vectors.

### HNSW Algorithm

HNSW builds a multi-layer graph structure where each layer is a subset of the previous layer. Search starts at the top layer 
and navigates through the graph to find nearest neighbors. This provides excellent search quality with logarithmic complexity.

### IVF Algorithm

IVF partitions the vector space into clusters using k-means. During search, only vectors in the most relevant clusters are examined, 
significantly reducing the number of distance calculations needed.

## Full-Text Search Fundamentals

Full-text search (FTS) focuses on exact keyword matching and lexical analysis. It uses inverted indexes to quickly find documents 
containing specific words or phrases.

### TF-IDF Scoring

Term Frequency-Inverse Document Frequency (TF-IDF) evaluates how relevant a document is to a query:
- Term Frequency (TF): How often a term appears in a document
- Inverse Document Frequency (IDF): How rare a term is across all documents
- Score = TF * IDF

### BM25 Ranking

BM25 is an advanced ranking function that improves upon TF-IDF by introducing:
- Term frequency saturation (diminishing returns for repeated terms)
- Document length normalization
- Better handling of rare terms

## Hybrid Search Approaches

Hybrid search combines both techniques to leverage their respective strengths.

### Reciprocal Rank Fusion (RRF)

RRF combines rankings from multiple search methods using the formula:
RRF_score = Σ 1/(k + rank)

This method doesn't require normalized scores and handles different ranking scales well.

### Score Fusion

Combine relevance scores from both FTS and vector search using weighted averaging:
final_score = α * fts_score + (1-α) * vector_score

## Document Chunking Strategies

Chunking is critical for Retrieval-Augmented Generation (RAG) systems.

### Fixed-Size Chunking

Splits text into chunks of equal size, regardless of content boundaries. Simple to implement but may cut sentences.

### Semantic Chunking

Uses embeddings to find natural breakpoints where meaning changes. Higher quality but more computationally expensive.

### Recursive Chunking

Starts with larger chunks (paragraphs) and recursively splits oversized chunks. Balances coherence and size constraints.

### Overlap Strategies

Overlapping chunks (typically 10-20%) helps maintain context across chunk boundaries.

## RAG Implementation

Retrieval-Augmented Generation combines retrieval systems with LLMs.

### Indexing Phase
1. Document preprocessing and cleaning
2. Intelligent chunking with overlap
3. Embedding generation for each chunk
4. Metadata enrichment
5. Multi-modal indexing

### Retrieval Phase
1. Query understanding and expansion
2. Hybrid search execution
3. Re-ranking for relevance
4. Diversity optimization

### Generation Phase
1. Context window management
2. Prompt engineering
3. Token limit handling
4. Response streaming

## Performance Optimization

### Indexing Optimization
- Pre-filtering: Apply metadata filters before vector search
- Post-filtering: Apply filters after vector search
- Quantization: Reduce vector precision to save memory
- Partitioning: Split vectors into partitions based on metadata

### Query Optimization
- Batch Queries: Process multiple queries together
- Caching: Cache frequent query results
- Approximate Search: Use ANN for speed, exact for accuracy
- Early Termination: Stop search when enough results found

## Real-World Applications

### E-commerce
- Product discovery using visual and text search
- Similar item recommendations
- Personalized ranking

### Enterprise Search
- Legal document retrieval
- Technical documentation search
- Knowledge base Q&A

### Healthcare
- Medical literature search
- Patient record similarity
- Drug discovery
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

fn hybrid_search(
    manager: &Arc<RwLock<DataDistributionManager>>,
    embedder: &EmbeddingGenerator,
    documents: &[StoredDocument],
    query: &str,
    top_k: usize,
) -> Result<Vec<SearchResult>, String> {
    // Generate query embedding
    let query_embedding = embedder.embed(query)?;
    
    // Get FTS results
    let fts_results = full_text_search(documents, query);
    
    // Get vector results
    let vector_results = vector_search(documents, &query_embedding, top_k * 2);
    
    // Combine using Reciprocal Rank Fusion
    let combined = combine_results(&fts_results, &vector_results, top_k);
    
    // Retrieve full content for results
    let mut results = Vec::new();
    for (doc_id, hybrid_score) in combined {
        if let Some(doc) = documents.iter().find(|d| d.id == doc_id) {
            let fts_score = fts_results.iter()
                .find(|(id, _)| *id == doc_id)
                .map(|(_, s)| *s)
                .unwrap_or(0.0);
            let vector_score = vector_results.iter()
                .find(|(id, _)| *id == doc_id)
                .map(|(_, s)| *s)
                .unwrap_or(0.0);
            
            results.push(SearchResult {
                chunk_id: doc.id.clone(),
                content: doc.content.clone(),
                metadata: doc.metadata.clone(),
                fts_score,
                vector_score,
                hybrid_score,
            });
        }
    }
    
    Ok(results)
}

fn filtered_hybrid_search(
    manager: &Arc<RwLock<DataDistributionManager>>,
    embedder: &EmbeddingGenerator,
    documents: &[StoredDocument],
    query: &str,
    tags: &[String],
    top_k: usize,
) -> Result<Vec<SearchResult>, String> {
    // Filter documents by tags
    let filtered_docs: Vec<StoredDocument> = documents.iter()
        .filter(|doc| doc.metadata.tags.iter().any(|tag| tags.contains(tag)))
        .cloned()
        .collect();
    
    if filtered_docs.is_empty() {
        return Ok(Vec::new());
    }
    
    // Perform hybrid search on filtered documents
    hybrid_search(manager, embedder, &filtered_docs, query, top_k)
}

fn full_text_search(documents: &[StoredDocument], query: &str) -> Vec<(String, f32)> {
    let query_terms: Vec<&str> = query.split_whitespace().collect();
    let mut results = Vec::new();
    
    for doc in documents {
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
        
        if !query_terms.is_empty() {
            score /= query_terms.len() as f32;
        }
        
        if score > 0.0 {
            results.push((doc.id.clone(), score));
        }
    }
    
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results
}

fn vector_search(
    documents: &[StoredDocument],
    query_embedding: &[f32],
    top_k: usize,
) -> Vec<(String, f32)> {
    let mut results: Vec<(String, f32)> = documents
        .iter()
        .map(|doc| {
            let similarity = cosine_similarity(query_embedding, &doc.embedding);
            (doc.id.clone(), similarity)
        })
        .collect();
    
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(top_k);
    results
}

fn combine_results(
    fts_results: &[(String, f32)],
    vector_results: &[(String, f32)],
    top_k: usize,
) -> Vec<(String, f32)> {
    use std::collections::HashMap;
    
    let mut scores: HashMap<String, f32> = HashMap::new();
    let k = 60.0;
    
    // Add FTS results with RRF scores
    for (rank, (doc_id, _)) in fts_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + (rank + 1) as f32);
        *scores.entry(doc_id.clone()).or_insert(0.0) += rrf_score;
    }
    
    // Add vector results with RRF scores
    for (rank, (doc_id, _)) in vector_results.iter().enumerate() {
        let rrf_score = 1.0 / (k + (rank + 1) as f32);
        *scores.entry(doc_id.clone()).or_insert(0.0) += rrf_score;
    }
    
    let mut results: Vec<(String, f32)> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    results.truncate(top_k);
    results
}
```

## Integration Guide

### 1. Adding to Your Project

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["full"] }
fastembed = "5.13"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 2. Basic Usage Pattern

```rust
// Initialize
let manager = DataDistributionManager::new("./data", DistributionStrategy::RoundRobin)?;
let embedder = EmbeddingGenerator::new()?;

// Store documents
let chunks = chunk_document(long_text, 800, 100);
for (i, chunk) in chunks.iter().enumerate() {
    let embedding = embedder.embed(chunk)?;
    store_in_manager(&manager, &format!("chunk_{}", i), chunk, embedding)?;
}

// Search
let results = hybrid_search(&manager, &embedder, "your query", 5)?;
```

### 3. Performance Tips

- **Batch Size**: 32-128 embeddings per batch for optimal performance
- **Chunk Size**: 500-1000 characters for good context preservation
- **Overlap**: 10-20% of chunk size for continuity
- **Cache**: Cache frequent query embeddings
- **Indexing**: Create metadata indexes for frequently filtered fields

## Best Practices

1. **Chunk Size Selection**
   - Small (300-500 chars): Better for specific facts
   - Medium (500-800 chars): Balanced for general use
   - Large (800-1200 chars): Better for context preservation

2. **Embedding Model Choice**
   - all-MiniLM-L6-v2: Fast, good for production
   - BAAI/bge-base-en-v1.5: Higher quality, slower

3. **Hybrid Search Tuning**
   - Adjust RRF constant (k=60 is standard)
   - Weight FTS vs vector based on use case
   - Use filters to narrow search scope

4. **Storage Optimization**
   - Store embeddings separately from content
   - Use compression for large datasets
   - Implement retention policies

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Slow embedding generation | Use batch processing, reduce model size |
| Poor search results | Adjust chunk size, increase overlap, tune thresholds |
| High memory usage | Process in smaller batches, use streaming |
| Model download fails | Check network, increase timeout, use cache |

## See Also

- [Embeddings Module Documentation](./EMBEDDINGS.md)
- [Data Distribution Manager](./DATA_DISTRIBUTION.md)
- [Hybrid Search Demo](../examples/hybrid_search_demo.rs)

## Complete Example

Run the complete example:
```bash
cargo run --example text_documents_embeddings --release
```

This will:
1. Initialize DataDistributionManager and FastEmbed
2. Create a large technical document
3. Split into intelligent chunks
4. Generate embeddings for each chunk
5. Store everything in DataDistributionManager
6. Perform hybrid search queries
7. Display results with scores and metadata
```
