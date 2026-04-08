```markdown
# Data Distribution Manager Documentation

## Overview

The `DataDistributionManager` provides intelligent data distribution, sharding, and caching for the Bund BlobStore. It handles automatic data distribution across multiple shards, implements LRU caching with TTL, and supports various distribution strategies for optimal data placement.

## Features

- **Multiple Distribution Strategies** - Round-robin, time-bucket, key similarity, and adaptive distribution
- **Dynamic Shard Management** - Automatic shard creation and rebalancing
- **Intelligent Caching** - LRU cache with configurable TTL and size limits
- **Telemetry Storage** - Optimized storage for telemetry data with vector embeddings
- **Shard Statistics** - Comprehensive per-shard and global statistics
- **Thread-Safe Operations** - Concurrent read/write access with fine-grained locking
- **Hot/Cold Data Separation** - Automatic promotion/demotion of shards based on access patterns
- **Query Optimization** - Intelligent query routing to appropriate shards

## Quick Start

```rust
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use std::path::PathBuf;

// Create a new distribution manager
let manager = DataDistributionManager::new(
    PathBuf::from("/data/bund"),
    DistributionStrategy::RoundRobin,
)?;

// Store data
manager.put("user:123", b"John Doe", None)?;

// Retrieve data
if let Some(data) = manager.get("user:123")? {
    println!("Retrieved: {}", String::from_utf8_lossy(&data));
}

// Get statistics
let stats = manager.get_stats();
println!("Total shards: {}", stats.total_shards);
println!("Total records: {}", stats.total_records);
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["data-distribution"] }
```

## Core Components

### DistributionStrategy

The strategy for distributing data across shards:

```rust
pub enum DistributionStrategy {
    RoundRobin,                    // Simple round-robin distribution
    TimeBucket(TimeBucketConfig),  // Time-based bucketing
    KeySimilarity(SimilarityConfig), // Similarity-based grouping
    Adaptive(AdaptiveConfig),      // Adaptive strategy based on access patterns
}
```

### TimeBucketConfig

Configuration for time-based bucketing:

```rust
pub struct TimeBucketConfig {
    pub bucket_size_seconds: i64,  // Size of each time bucket
    pub retention_days: u32,       // How long to retain data
    pub hot_bucket_count: usize,   // Number of recent buckets to keep hot
}
```

### SimilarityConfig

Configuration for similarity-based distribution:

```rust
pub struct SimilarityConfig {
    pub threshold: f32,            // Similarity threshold (0.0 to 1.0)
    pub max_group_size: usize,     // Maximum records per similarity group
    pub use_embeddings: bool,      // Use vector embeddings for similarity
}
```

### AdaptiveConfig

Configuration for adaptive distribution:

```rust
pub struct AdaptiveConfig {
    pub learning_rate: f32,        // How quickly to adapt (0.0 to 1.0)
    pub sample_window_seconds: u64, // Window for access pattern sampling
    pub min_shards: usize,         // Minimum number of shards
    pub max_shards: usize,         // Maximum number of shards
}
```

### ShardStats

Statistics for individual shards:

```rust
pub struct ShardStats {
    pub shard_id: String,
    pub record_count: usize,
    pub size_bytes: u64,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
    pub hit_rate: f64,
}
```

### DistributionStats

Global statistics:

```rust
pub struct DistributionStats {
    pub total_shards: usize,
    pub total_records: usize,
    pub total_size_bytes: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub cache_hit_rate: f64,
    pub active_operations: usize,
}
```

## Usage Examples

### 1. Round-Robin Distribution

```rust
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};

let manager = DataDistributionManager::new(
    "/data/bund",
    DistributionStrategy::RoundRobin,
)?;

// Data is evenly distributed across shards
for i in 0..1000 {
    let key = format!("record_{}", i);
    manager.put(&key, b"data", None)?;
}

let stats = manager.get_stats();
println!("Distribution across {} shards", stats.total_shards);
```

### 2. Time-Bucket Distribution

```rust
use bund_blobstore::data_distribution::{
    DataDistributionManager, DistributionStrategy, TimeBucketConfig
};

let time_config = TimeBucketConfig {
    bucket_size_seconds: 3600,     // 1-hour buckets
    retention_days: 30,             // Keep 30 days of data
    hot_bucket_count: 24,           // Keep 24 hours hot in cache
};

let manager = DataDistributionManager::new(
    "/data/bund",
    DistributionStrategy::TimeBucket(time_config),
)?;

// Store telemetry data with timestamps
let now = Utc::now();
manager.put_telemetry_with_timestamp("cpu_usage", 85.5, now)?;
manager.put_telemetry_with_timestamp("memory_usage", 4096, now)?;

// Query recent data (automatically routed to hot shards)
let results = manager.query_time_range("cpu_usage", now - Duration::hours(1), now)?;
```

### 3. Similarity-Based Distribution

```rust
use bund_blobstore::data_distribution::{
    DataDistributionManager, DistributionStrategy, SimilarityConfig
};

let similarity_config = SimilarityConfig {
    threshold: 0.85,                // 85% similarity threshold
    max_group_size: 10000,          // Max 10k records per group
    use_embeddings: true,           // Use vector embeddings
};

let manager = DataDistributionManager::new(
    "/data/bund",
    DistributionStrategy::KeySimilarity(similarity_config),
)?;

// Similar keys are grouped together
manager.put("error:database:timeout", b"error1", None)?;
manager.put("error:database:connection", b"error2", None)?;  // Similar to above
manager.put("info:user:login", b"info1", None)?;            // Different group
```

### 4. Adaptive Distribution

```rust
use bund_blobstore::data_distribution::{
    DataDistributionManager, DistributionStrategy, AdaptiveConfig
};

let adaptive_config = AdaptiveConfig {
    learning_rate: 0.1,             // 10% adaptation rate
    sample_window_seconds: 3600,    // Learn from last hour
    min_shards: 4,                  // Minimum 4 shards
    max_shards: 64,                 // Maximum 64 shards
};

let manager = DataDistributionManager::new(
    "/data/bund",
    DistributionStrategy::Adaptive(adaptive_config),
)?;

// Manager automatically adapts to access patterns
for i in 0..10000 {
    let key = format!("hot_key_{}", i % 100);  // Only 100 hot keys
    manager.put(&key, b"data", None)?;
}

// Frequently accessed keys are moved to faster shards
let stats = manager.get_stats();
println!("Adaptive sharding created {} shards", stats.total_shards);
```

### 5. Working with Telemetry Data

```rust
use bund_blobstore::timeline::TelemetryRecord;
use bund_blobstore::data_distribution::DataDistributionManager;

let manager = DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin)?;

// Store telemetry record with vector embedding
let record = TelemetryRecord {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp_seconds: Utc::now().timestamp(),
    key: "cpu_temperature".to_string(),
    source: "sensor_1".to_string(),
    value: TelemetryValue::Float(72.5),
    metadata: HashMap::new(),
    is_primary: true,
    primary_id: None,
    secondary_ids: vec![],
};

let embedding = vec![0.1, 0.2, 0.3, 0.4]; // Vector embedding
manager.put_telemetry_with_vector(record, &embedding)?;

// Query similar telemetry
let similar = manager.find_similar_telemetry(&embedding, 0.85)?;
println!("Found {} similar records", similar.len());
```

### 6. Cache Management

```rust
// Configure cache size and TTL
let manager = DataDistributionManager::with_cache_config(
    "/data/bund",
    DistributionStrategy::RoundRobin,
    10000,          // Max 10,000 items in cache
    3600,           // 1-hour TTL
)?;

// Cache is automatically managed
manager.put("frequent_key", b"data", None)?;

// First access (cache miss)
let data = manager.get("frequent_key")?;

// Second access (cache hit - faster)
let data = manager.get("frequent_key")?;

let stats = manager.get_stats();
println!("Cache hit rate: {:.2}%", stats.cache_hit_rate * 100.0);
```

### 7. Batch Operations

```rust
let manager = DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin)?;

// Batch put operation
let mut batch = Vec::new();
for i in 0..1000 {
    batch.push((format!("key_{}", i), vec![i as u8]));
}
manager.batch_put(batch)?;

// Batch get operation
let keys: Vec<String> = (0..100).map(|i| format!("key_{}", i)).collect();
let results = manager.batch_get(&keys)?;

for (key, data) in results {
    if let Some(data) = data {
        println!("Found {}: {} bytes", key, data.len());
    }
}
```

### 8. Shard Management

```rust
let manager = DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin)?;

// Get all shards
let shards = manager.list_shards()?;
println!("Active shards: {:?}", shards);

// Get shard statistics
for shard_id in &shards {
    let stats = manager.get_shard_stats(shard_id)?;
    println!("Shard {}: {} records, {} bytes", 
             shard_id, stats.record_count, stats.size_bytes);
}

// Compact a shard (removes tombstones)
manager.compact_shard("shard_0")?;

// Rebalance shards (redistributes data)
manager.rebalance_shards()?;
```

### 9. Query Optimization

```rust
let manager = DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin)?;

// Range query with timestamp
let start_time = Utc::now() - Duration::days(7);
let end_time = Utc::now();
let results = manager.query_time_range("metric_name", start_time, end_time)?;

// Prefix scan
let results = manager.scan_prefix("user:")?;

// Filtered query
let results = manager.query_filtered(|key, _| {
    key.starts_with("error:") && key.contains("database")
})?;
```

### 10. Hot/Cold Data Separation

```rust
use bund_blobstore::data_distribution::{DataDistributionManager, TimeBucketConfig};

let config = TimeBucketConfig {
    bucket_size_seconds: 86400,     // Daily buckets
    retention_days: 90,              // Keep 90 days
    hot_bucket_count: 7,             // Last 7 days hot in cache
};

let manager = DataDistributionManager::new(
    "/data/bund",
    DistributionStrategy::TimeBucket(config),
)?;

// Store daily metrics
for day in 0..30 {
    let timestamp = Utc::now() - Duration::days(day);
    manager.put_telemetry_with_timestamp("daily_metric", day, timestamp)?;
}

// Recent data (last 7 days) is cached and fast
let recent = manager.query_time_range("daily_metric", 
    Utc::now() - Duration::days(7), Utc::now())?;

// Older data is still available but may be slower
let old = manager.query_time_range("daily_metric", 
    Utc::now() - Duration::days(30), Utc::now() - Duration::days(8))?;
```

### 11. Custom Distribution Logic

```rust
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};

// Implement custom sharding function
fn custom_shard_fn(key: &str, total_shards: usize) -> usize {
    // Shard by key prefix
    match key.chars().next() {
        Some('a'..='m') => 0,
        Some('n'..='z') => 1,
        _ => 2,
    }
}

// Use with custom shard selection
let manager = DataDistributionManager::new_with_shard_fn(
    "/data/bund",
    custom_shard_fn,
)?;
```

### 12. Monitoring and Alerts

```rust
let manager = DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin)?;

// Set up monitoring thread
std::thread::spawn(move || loop {
    let stats = manager.get_stats();
    
    if stats.cache_hit_rate < 0.5 {
        println!("Warning: Low cache hit rate: {:.2}%", stats.cache_hit_rate * 100.0);
    }
    
    if stats.total_shards > 100 {
        println!("Warning: Too many shards: {}", stats.total_shards);
    }
    
    if stats.total_size_bytes > 1_000_000_000_000 {
        println!("Alert: Storage exceeded 1TB");
    }
    
    std::thread::sleep(std::time::Duration::from_secs(60));
});
```

## Performance Optimization

### Cache Configuration

```rust
// Small cache for memory-constrained environments
let manager = DataDistributionManager::with_cache_config(
    "/data/bund",
    DistributionStrategy::RoundRobin,
    1000,    // Small cache
    300,     // 5-minute TTL
)?;

// Large cache for high-performance needs
let manager = DataDistributionManager::with_cache_config(
    "/data/bund",
    DistributionStrategy::RoundRobin,
    1_000_000,  // 1M items
    86400,      // 24-hour TTL
)?;
```

### Shard Tuning

```rust
// For time-series data
let config = TimeBucketConfig {
    bucket_size_seconds: 3600,     // Hourly buckets
    retention_days: 30,             // 30-day retention
    hot_bucket_count: 24,           // Keep 24 hours hot
};

// For high-cardinality data
let config = AdaptiveConfig {
    learning_rate: 0.2,
    sample_window_seconds: 1800,    // 30-minute window
    min_shards: 16,                  // Start with 16 shards
    max_shards: 256,                 // Max 256 shards
};
```

## Error Handling

```rust
use bund_blobstore::data_distribution::DataDistributionManager;

match DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin) {
    Ok(manager) => {
        match manager.put("key", b"value", None) {
            Ok(_) => println!("Success"),
            Err(e) => eprintln!("Failed to store: {}", e),
        }
    }
    Err(e) => eprintln!("Failed to create manager: {}", e),
}
```

## Best Practices

1. **Strategy Selection**
   - Use `RoundRobin` for general-purpose distribution
   - Use `TimeBucket` for time-series telemetry data
   - Use `KeySimilarity` for grouped access patterns
   - Use `Adaptive` for unpredictable workloads

2. **Cache Sizing**
   - Monitor cache hit rates
   - Size cache for working set
   - Set TTL based on data volatility

3. **Shard Management**
   - Regularly monitor shard count
   - Rebalance when shards become imbalanced
   - Compact shards periodically

4. **Performance**
   - Batch operations for bulk loads
   - Use appropriate timeouts for queries
   - Leverage caching for frequently accessed data

## Troubleshooting

### Issue: Poor Cache Performance
**Solution**: Increase cache size or adjust TTL based on access patterns

### Issue: Uneven Shard Distribution
**Solution**: Rebalance shards or adjust distribution strategy

### Issue: Slow Query Performance
**Solution**: Use time-range queries, add indexes, or adjust sharding strategy

### Issue: High Memory Usage
**Solution**: Reduce cache size or decrease number of shards

## API Reference

### Core Methods
- `new<P: AsRef<Path>>(base_path: P, strategy: DistributionStrategy) -> Result<Self>`
- `put(&self, key: &str, data: &[u8], timestamp: Option<DateTime<Utc>>) -> Result<()>`
- `get(&self, key: &str) -> Result<Option<Vec<u8>>>`
- `delete(&self, key: &str) -> Result<bool>`
- `exists(&self, key: &str) -> Result<bool>`

### Telemetry Methods
- `put_telemetry(&self, record: TelemetryRecord) -> Result<()>`
- `put_telemetry_with_vector(&self, record: TelemetryRecord, embedding: &[f32]) -> Result<()>`
- `get_telemetry(&self, id: &str) -> Result<Option<TelemetryRecord>>`
- `find_similar_telemetry(&self, embedding: &[f32], threshold: f32) -> Result<Vec<TelemetryRecord>>`

### Query Methods
- `query_time_range(&self, key: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<TelemetryRecord>>`
- `scan_prefix(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>>`
- `query_filtered<F>(&self, filter: F) -> Result<Vec<(String, Vec<u8>)>> where F: Fn(&str, &[u8]) -> bool`

### Statistics Methods
- `get_stats(&self) -> DistributionStats`
- `get_shard_stats(&self, shard_id: &str) -> Result<ShardStats>`
- `list_shards(&self) -> Result<Vec<String>>`

### Maintenance Methods
- `compact_shard(&self, shard_id: &str) -> Result<()>`
- `rebalance_shards(&self) -> Result<()>`
- `backup_shard(&self, shard_id: &str, backup_path: &Path) -> Result<()>`

## See Also

- [Log Ingestor Documentation](./LOG_INGESTOR.md)
- [Log Worker Pool Documentation](./LOG_WORKER_POOL.md)
- [Telemetry Timeline Documentation](./TIMELINE.md)

## License

This module is part of the Bund BlobStore project and is licensed under the same terms.
```
