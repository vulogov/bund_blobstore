Based on my review of the current `bund_blobstore` repository, the project is a **high-performance, ACID-compliant embedded database written in Rust** with an impressive range of enterprise-grade features. The latest version is `0.13.0` (updated April 9, 2026), and it has evolved significantly with advanced capabilities for telemetry, RAG applications, distributed systems, and graph algorithms.

Here is an updated `README.md` that incorporates the latest features and proper references to the documentation files in the `Documentation` folder:

---

# Bund BlobStore

[![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)

A **high-performance, ACID-compliant embedded database** with enterprise-grade features including full-text search, fuzzy search, vector similarity, hybrid search, faceted search, multi-modal embeddings, graph storage, telemetry timeline, vector-telemetry integration, distributed graph algorithms, intelligent data distribution, dynamic sharding, LRU caching, advanced chunked document storage with RAG support, and multidimensional telemetry storage.

## 📚 Documentation

Comprehensive documentation for all major features is available in the `Documentation/` folder:

| Feature | Documentation |
|---------|---------------|
| **Multidimensional Storage** | [Documentation/MULTIDIMENSIONAL_STORAGE.md](Documentation/MULTIDIMENSIONAL_STORAGE.md) - Complete guide to 1D, 2D, and 3D telemetry storage with FIFO queues |
| **Chunked Document Storage** | [Documentation/CHUNKED_DOCUMENT_STORAGE.md](Documentation/CHUNKED_DOCUMENT_STORAGE.md) - RAG-ready document processing with advanced chunking |
| **Close & Sync Operations** | [Documentation/CLOSE_SYNC.md](Documentation/CLOSE_SYNC.md) - Cache management and synchronization |
| **Distributed Graph Algorithms** | [Documentation/DISTRIBUTED_GRAPH.md](Documentation/DISTRIBUTED_GRAPH.md) - Cross-shard graph operations |
| **Vector & Hybrid Search** | [Documentation/SEARCH.md](Documentation/SEARCH.md) - Vector similarity and hybrid search capabilities |
| **Telemetry Timeline** | [Documentation/TELEMETRY.md](Documentation/TELEMETRY.md) - Time series data and event storage |
| **Data Distribution** | [Documentation/DATA_DISTRIBUTION.md](Documentation/DATA_DISTRIBUTION.md) - Sharding and load balancing strategies |

## ✨ Features

### Core Database
- **⚡ Blazing Fast** - Built on RedB, one of the fastest embedded databases for Rust
- **🔐 ACID Compliant** - Full transaction support with MVCC
- **📦 Single File** - Everything stored in a single `.redb` file per component
- **📊 Metadata Tracking** - Automatic timestamps, sizes, and checksums for data integrity
- **🔍 Advanced Querying** - Prefix search, wildcard patterns, pagination
- **🛡️ Integrity Verification** - Automatic checksum validation

### Multidimensional Telemetry Storage
- **📐 1D, 2D, 3D Coordinate Spaces** - Store telemetry in linear, grid, or volumetric spaces
- **🔄 Fixed-Size FIFO Queues** - Automatic eviction of oldest samples when capacity is reached
- **🎯 Coordinate-Based Access** - Push and query samples by specific coordinates
- **📊 Time Range Queries** - Retrieve samples within custom time windows
- **🏷️ Mixed Value Types** - Float, int, string, bool, blob, JSON, dynamic values
- **💾 Metadata Preservation** - Store custom metadata with each sample
- **⚡ Distributed Storage** - Automatic sharding using round-robin distribution
- **🔍 Vector Search** - Semantic search for dimension labels
- **🗑️ Dynamic Dimension Management** - Create, list, and delete dimensions at runtime

### Advanced Chunked Document Storage with RAG Support
- **📄 Intelligent Text Chunking** - Split documents at sentence and paragraph boundaries
- **🔄 Round-Robin Distribution** - Distribute chunks evenly across all shards
- **🎯 Configurable Context Windows** - Include before/after text for each chunk
- **🌍 Multi-Language Stemming** - Snowball stemming for 8+ languages (English, Russian, German, French, Spanish, Italian, Dutch, Portuguese)
- **🔍 Hybrid Search on Chunks** - Combine vector similarity with keyword matching
- **🤖 RAG-Ready Results** - Return chunks with full context for LLM integration
- **📊 Chunk Statistics** - Track word, sentence, and paragraph counts
- **💾 Metadata Preservation** - Preserve document metadata across all chunks

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

Add this to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = "0.13.0"
```

## 🚀 Quick Start

### Basic Key-Value Operations

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

## 📐 Multidimensional Telemetry Storage

```rust
use bund_blobstore::common::{
    MultidimensionalStorage, DimensionType, Coordinate, Coord1D, Coord2D, Coord3D
};
use bund_blobstore::TelemetryValue;
use std::collections::HashMap;

let storage = MultidimensionalStorage::open("telemetry.db")?;

// Create 1D dimension for sensors
storage.create_dimension("sensors", DimensionType::OneD, 1000, None)?;

// Store temperature reading at sensor 42
let coord = Coordinate::OneD(Coord1D(42));
storage.push_sample(
    "sensors",
    coord,
    TelemetryValue::Float(23.5),
    None,
    HashMap::new(),
)?;

// Retrieve latest 10 samples
let samples = storage.get_latest_samples("sensors", coord, 10)?;
```

For detailed documentation, see [Documentation/MULTIDIMENSIONAL_STORAGE.md](Documentation/MULTIDIMENSIONAL_STORAGE.md).

## 📄 Advanced Chunked Document Storage

```rust
use bund_blobstore::{DataDistributionManager, AdvancedChunkingConfig, StemmingLanguage};

let manager = DataDistributionManager::new("rag_data", DistributionStrategy::RoundRobin)?;

let config = AdvancedChunkingConfig {
    chunk_size: 512,
    chunk_overlap: 50,
    min_chunk_size: 100,
    break_on_sentences: true,
    break_on_paragraphs: true,
    enable_stemming: true,
    language: StemmingLanguage::English,
    ..Default::default()
};

let doc = manager.store_advanced_chunked_document(
    "technical_guide",
    long_text,
    metadata,
    &config,
)?;

// Hybrid search across chunks
let results = manager.search_advanced_chunks("database optimization", 5, 0.7, true)?;
```

For detailed documentation, see [Documentation/CHUNKED_DOCUMENT_STORAGE.md](Documentation/CHUNKED_DOCUMENT_STORAGE.md).

## 🔍 Search Capabilities

### Full-Text Search

```rust
use bund_blobstore::SearchableBlobStore;

let mut store = SearchableBlobStore::open("searchable.redb")?;
store.put_text("doc1", "The quick brown fox jumps over the lazy dog", None)?;

let results = store.search("quick brown", 10)?;
```

### Vector Search

```rust
use bund_blobstore::VectorStore;

let mut store = VectorStore::open("vectors.redb")?;
store.insert_text("vec1", "Rust is a systems programming language", None)?;

let results = store.search_similar("system programming", 5)?;
```

For detailed documentation, see [Documentation/SEARCH.md](Documentation/SEARCH.md).

## 📊 Telemetry Timeline

```rust
use bund_blobstore::{TelemetryStore, TelemetryRecord, TelemetryValue, TelemetryQuery, TimeInterval};
use chrono::Utc;

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

For detailed documentation, see [Documentation/TELEMETRY.md](Documentation/TELEMETRY.md).

## 🕸️ Distributed Graph Algorithms

```rust
use bund_blobstore::{DistributedGraphManager, GraphAlgorithms};
use std::sync::Arc;

let manager = Arc::new(DistributedGraphManager::new("distributed_graph")?);
let algorithms = GraphAlgorithms::new(manager.clone());

// Detect cycles
let cycle_result = algorithms.detect_cycles(None)?;
println!("Found {} cycles", cycle_result.cycle_count);

// Find shortest path
let shortest = algorithms.shortest_path_optimized("node_A", "node_Z", true)?;
```

For detailed documentation, see [Documentation/DISTRIBUTED_GRAPH.md](Documentation/DISTRIBUTED_GRAPH.md).

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

| Operation | Throughput/Latency |
|-----------|-------------------|
| Write throughput | ~50,000 ops/second |
| Read throughput | ~100,000 ops/second |
| Multidimensional storage | <10ms per sample |
| Time range queries | <50ms for 10K samples |
| Chunked document storage | <100ms for 1MB document |
| Hybrid chunk search | <100ms across 1000 chunks |
| Vector search | <50ms for 10K vectors |
| Load balance score | >0.7 with adaptive distribution |

## 🔧 Configuration Examples

### Multidimensional Storage Configuration

```rust
let storage = MultidimensionalStorage::open("data.db")?;

// 1D with capacity 1000
storage.create_dimension("sensors", DimensionType::OneD, 1000, None)?;

// 2D with bounds
let bounds = Bounds {
    min_x: 0, max_x: 100,
    min_y: Some(0), max_y: Some(100),
    min_z: None, max_z: None,
};
storage.create_dimension("grid", DimensionType::TwoD, 500, Some(bounds))?;
```

### Chunking Configuration

```rust
let config = AdvancedChunkingConfig {
    chunk_size: 512,
    chunk_overlap: 50,
    min_chunk_size: 100,
    break_on_sentences: true,
    break_on_paragraphs: true,
    enable_stemming: true,
    language: StemmingLanguage::English,
    ..Default::default()
};
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run multidimensional storage tests
cargo test --test commonmultidimensionalstorage-test -- --nocapture

# Run chunked document tests
cargo test --test advancedchunking-test -- --nocapture

# Run with logging
RUST_LOG=debug cargo test
```

## 📈 Use Cases

### Multidimensional Telemetry
- **IoT Sensor Networks** - Temperature, humidity, pressure sensors in 1D/2D/3D spaces
- **Spatial Data Analysis** - Grid-based environmental monitoring
- **Voxel Storage** - 3D scanning and volumetric data
- **Time-Series Databases** - Historical data with FIFO retention

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
git clone https://github.com/vulogov/bund_blobstore.git
cd bund_blobstore
cargo build
cargo test
```

## 📄 License

This project is licensed under either of:

- Apache License, Version 2.0
- MIT license

## 🙏 Acknowledgments

- [RedB](https://github.com/redb/redb) - Embedded database backend
- [fastembed](https://github.com/qdrant/fastembed) - Vector embeddings
- [rust-stemmers](https://github.com/GuillaumeGomez/rust-stemmers) - Snowball stemming
- [chrono](https://github.com/chronotope/chrono) - Time handling

## 🚀 Roadmap

- Async API support
- Encryption at rest
- Automatic shard rebalancing
- Streaming chunk processing
- WebAssembly support
- 4D+ dimensional storage
- Real-time analytics

---

**Built with ❤️ using Rust**
