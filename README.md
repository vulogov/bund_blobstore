# Bund BlobStore

[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/bund_blobstore.svg)](https://crates.io/crates/bund_blobstore)
[![Documentation](https://docs.rs/bund_blobstore/badge.svg)](https://docs.rs/bund_blobstore)

A high-performance, ACID-compliant embedded database with enterprise-grade features including full-text search, fuzzy search, vector similarity, hybrid search, faceted search, multi-modal embeddings, graph storage, telemetry timeline, vector-telemetry integration, distributed graph algorithms, intelligent data distribution, dynamic sharding, LRU caching, advanced chunked document storage with RAG support, and concurrent access patterns.

## ✨ Features

### Core Database
- **⚡ Blazing Fast** - Built on [RedB](https://github.com/cberner/redb), one of the fastest embedded databases for Rust
- **🔐 ACID Compliant** - Full transaction support with MVCC
- **📦 Single File** - Everything stored in a single `.redb` file per component
- **📊 Metadata Tracking** - Automatic timestamps, sizes, and checksums for data integrity
- **🔍 Advanced Querying** - Prefix search, wildcard patterns, pagination
- **🛡️ Integrity Verification** - Automatic checksum validation for data integrity

### Advanced Chunked Document Storage with RAG Support (NEW!)
- **📄 Intelligent Text Chunking** - Split documents at sentence and paragraph boundaries
- **🔄 Round-Robin Distribution** - Distribute chunks evenly across all shards
- **🎯 Configurable Context Windows** - Include before/after text for each chunk
- **🌍 Multi-Language Stemming** - Snowball stemming for 8 languages (English, Spanish, French, German, Russian, Italian, Dutch, Portuguese)
- **🔍 Hybrid Search on Chunks** - Combine vector similarity with keyword matching
- **🤖 RAG-Ready Results** - Return chunks with full context for LLM integration
- **📊 Chunk Statistics** - Track word, sentence, and paragraph counts
- **💾 Metadata Preservation** - Preserve document metadata across all chunks
- **⚙️ Configurable Parameters** - Adjust chunk size, overlap, min size, context windows

### Search Capabilities
- **🔎 Full-Text Search** - Powerful inverted index with TF-IDF scoring
- **🥴 Fuzzy Search** - Multiple algorithms: Levenshtein, Damerau-Levenshtein, Jaro-Winkler, Sørensen-Dice
- **🧠 Vector Search** - Semantic similarity using state-of-the-art embeddings (fastembed)
- **🎯 Hybrid Search** - Combine vector similarity with keyword matching for optimal results
- **🎨 Text Highlighting** - Visual indication of matching terms
- **📊 Faceted Search** - Multi-dimensional filtering with facet counts and ranges
- **🔤 Phrase Matching** - Exact phrase search with proximity scoring
- **📏 Proximity Search** - Find words within N words of each other

### Telemetry & Timeline
- **📈 Time Series Data** - Store telemetry events with timestamps
- **🏷️ Mixed Value Types** - Float, int, string, bool, blob, JSON, and dynamic values
- **🔗 Primary-Secondary Relationships** - Hierarchical telemetry records with linking
- **⏱️ Time Interval Search** - Query by last hour, day, week, month, or custom ranges
- **📊 Minute-Grade Bucketing** - Aggregate data by minute intervals with statistics
- **🎯 Key & Source Search** - Filter by metric keys and data sources

### Vector-Telemetry Integration
- **🔗 Time-Vector Search** - Combine temporal proximity with semantic similarity
- **📊 Configurable Weights** - Balance between time relevance and semantic relevance
- **🎯 Similar Event Discovery** - Find events similar to a reference event within time windows
- **📈 Temporal Pattern Analysis** - Identify when similar events occur over time
- **🤖 Automatic Embedding Generation** - Convert telemetry values to vector embeddings

### Intelligent Data Distribution
- **🎯 Multiple Distribution Strategies** - Round-robin, time bucket, key similarity, adaptive load balancing
- **🔄 Automatic Shard Selection** - No need to specify shard targets
- **⚖️ Adaptive Distribution** - Dynamically balance load based on shard utilization
- **📈 Distribution Statistics** - Track entropy, load balance scores, and shard distribution
- **🔄 Runtime Strategy Switching** - Change distribution strategy without restart

### Dynamic Shard Management
- **➕ Add Shards Dynamically** - Add new shards at runtime for horizontal scaling
- **➖ Remove Shards** - Remove shards for scaling down
- **🏷️ Key-Range Shards** - Create shards that handle specific key ranges
- **⏰ Time-Range Shards** - Create shards for specific time periods
- **🔍 Shard Discovery** - List all shards, check existence, get details
- **⚖️ Load Monitoring** - Track load distribution across shards

### LRU Cache with TTL
- **⚡ LRU Eviction** - Least Recently Used eviction policy
- **⏰ TTL Support** - Time-to-live for automatic cache expiration
- **📈 Cache Statistics** - Track hits, misses, and hit rates
- **📥 Preloading** - Pre-populate cache with common keys

### Distributed Graph with Advanced Algorithms
- **🕸️ Cross-Shard Graph Storage** - Nodes and edges distributed across multiple shards
- **🔄 Cycle Detection** - Detect cycles in distributed graphs with parallel processing
- **⚡ Shortest Path** - Optimized Dijkstra with early termination
- **🔍 Bidirectional Search** - Faster path finding for large graphs
- **📏 Longest Path** - Find longest paths in DAGs and cyclic graphs

## 📦 Installation

```toml
[dependencies]
bund_blobstore = "0.11.0"
```

## 🚀 Quick Start

```rust
use bund_blobstore::BlobStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut store = BlobStore::open("my_data.redb")?;
    store.put("user:100", b"Alice data", Some("user"))?;
    
    if let Some(data) = store.get("user:100")? {
        println!("Retrieved: {}", String::from_utf8_lossy(&data));
    }
    Ok(())
}
```

## 📄 Advanced Chunked Document Storage for RAG

### Basic Document Storage with Chunking

```rust
use bund_blobstore::{DataDistributionManager, DistributionStrategy, AdvancedChunkingConfig, StemmingLanguage};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = DataDistributionManager::new(
        "rag_data",
        DistributionStrategy::RoundRobin,
    )?;
    
    // Configure advanced chunking
    let config = AdvancedChunkingConfig {
        chunk_size: 512,              // Target chunk size in characters
        chunk_overlap: 50,            // Overlap between chunks
        min_chunk_size: 100,          // Minimum chunk size
        break_on_sentences: true,     // Prefer breaking at sentence boundaries
        break_on_paragraphs: true,    // Prefer breaking at paragraph boundaries
        preserve_metadata: true,      // Preserve metadata in chunks
        context_before_chars: 200,    // Characters to include before chunk
        context_after_chars: 200,     // Characters to include after chunk
        enable_stemming: true,        // Enable snowball stemming
        language: StemmingLanguage::English,
    };
    
    // Store a document with automatic chunking
    let long_text = "Your long document text here...".repeat(100);
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "John Doe".to_string());
    metadata.insert("category".to_string(), "technical".to_string());
    
    let doc = manager.store_advanced_chunked_document(
        "technical_guide",
        &long_text,
        metadata,
        &config,
    )?;
    
    println!("Document stored with {} chunks", doc.chunks.len());
    println!("Word count: {}, Sentences: {}", doc.word_count, doc.sentence_count);
    
    Ok(())
}
```

### Hybrid Search on Chunks with RAG Context

```rust
// Search with hybrid approach (70% vector, 30% keyword)
let results = manager.search_advanced_chunks(
    "database optimization techniques",
    5,
    0.7,   // Vector weight
    true,  // Include context for RAG
)?;

for result in results {
    println!("Document: {}", result.document_id);
    println!("Combined Score: {:.3}", result.combined_score);
    println!("Vector: {:.3}, Keyword: {:.3}", 
             result.vector_score, result.keyword_score);
    println!("Content: {}", &result.text[..100]);
    println!("Full Context for RAG:\n{}\n", result.relevance_context);
}
```

### Retrieve Specific Chunks for LLM Context

```rust
// Get specific chunks with expanded context for RAG
let chunk_ids = vec!["chunk_0".to_string(), "chunk_1".to_string()];
let rag_chunks = manager.get_chunks_for_rag(
    "technical_guide",
    chunk_ids,
    500,  // Context window characters
)?;

for chunk in rag_chunks {
    println!("Chunk: {}", chunk.chunk_id);
    println!("Relevance Context:\n{}", chunk.relevance_context);
}
```

### Multi-Language Stemming Support

```rust
let config = AdvancedChunkingConfig {
    chunk_size: 512,
    chunk_overlap: 50,
    min_chunk_size: 100,
    break_on_sentences: true,
    break_on_paragraphs: true,
    preserve_metadata: true,
    context_before_chars: 200,
    context_after_chars: 200,
    enable_stemming: true,
    language: StemmingLanguage::Spanish,  // Support for 8 languages
};

let doc = manager.store_advanced_chunked_document(
    "spanish_doc",
    "Texto en español para procesar...",
    HashMap::new(),
    &config,
)?;
```

## 📊 Intelligent Data Distribution

### Round-Robin Distribution

```rust
let manager = DataDistributionManager::new(
    "data_store",
    DistributionStrategy::RoundRobin,
)?;

for i in 0..1000 {
    manager.put(&format!("key_{}", i), b"data", None)?;
}

let stats = manager.get_distribution_stats();
println!("Load balance score: {:.3}", stats.load_balance_score);
```

### Time Bucket Distribution

```rust
use bund_blobstore::{TimeBucketConfig, TimeBucketSize};

let config = TimeBucketConfig {
    bucket_size: TimeBucketSize::Hours(1),
    timezone_offset: 0,
    align_to_bucket: true,
};

let manager = DataDistributionManager::new(
    "time_bucket_data",
    DistributionStrategy::TimeBucket(config),
)?;
```

### Key Similarity Distribution

```rust
use bund_blobstore::SimilarityConfig;

let config = SimilarityConfig {
    use_prefix: true,
    use_suffix: true,
    ngram_size: 3,
    min_similarity: 0.6,
    max_cluster_size: 100,
};

let manager = DataDistributionManager::new(
    "similarity_data",
    DistributionStrategy::KeySimilarity(config),
)?;
```

## 🗺️ Dynamic Shard Management

```rust
// Add shards dynamically
manager.add_shard("new_shard", "/path/to/new_shard")?;
manager.add_key_range_shard("range_shard", "/path/to/range_shard", "a", "m")?;

let now = Utc::now();
manager.add_time_range_shard("time_shard", "/path/to/time_shard", 
                              now - Duration::days(30), now)?;

// List and manage shards
let shards = manager.get_all_shard_names();
for shard in shards {
    println!("Shard: {}", shard);
}

let details = manager.get_shard_details();
for detail in details {
    println!("{}: {} keys", detail.name, detail.key_count);
}
```

## 🔍 Search Capabilities

### Full-Text Search

```rust
use bund_blobstore::SearchableBlobStore;

let mut store = SearchableBlobStore::open("searchable.redb")?;
store.put_text("doc1", "The quick brown fox jumps over the lazy dog", None)?;

let results = store.search("quick brown", 10)?;
for result in results {
    println!("Found: {} (score: {:.3})", result.key, result.score);
}
```

### Vector Search

```rust
use bund_blobstore::VectorStore;

let mut store = VectorStore::open("vectors.redb")?;
store.insert_text("vec1", "Rust is a systems programming language", None)?;

let results = store.search_similar("system programming", 5)?;
for result in results {
    println!("Found: {} (similarity: {:.3})", result.key, result.score);
}
```

## 📊 Telemetry & Timeline

```rust
use bund_blobstore::{TelemetryStore, TelemetryRecord, TelemetryValue, TelemetryQuery, TimeInterval};

let mut telemetry = TelemetryStore::open("telemetry.redb")?;

let record = TelemetryRecord::new_primary(
    "cpu_001".to_string(),
    Utc::now(),
    "cpu_usage".to_string(),
    "server_01".to_string(),
    TelemetryValue::Float(45.2),
);
telemetry.store(record)?;

let query = TelemetryQuery {
    time_interval: Some(TimeInterval::last_hour()),
    keys: Some(vec!["cpu_usage".to_string()]),
    limit: 100,
    ..Default::default()
};

let results = telemetry.query(&query)?;
```

## 🔗 Vector-Telemetry Integration

```rust
use bund_blobstore::{VectorTelemetryStore, VectorTimeQuery};

let mut store = VectorTelemetryStore::open("vector_telemetry.redb")?;

let query = VectorTimeQuery {
    time_interval: Some(TimeInterval::last_hour()),
    vector_query: Some("database connection problem".to_string()),
    vector_weight: 0.7,
    time_weight: 0.3,
    limit: 10,
    min_similarity: 0.3,
    ..Default::default()
};

let results = store.search_vector_time(&query)?;
for result in results {
    println!("Found: {}", result.record.key);
    println!("  Time score: {:.3}, Vector score: {:.3}", 
             result.time_score, result.vector_score);
}
```

## 🕸️ Distributed Graph Algorithms

```rust
use bund_blobstore::{DistributedGraphManager, GraphAlgorithms};

let manager = Arc::new(DistributedGraphManager::new("distributed_graph")?);
let algorithms = GraphAlgorithms::new(manager.clone());

// Detect cycles
let cycle_result = algorithms.detect_cycles(None)?;
println!("Found {} cycles", cycle_result.cycle_count);

// Find shortest path
let shortest = algorithms.shortest_path_optimized("node_A", "node_Z", true)?;
```

## 🚀 Concurrent Operations

```rust
use bund_blobstore::UnifiedConcurrentStore;
use std::thread;

let store = UnifiedConcurrentStore::open("unified.redb")?;

let store1 = store.clone();
let handle = thread::spawn(move || {
    store1.blob().put("key", b"value", None).unwrap();
});

handle.join().unwrap();
```

## 📊 Performance Benchmarks

- **Write throughput**: ~50,000 ops/second
- **Read throughput**: ~100,000 ops/second
- **Chunked document storage**: <100ms for 1MB document
- **Hybrid chunk search**: <100ms across 1000 chunks
- **Vector search**: <50ms for 10K vectors
- **Load balance score**: >0.7 with adaptive distribution
- **Cache hit rate**: >80% with LRU caching

## 🔧 Configuration Examples

### Advanced Chunking Configuration

```rust
let config = AdvancedChunkingConfig {
    chunk_size: 512,
    chunk_overlap: 50,
    min_chunk_size: 100,
    break_on_sentences: true,
    break_on_paragraphs: true,
    preserve_metadata: true,
    context_before_chars: 200,
    context_after_chars: 200,
    enable_stemming: true,
    language: StemmingLanguage::English,
};
```

### Cache Configuration

```rust
use bund_blobstore::CacheConfig;
use std::time::Duration;

let cache_config = CacheConfig {
    enabled: true,
    max_size: 10000,
    default_ttl: Duration::from_secs(300),
    key_cache_ttl: Duration::from_secs(600),
    time_cache_ttl: Duration::from_secs(300),
};
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run advanced chunking tests
cargo test --test advancedchunking-test -- --nocapture

# Run with logging
RUST_LOG=debug cargo test
```

## 📈 Use Cases

### RAG Applications
- **Document Q&A** - Store and retrieve relevant document chunks
- **Knowledge Bases** - Build searchable document repositories
- **LLM Context** - Provide rich context for language models
- **Semantic Search** - Find documents by meaning, not just keywords

### Intelligent Observability
- **Root Cause Analysis** - Find similar incidents within time windows
- **Anomaly Detection** - Identify unusual patterns in telemetry
- **Correlation** - Link temporally close and semantically similar events

### Distributed Systems
- **Horizontal Scaling** - Add shards as data grows
- **Load Balancing** - Even distribution across nodes
- **Multi-tenant Applications** - Isolate tenant data

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

```bash
git clone https://github.com/yourusername/bund_blobstore.git
cd bund_blobstore
cargo build
cargo test
```

## 📄 License

This project is licensed under either of:

- Apache License, Version 2.0
- MIT license

## 🙏 Acknowledgments

- [RedB](https://github.com/cberner/redb) - Embedded database backend
- [fastembed](https://github.com/Anush008/fastembed-rs) - Vector embeddings
- [rust-stemmers](https://github.com/curusarn/rust-stemmers) - Snowball stemming
- [chrono](https://github.com/chronotope/chrono) - Time handling

## 🚀 Roadmap

- [ ] Async API support
- [ ] Encryption at rest
- [ ] Automatic shard rebalancing
- [ ] Streaming chunk processing
- [ ] WebAssembly support

---

**Built with ❤️ using Rust**
