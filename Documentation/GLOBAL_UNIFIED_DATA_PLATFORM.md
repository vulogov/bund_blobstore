# GLOBAL_UNIFIED_DATA_PLATFORM.md

## Overview

This document provides a detailed explanation of the `global_unified_data_platform.rs` example, which demonstrates a comprehensive unified data platform using a single `DataDistributionManager` instance to coordinate seven different storage types. The example showcases how to build an enterprise-grade data platform with ACID compliance, automatic data distribution, and intelligent storage management.

## Table of Contents

- [Architecture](#architecture)
- [Storage Types](#storage-types)
  - [1. Multidimensional Telemetry Storage](#1-multidimensional-telemetry-storage)
  - [2. Time Series Telemetry Storage](#2-time-series-telemetry-storage)
  - [3. Full-Text Search Storage](#3-full-text-search-storage)
  - [4. Vector Similarity Search](#4-vector-similarity-search)
  - [5. Graph Storage](#5-graph-storage)
  - [6. Log Storage with Primary/Secondary Separation](#6-log-storage-with-primarysecondary-separation)
  - [7. Binary Blob Storage](#7-binary-blob-storage)
- [Distribution Strategies](#distribution-strategies)
- [Performance Considerations](#performance-considerations)
- [Best Practices](#best-practices)
- [Integration Patterns](#integration-patterns)
- [Troubleshooting](#troubleshooting)
- [Complete Example Output](#complete-example-output)

## Architecture

### Core Concept: Single Manager Pattern

The example uses a **single `DataDistributionManager` instance** that coordinates all storage operations. This pattern provides:

- **Centralized Configuration**: One place to configure distribution strategies
- **Unified Sharding**: All data types share the same sharding infrastructure
- **Consistent ACID Compliance**: Transactions span across all storage types
- **Resource Efficiency**: Single point of coordination reduces overhead
- **Simplified Management**: One manager to monitor and maintain

For detailed documentation on the DataDistributionManager, see:
- [Data Distribution Documentation](Documentation/DATA_DISTRIBUTION.md)
- [Dynamic Shard Management](Documentation/DYNAMIC_SHARDING.md)

### Global Initialization with lazy_static

```rust
lazy_static! {
    static ref MANAGER: Arc<DataDistributionManager> = {
        let manager = DataDistributionManager::new(
            "./unified_data_store",
            DistributionStrategy::RoundRobin,
        ).expect("Failed to create DataDistributionManager");
        
        Arc::new(manager)
    };
}
```

**Why lazy_static?**
- Thread-safe global initialization
- Single instance throughout application lifecycle
- Lazy initialization (created on first access)
- Zero-cost abstraction

## Storage Types

### 1. Multidimensional Telemetry Storage

**Purpose**: Store telemetry data in 1D, 2D, and 3D coordinate spaces with FIFO queues.

**Key Features**:
- Fixed-size FIFO queues per coordinate
- Automatic eviction of oldest samples
- Time-range queries
- Metadata preservation

**Usage Example**:
```rust
// Create dimensions
MULTIDIM_STORAGE.create_dimension("sensors_1d", DimensionType::OneD, 1000, None)?;
MULTIDIM_STORAGE.create_dimension("grid_2d", DimensionType::TwoD, 500, Some(bounds_2d))?;
MULTIDIM_STORAGE.create_dimension("voxels_3d", DimensionType::ThreeD, 1000, None)?;

// Store 1D data
let coord = Coordinate::OneD(Coord1D(sensor_id));
MULTIDIM_STORAGE.push_sample(
    "sensors_1d",
    coord,
    TelemetryValue::Float(temp_value),
    Some(Utc::now()),
    metadata,
)?;

// Query data
let samples = MULTIDIM_STORAGE.get_latest_samples(
    "sensors_1d", 
    Coordinate::OneD(Coord1D(5)), 
    5
)?;
```

**Use Cases**:
- **1D**: IoT sensor arrays, temperature monitoring along a pipeline
- **2D**: Grid-based environmental monitoring, pressure maps
- **3D**: Volumetric data, 3D scanning, spatial analysis

**For complete documentation, see:** [Multidimensional Storage Documentation](Documentation/MULTIDIMENSIONAL_STORAGE.md)

### 2. Time Series Telemetry Storage

**Purpose**: Store time-series data with efficient querying and aggregation.

**Key Features**:
- Automatic timestamp indexing
- Time-interval queries (last hour, day, week, month)
- Minute-grade bucketing
- Primary-secondary record relationships

**Usage Example**:
```rust
let cpu_record = TelemetryRecord::new_primary(
    format!("server_{}", i % 3),
    timestamp,
    "cpu_usage".to_string(),
    "production".to_string(),
    TelemetryValue::Float(cpu_usage),
);
TELEMETRY_STORE.store(cpu_record)?;

let query = TelemetryQuery {
    time_interval: Some(TimeInterval::last_hour()),
    keys: Some(vec!["cpu_usage".to_string()]),
    limit: 10,
    ..Default::default()
};
let results = TELEMETRY_STORE.query(&query)?;
```

**Use Cases**:
- Server monitoring (CPU, memory, disk)
- Application performance metrics
- Business KPIs tracking
- Industrial sensor data logging

**For complete documentation, see:** [Telemetry Timeline Documentation](Documentation/TELEMETRY.md)

### 3. Full-Text Search Storage

**Purpose**: Index and search text documents with relevance scoring.

**Key Features**:
- Inverted index with TF-IDF scoring
- Fuzzy search algorithms
- Phrase matching
- Proximity search

**Usage Example**:
```rust
SEARCH_STORE.put_text(
    "doc1",
    "The quick brown fox jumps over the lazy dog",
    Some("animals"),
)?;

let results = SEARCH_STORE.search("programming language", 10)?;
for result in results {
    println!("{} (score: {:.3})", result.key, result.score);
}
```

**Use Cases**:
- Document management systems
- Knowledge bases
- Log analysis
- Content search engines

**For complete documentation, see:** [Search Capabilities Documentation](Documentation/SEARCH.md)

### 4. Vector Similarity Search

**Purpose**: Perform semantic similarity search using embeddings.

**Key Features**:
- State-of-the-art embeddings (fastembed)
- Semantic similarity scoring
- Hybrid search combining vector + keyword
- Multi-modal embeddings

**Usage Example**:
```rust
VECTOR_STORE.insert_text(
    "vec1",
    "Artificial intelligence and deep learning",
    Some("ai"),
)?;

let results = VECTOR_STORE.search_similar("neural networks", 5)?;
for result in results {
    println!("{} (similarity: {:.3})", result.key, result.score);
}
```

**Use Cases**:
- Recommendation systems
- Duplicate detection
- Semantic search
- Question answering systems

**For complete documentation, see:** [Vector Search Documentation](Documentation/SEARCH.md#vector-search)

### 5. Graph Storage

**Purpose**: Store and query graph data structures with path finding.

**Key Features**:
- Node and edge storage
- Weighted relationships
- Shortest path algorithms (BFS/Dijkstra)
- Relationship-based queries

**Usage Example**:
```rust
// Add nodes
let node = GraphNode {
    id: "A".to_string(),
    label: "Node A".to_string(),
    properties: HashMap::new(),
};
GRAPH_STORE.add_node(node)?;

// Add edges
let edge = GraphEdge {
    from: "A".to_string(),
    to: "B".to_string(),
    weight: 1.0,
    relationship: "connects".to_string(),
};
GRAPH_STORE.add_edge(edge)?;

// Find shortest path
if let Some(path) = GRAPH_STORE.find_shortest_path("A", "E")? {
    println!("Shortest path: {:?}", path);
}
```

**Use Cases**:
- Social networks
- Dependency graphs
- Route planning
- Network topology

**For complete documentation, see:** [Distributed Graph Documentation](Documentation/DISTRIBUTED_GRAPH.md)

### 6. Log Storage with Primary/Secondary Separation

**Purpose**: Store logs with automatic classification and priority-based separation.

**Key Features**:
- Automatic log classification (primary/secondary)
- Priority-based routing
- Separate storage for high-priority logs
- Faster querying for critical logs

**Classification Rules**:
```rust
fn is_primary_log(&self, log: &LogEntry) -> bool {
    let critical_services = vec!["database", "payment-processor", "auth-service"];
    
    log.primary 
        || matches!(log.level, LogLevel::Error | LogLevel::Critical)
        || critical_services.contains(&log.service.as_str())
}
```

**Usage Example**:
```rust
// Logs are automatically classified
let log = LogEntry {
    timestamp: Utc::now(),
    level: LogLevel::Error,
    service: "database".to_string(),
    message: "Connection pool exhausted".to_string(),
    metadata: HashMap::new(),
    correlation_id: Some("corr_123".to_string()),
    primary: false,  // Will go to primary due to Error level
};
LOG_STORE.ingest(log)?;

// Query primary logs only (high priority)
let primary_logs = LOG_STORE.get_primary_logs(10)?;

// Get statistics
let (primary_count, secondary_count) = LOG_STORE.get_primary_secondary_stats()?;
```

**Use Cases**:
- Production monitoring
- Security audit logging
- Compliance logging
- Debugging with priority separation

### 7. Binary Blob Storage

**Purpose**: Store arbitrary binary data with metadata.

**Key Features**:
- Arbitrary binary data storage
- Category-based organization
- Metadata tracking
- Efficient retrieval

**Usage Example**:
```rust
let config_data = br#"{"version": "1.0", "environment": "production"}"#;
BLOB_STORE.put("config.json", config_data, Some("configs"))?;

if let Some(data) = BLOB_STORE.get("config.json")? {
    println!("Retrieved {} bytes", data.len());
}
```

**Use Cases**:
- Configuration files
- User uploads
- Binary assets
- Serialized objects

## Distribution Strategies

The `DataDistributionManager` supports multiple distribution strategies:

| Strategy | Description | Best For |
|----------|-------------|----------|
| `RoundRobin` | Evenly distributes across shards | High-throughput writes |
| `TimeBucket` | Groups by time intervals | Time-series data |
| `KeySimilarity` | Keeps similar keys together | Graph data, related items |
| `Adaptive` | Dynamically balances based on load | Variable workloads |

**For detailed documentation, see:** [Data Distribution Strategies](Documentation/DATA_DISTRIBUTION.md#distribution-strategies)

## Performance Considerations

### Write Throughput
- **Blob Store**: ~50,000 ops/second
- **Search Store**: ~10,000 docs/second
- **Vector Store**: ~5,000 embeddings/second
- **Telemetry Store**: ~100,000 samples/second
- **Multidimensional**: ~10,000 samples/second
- **Graph Store**: ~20,000 nodes/second
- **Log Store**: ~50,000 entries/second

### Read Performance
- **Point queries**: <1ms
- **Range queries**: <50ms for 10K items
- **Search queries**: <100ms
- **Vector search**: <50ms for 10K vectors
- **Path finding**: O(V+E) with optimization

**For performance benchmarks, see:** [Performance Documentation](Documentation/PERFORMANCE.md)

## Best Practices

### 1. Manager Configuration
```rust
// Use Adaptive strategy for mixed workloads
let manager = DataDistributionManager::new(
    "./data",
    DistributionStrategy::Adaptive(AdaptiveConfig::default())
)?;
```

### 2. Dimension Sizing
```rust
// Estimate capacity based on retention needs
// 1 sample/second * 3600 seconds = 3600 samples/hour
let capacity = 3600 * 24; // 24 hours of data
MULTIDIM_STORAGE.create_dimension("sensors", DimensionType::OneD, capacity, None)?;
```

**For dimension management, see:** [Dimension Configuration](Documentation/MULTIDIMENSIONAL_STORAGE.md#dimension-management)

### 3. Log Classification
```rust
// Define critical services for your domain
let critical_services = vec![
    "payment-gateway",
    "user-auth",
    "database-primary",
    "cache-layer"
];
```

### 4. Error Handling
```rust
// Always handle storage errors gracefully
match BLOB_STORE.put("key", data, None) {
    Ok(_) => println!("Stored successfully"),
    Err(e) => eprintln!("Storage failed: {}", e),
}
```

### 5. Batch Operations
```rust
// Batch related operations in a transaction
let transaction = MANAGER.begin_transaction()?;
for item in batch_items {
    transaction.put(&item.key, &item.value, None)?;
}
transaction.commit()?;
```

**For transaction documentation, see:** [ACID Compliance](Documentation/ACID.md)

## Integration Patterns

### Pattern 1: Event Sourcing
```rust
// Store events with correlation
let event = LogEntry {
    timestamp: Utc::now(),
    correlation_id: Some(transaction_id),
    // ... other fields
};
LOG_STORE.ingest(event)?;
```

### Pattern 2: CQRS (Command Query Responsibility Segregation)
```rust
// Command side (write)
BLOB_STORE.put("aggregate_id", command_data, None)?;

// Query side (read)
let results = SEARCH_STORE.search("query", 10)?;
```

### Pattern 3: Materialized Views
```rust
// Store pre-computed aggregates
let hourly_avg = compute_average(samples);
TELEMETRY_STORE.store(TelemetryRecord::new_primary(
    "aggregate", timestamp, "hourly_avg", "metrics", 
    TelemetryValue::Float(hourly_avg)
))?;
```

**For advanced integration patterns, see:** [Integration Guide](Documentation/INTEGRATION.md)

## Troubleshooting

### Common Issues and Solutions

| Issue | Symptom | Solution |
|-------|---------|----------|
| High latency | Slow queries | Check shard balance, increase shard count |
| Write failures | `Storage full` | Increase capacity, implement retention |
| Search relevance | Poor results | Adjust TF-IDF, add more documents |
| Vector similarity | Low accuracy | Fine-tune embeddings, increase dimensions |

**For troubleshooting guides, see:** [Troubleshooting Documentation](Documentation/TROUBLESHOOTING.md)

## Complete Example Output

When running the demo, you'll see:

```
📊 MULTIDIMENSIONAL TELEMETRY
==============================
✓ Created 1D, 2D, and 3D dimensions
  [1D] Linear Sensor Array: 10 readings
  [2D] Pressure Grid: 25 points
  [3D] Voxel Space: 27 samples

📈 TELEMETRY TIME SERIES
========================
✓ Stored 20 telemetry samples
✓ Retrieved 10 CPU usage records

🔍 SEARCH & VECTOR STORAGE
==========================
✓ Found 2 search results
✓ Found 2 similar vectors

🕸️ GRAPH STORAGE
================
✓ Shortest path from A to E: ["A", "D", "E"]

📝 LOG STORAGE WITH PRIMARY/SECONDARY SEPARATION
================================================
✓ Ingested 7 logs
📊 Primary: 5 logs, Secondary: 2 logs
```

## Additional Resources

### Core Documentation
- [README.md](../README.md) - Project overview and quick start
- [Multidimensional Storage](Documentation/MULTIDIMENSIONAL_STORAGE.md) - Complete guide to 1D, 2D, and 3D storage
- [Chunked Document Storage](Documentation/CHUNKED_DOCUMENT_STORAGE.md) - RAG-ready document processing
- [Close & Sync Operations](Documentation/CLOSE_SYNC.md) - Cache management and synchronization
- [Distributed Graph](Documentation/DISTRIBUTED_GRAPH.md) - Cross-shard graph operations
- [Search Capabilities](Documentation/SEARCH.md) - Full-text, fuzzy, and vector search
- [Telemetry Timeline](Documentation/TELEMETRY.md) - Time series data and event storage
- [Data Distribution](Documentation/DATA_DISTRIBUTION.md) - Sharding and load balancing

### API Reference
- [DataDistributionManager API](../src/data_distribution.rs)
- [MultidimensionalStorage API](../src/common/multidimensional_storage.rs)
- [TelemetryStore API](../src/timeline.rs)
- [SearchableBlobStore API](../src/search.rs)
- [VectorStore API](../src/vector.rs)

## Conclusion

The `global_unified_data_platform.rs` example demonstrates a production-ready unified data platform that:

- **Scales horizontally** with automatic sharding
- **Maintains ACID compliance** across all operations
- **Provides 7 storage types** for different data models
- **Uses a single manager** for coordination
- **Includes monitoring** and observability features
- **Handles errors gracefully** with proper propagation

This architecture is suitable for:
- Microservices data layer
- IoT data platforms
- Analytics pipelines
- Content management systems
- Observability platforms

**For the complete example code, see:** [examples/global_unified_data_platform.rs](../examples/global_unified_data_platform.rs)
