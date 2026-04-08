```markdown
# Bund BlobStore

A high-performance, ACID-compliant embedded database with advanced features for modern data applications.

## Overview

Bund BlobStore is a feature-rich embedded database written in Rust, designed for applications requiring advanced data management capabilities. It combines traditional key-value storage with cutting-edge features like vector similarity search, full-text search, time-series telemetry, distributed graph processing, and intelligent data distribution.

## Key Features

### Core Database Features
- **ACID Compliance** - Full transaction support with atomicity, consistency, isolation, and durability
- **High Performance** - Optimized for both read and write operations with configurable caching
- **Concurrent Access** - Thread-safe operations with fine-grained locking
- **Dynamic Sharding** - Automatic data distribution across shards for horizontal scaling

### Search Capabilities
- **Full-Text Search** - Advanced text search with relevance scoring (TF-IDF, BM25)
- **Vector Similarity Search** - Semantic search using embeddings with cosine similarity
- **Hybrid Search** - Combine FTS and vector search for optimal results
- **Fuzzy Search** - Approximate string matching and typo tolerance
- **Faceted Search** - Filter and aggregate search results by metadata

### Telemetry & Time-Series
- **Telemetry Timeline** - Optimized storage for time-series telemetry data
- **Primary/Secondary Relationships** - Link related telemetry records
- **Vector Timeline** - Combine time-series with vector embeddings
- **Downsampling & Aggregation** - Built-in data reduction and statistics

### Data Processing
- **Document Chunking** - Intelligent splitting of large documents for RAG applications
- **Grok Pattern Parsing** - Parse unstructured logs using Grok patterns
- **Log Ingestion** - Batch processing with deduplication and embedding generation
- **Worker Pool** - Multi-threaded log processing with configurable workers

### Analytics
- **Root Cause Analysis** - Discover causal relationships between events
- **Pattern Mining** - Identify frequent event sequences using Apriori algorithm
- **Correlation Analysis** - Build correlation matrices between event types
- **Anomaly Detection** - Identify unusual patterns in telemetry data

### Graph Processing
- **Distributed Graphs** - Store and query graph data across shards
- **Graph Algorithms** - Path finding, cycle detection, centrality measures
- **Real-Time Traversal** - Efficient graph navigation and querying

## Quick Start

```rust
use bund_blobstore::blobstore::BlobStore;
use bund_blobstore::search::SearchIndex;
use bund_blobstore::vector::VectorStore;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize blob store
    let store = BlobStore::new("./data")?;
    
    // Store data
    store.put("key1", b"Hello World")?;
    
    // Retrieve data
    let data = store.get("key1")?;
    println!("Retrieved: {}", String::from_utf8_lossy(&data));
    
    // Full-text search
    let search_index = SearchIndex::new("./search_index")?;
    search_index.index_document("doc1", "This is a test document")?;
    let results = search_index.search("test", 10)?;
    
    Ok(())
}
```

## Architecture

Bund BlobStore is organized into several core modules:

```
bund_blobstore/
├── blobstore/          # Core key-value store with ACID compliance
├── search/             # Full-text search and indexing
├── vector/             # Vector embeddings and similarity search
├── timeline/           # Time-series telemetry storage
├── data_distribution/  # Sharding and distribution strategies
├── distributed_graph/  # Graph storage and algorithms
├── common/             # Utilities and helpers
│   ├── grok_integration/    # Grok pattern parsing
│   ├── log_ingestor/        # Log ingestion pipeline
│   ├── log_worker_pool/     # Multi-threaded processing
│   ├── root_cause_analyzer/ # Event correlation analysis
│   └── embeddings/          # Vector embedding generation
└── chunked_document/   # Document chunking for RAG
```

## Documentation

Complete documentation is available in the `Documentation/` directory:

### Core Documentation
- [**BlobStore Documentation**](Documentation/BLOBSTORE.md) - Core key-value store features and usage
- [**Search Documentation**](Documentation/SEARCH.md) - Full-text search and indexing
- [**Vector Documentation**](Documentation/VECTOR.md) - Vector embeddings and similarity search
- [**Timeline Documentation**](Documentation/TIMELINE.md) - Telemetry timeline and time-series data

### Advanced Features
- [**Data Distribution**](Documentation/DATA_DISTRIBUTION.md) - Sharding strategies and distribution management
- [**Chunked Documents**](Documentation/CHUNKED_DOCUMENT.md) - Document chunking for RAG applications
- [**Distributed Graphs**](Documentation/DISTRIBUTED_GRAPH.md) - Graph storage and algorithms

### Common Utilities
- [**Grok Integration**](Documentation/GROK.md) - Log parsing with Grok patterns
- [**Log Ingestor**](Documentation/LOG_INGESTOR.md) - Log ingestion with deduplication
- [**Log Worker Pool**](Documentation/LOG_WORKER_POOL.md) - Multi-threaded log processing
- [**Root Cause Analysis**](Documentation/ROOT_CAUSE_ANALYSIS.md) - Event correlation and RCA

### Examples
- [**Hybrid Search Demo**](examples/hybrid_search_demo.rs) - Full-text + vector search
- [**Deduplication Demo**](examples/deduplication_demo.rs) - Duplicate detection and removal
- [**Root Cause Analysis Demo**](examples/root_cause_analysis_demo.rs) - Event correlation analysis

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["full"] }
```

### Feature Flags

- `full` - Enable all features (default)
- `fastembed` - Enable vector embeddings with FastEmbed
- `grok` - Enable Grok pattern parsing
- `reqwest` - Enable HTTP downloads for log ingestion
- `tokio` - Enable async support

## Usage Examples

### Document Storage with Hybrid Search

```rust
use bund_blobstore::data_distribution::DataDistributionManager;
use bund_blobstore::common::embeddings::EmbeddingGenerator;

let manager = DataDistributionManager::new("./data", DistributionStrategy::RoundRobin)?;
let embedder = EmbeddingGenerator::new()?;

// Store document chunks with embeddings
for (i, chunk) in document_chunks.iter().enumerate() {
    let embedding = embedder.embed(chunk)?;
    let doc = StoredDocument { id: format!("doc_{}", i), content: chunk, embedding };
    manager.put(&doc.id, &serde_json::to_vec(&doc)?, None)?;
}

// Hybrid search
let query_embedding = embedder.embed("search query")?;
let fts_results = full_text_search(&manager, "query")?;
let vector_results = vector_search(&manager, &query_embedding, 10)?;
let hybrid_results = combine_results(&fts_results, &vector_results);
```

### Log Ingestion with Deduplication

```rust
use bund_blobstore::common::{LogIngestor, LogIngestionConfig, GrokLogParser};

let grok = GrokLogParser::new("my_app");
let config = LogIngestionConfig {
    enable_deduplication: true,
    enable_embedding: true,
    enable_similarity_matching: true,
    ..Default::default()
};

let ingestor = LogIngestor::new(manager, grok, config);
let stats = ingestor.ingest_log_file("app.log", "application")?;

println!("Stored {} records, filtered {} duplicates", 
         stats.total_records_stored, stats.duplicates_filtered);
```

### Root Cause Analysis

```rust
use bund_blobstore::common::root_cause_analyzer::{RootCauseAnalyzer, RCAConfig};

let analyzer = RootCauseAnalyzer::new(manager, RCAConfig::default());
let result = analyzer.analyze_time_range(start_time, end_time, None)?;

println!("Root causes: {:?}", result.root_events);
for link in &result.causal_links {
    println!("{} → {} (confidence: {:.1}%)", 
             link.cause_event, link.effect_event, link.confidence * 100.0);
}

// Generate JSON report
let json_report = analyzer.generate_json_report(&result)?;
std::fs::write("rca_report.json", json_report)?;
```

## Performance

Bund BlobStore is designed for high performance:

- **Read Throughput**: 100,000+ ops/second (depending on data size)
- **Write Throughput**: 50,000+ ops/second with batching
- **Search Latency**: <10ms for FTS, <20ms for vector search (1M vectors)
- **Vector Similarity**: 95% accuracy with HNSW indexing

Benchmarks are available in the `benches/` directory.

## Use Cases

### RAG Applications
- Document chunking and embedding storage
- Hybrid search for context retrieval
- Metadata filtering and ranking

### Observability Platforms
- Telemetry timeline storage
- Log aggregation and analysis
- Root cause detection

### Search Engines
- Full-text search with relevance scoring
- Vector similarity for semantic search
- Hybrid ranking algorithms

### Time-Series Analytics
- Metric storage and aggregation
- Event correlation and pattern detection
- Anomaly detection

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgements

Bund BlobStore builds on several excellent open-source crates:
- [fastembed](https://crates.io/crates/fastembed) - Vector embeddings
- [grok](https://crates.io/crates/grok) - Pattern matching
- [tantivy](https://crates.io/crates/tantivy) - Full-text search (optional)
- [petgraph](https://crates.io/crates/petgraph) - Graph algorithms

## Support

- **Documentation**: See the [Documentation/](Documentation/) directory
- **Examples**: Run examples in the `examples/` directory
- **Issues**: Report bugs on [GitHub Issues](https://github.com/vulogov/bund_blobstore/issues)

## Version History

### Version 0.11.0
- Added chunked document storage for RAG applications
- Integrated FastEmbed for vector embeddings
- Added root cause analysis module
- Enhanced log ingestion with worker pool
- Added hybrid search capabilities

### Version 0.10.0
- Added data distribution manager
- Implemented dynamic sharding
- Added distributed graph storage

### Version 0.9.0
- Added telemetry timeline
- Implemented vector similarity search
- Added Grok pattern integration

## Roadmap

- [ ] Multi-node replication
- [ ] Backup and restore utilities
- [ ] WebAssembly support
- [ ] More embedding models
- [ ] Advanced graph algorithms
- [ ] Time-series forecasting

---

For detailed API documentation, run `cargo doc --open` after adding the dependency.
```
