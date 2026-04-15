# Bund BlobStore

[](https://www.rust-lang.org)
[](https://www.google.com/search?q=LICENSE)

A **high-performance, ACID-compliant embedded database** designed for the next generation of RAG (Retrieval-Augmented Generation), Observability, and Distributed Computing. Built on the speed of RedB, Bund BlobStore provides a unified interface for KV-storage, vector similarity, and complex telemetry.

## 📚 Documentation

| Feature | Documentation |
|---------|---------------|
| **Data Distribution** | [Documentation/DATA\_DISTRIBUTION.md](https://www.google.com/search?q=Documentation/DATA_DISTRIBUTION.md) - Sharding, load balancing, and shard management |
| **Virtual Filesystem** | [Documentation/VIRTUAL\_FILESYSTEM.md](https://www.google.com/search?q=Documentation/VIRTUAL_FILESYSTEM.md) - Hierarchical file storage over KV shards |
| **Multidimensional Storage** | [Documentation/MULTIDIMENSIONAL\_STORAGE.md](https://www.google.com/search?q=Documentation/MULTIDIMENSIONAL_STORAGE.md) - 1D, 2D, and 3D telemetry storage |
| **Chunked Document Storage** | [Documentation/CHUNKED\_DOCUMENT\_STORAGE.md](https://www.google.com/search?q=Documentation/CHUNKED_DOCUMENT_STORAGE.md) - RAG-ready document processing |
| **Distributed Graph Algorithms** | [Documentation/DISTRIBUTED\_GRAPH.md](https://www.google.com/search?q=Documentation/DISTRIBUTED_GRAPH.md) - Cross-shard graph operations |
| **Vector & Hybrid Search** | [Documentation/SEARCH.md](https://www.google.com/search?q=Documentation/SEARCH.md) - Vector similarity and hybrid search |
| **Telemetry Timeline** | [Documentation/TELEMETRY.md](https://www.google.com/search?q=Documentation/TELEMETRY.md) - Time series data and event storage |

## ✨ Enterprise Features

### ⚖️ DataDistributionManager & Intelligent Sharding

The backbone of Bund's scalability, managing data across multiple physical shards.

  - **Multiple Distribution Strategies**: Round-Robin, Time-Bucket, Key-Similarity, and Adaptive load balancing.
  - **Dynamic Shard Management**: Add or remove shards at runtime without stopping the system.
  - **Adaptive Load Balancing**: Automatically monitors shard entropy and load scores to optimize data placement.
  - **Persistence Lifecycle**: Complete management of shard initialization, synchronization, and graceful shutdown.

### 📄 Sharded Text & RAG Intelligence

Optimized for Large Language Model (LLM) workflows and massive text processing.

  - **Advanced Chunking**: Split documents at sentence/paragraph boundaries with configurable context overlaps.
  - **Distributed Text Storage**: Large documents are sharded across the cluster to prevent hot spots.
  - **Hybrid Search**: Unified querying combining TF-IDF keyword relevance with semantic vector similarity.
  - **Multi-Language Support**: Built-in stemming for 8+ languages for high-precision retrieval.

### 📂 Virtual Filesystem (VFS) & Document Analysis

A logical organization layer that provides a hierarchical view of sharded data.

  - **Hierarchical Path Resolution**: Standard `/root/dir/file` semantics mapped over distributed KV pairs.
  - **JSON Fingerprinting**: Automatic generation of unique fingerprints and schema versioning for JSON documents.
  - **Code Analysis**: Built-in parsers for source code to extract function names, imports, and line counts.
  - **Safety Guards**: Enforces path absolute integrity and prevents file-as-directory collisions.

### 📐 Multidimensional Telemetry & IoT

Beyond simple time-series, Bund supports coordinate-aware storage.

  - **Volumetric Data**: Store samples in 1D, 2D, or 3D coordinate spaces (perfect for digital twins or voxel data).
  - **Fixed-Size FIFO Queues**: Automatic eviction of oldest samples per dimension ensures a constant storage footprint.
  - **Vector-Telemetry Integration**: Combine temporal proximity with semantic similarity to identify pattern anomalies.

### 🕸️ Distributed Graph Engine

Store and analyze complex relationships across multiple physical shards.

  - **Parallel Algorithms**: Cycle detection and shortest-path (Dijkstra) operations optimized for sharded environments.
  - **Bidirectional Traversal**: Accelerated pathfinding for massive, sparse graphs.

-----

## 🚀 Usage Examples

### Intelligent Data Distribution

```rust
let manager = DataDistributionManager::new("./storage_path", DistributionStrategy::Adaptive(config))?;

// Store data - the manager automatically selects the optimal shard
manager.put("important_key", b"some_data", Some("namespace"))?;
```

### JSON Fingerprinting via VFS

```rust
// Stores JSON and automatically generates metadata/fingerprints
vfs.mkjson(
    "/configs/cluster_cfg.json",
    "doc_uuid_123",
    "sha256_fingerprint",
    "v1.2.0", // Schema version
    2048      // Size in bytes
)?;
```

### Hybrid Search (RAG)

```rust
// Perform hybrid search (Vector Similarity + Keyword Matching)
let results = manager.search_advanced_chunks(
    "How does the adaptive sharding strategy balance load?", 
    5,    // Limit
    0.7,  // Min similarity
    true  // Use hybrid mode
)?;
```

-----

## 📊 Performance Benchmarks

| Operation | Throughput/Latency |
|-----------|-------------------|
| Write throughput | \~50,000 ops/second |
| Read throughput | \~100,000 ops/second |
| VFS Path Resolution | \< 1ms |
| Multidimensional storage | \< 10ms per sample |
| Vector search | \< 50ms for 10K vectors |
| Load balance score | \> 0.7 (Adaptive Strategy) |

## 🧪 Testing

Bund BlobStore maintains a rigorous test suite for distributed invariants:

```bash
# Run the full suite including VFS and Sharding tests
cargo test
```

## 📦 Installation

```toml
[dependencies]
bund_blobstore = "0.14.0"
```

-----

**Built with ❤️ for the Distributed Future**
