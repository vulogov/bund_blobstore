# Close and Sync Operations Documentation

## Overview

The Bund BlobStore provides comprehensive cache management, synchronization, and optimization features to ensure data consistency and optimal performance. This guide covers all close, sync, and optimization operations available in the `DataDistributionManager`.

## Table of Contents
- [Cache Management](#cache-management)
- [Synchronization Operations](#synchronization-operations)
- [Optimization Features](#optimization-features)
- [Health Monitoring](#health-monitoring)
- [System Statistics](#system-statistics)
- [Best Practices](#best-practices)
- [Complete Examples](#complete-examples)

## Cache Management

### Clear All Caches

Clears both time bucket and key cluster caches simultaneously.

```rust
use bund_blobstore::{DataDistributionManager, DistributionStrategy};

let manager = DataDistributionManager::new(
    "data_store",
    DistributionStrategy::RoundRobin,
)?;

// Clear all caches
manager.clear_caches();
```

### Clear Specific Cache Types

Target specific cache types for fine-grained control.

```rust
use bund_blobstore::{DataDistributionManager, CacheType};

// Clear only time bucket cache
manager.clear_cache_by_type(CacheType::TimeBucket);

// Clear only key cluster cache
manager.clear_cache_by_type(CacheType::KeyCluster);

// Clear all caches
manager.clear_cache_by_type(CacheType::All);
```

### Get Cache Statistics

Monitor cache performance and size.

```rust
let cache_stats = manager.get_cache_stats();
println!("Time bucket cache size: {}", cache_stats.time_bucket_cache_size);
println!("Key cluster cache size: {}", cache_stats.key_cluster_cache_size);
println!("Total cache size: {}", cache_stats.total_cache_size);
```

## Synchronization Operations

### Sync All Shards

Forces synchronization across all shards, verifies accessibility, and clears caches.

```rust
// Sync all shards
manager.sync_all_shards()?;

// Output:
// [Sync] Starting sync across all shards...
// [Sync] Shard 'shard_0' is accessible
// [Sync] Shard 'shard_1' is accessible
// [Sync] Shard 'shard_2' is accessible
// [Sync] Shard 'shard_3' is accessible
// [Sync] All 4 shards synced successfully
```

### Sync Specific Shard

Target a single shard for synchronization.

```rust
// Sync a specific shard by name
manager.sync_shard("shard_0")?;

// Handle non-existent shard
match manager.sync_shard("non_existent") {
    Ok(_) => println!("Shard synced"),
    Err(e) => println!("Error: {}", e),
}
```

### Flush and Sync

Combines cache clearing with full synchronization.

```rust
// Flush all pending operations and sync
manager.flush_and_sync()?;

// This operation:
// 1. Clears all caches
// 2. Verifies all shards are accessible
// 3. Forces data durability
```

## Optimization Features

### Optimize All Shards

Triggers optimization across all storage components (blob, vector, and search stores).

```rust
// Optimize all shards
manager.optimize_all_shards()?;

// Output:
// [Optimize] Starting optimization across all shards...
// [Optimize] Optimizing blob store for shard: shard_0
// [Optimize] Optimizing vector store for shard: shard_0
// [Optimize] Optimizing search store for shard: shard_0
// [Optimize] Optimization completed
```

## Health Monitoring

### Get Shard Health Status

Monitor the health of each shard in the system.

```rust
let health_status = manager.get_shard_health();

for shard in health_status {
    println!("Shard: {}", shard.shard_name);
    println!("  Healthy: {}", shard.is_healthy);
    println!("  Key count: {}", shard.key_count);
    println!("  Last sync: {}", shard.last_sync);
}
```

## System Statistics

### Get Comprehensive System Statistics

Retrieve overall system metrics including distribution, cache, and health information.

```rust
let stats = manager.get_system_stats();

println!("=== System Statistics ===");
println!("Total records: {}", stats.total_records);
println!("Shard count: {}", stats.shard_count);
println!("Distribution entropy: {:.3}", stats.distribution_entropy);
println!("Load balance score: {:.3}", stats.load_balance_score);
println!("\nCache Statistics:");
println!("  Time bucket cache: {}", stats.cache_stats.time_bucket_cache_size);
println!("  Key cluster cache: {}", stats.cache_stats.key_cluster_cache_size);
println!("  Total cache size: {}", stats.cache_stats.total_cache_size);
println!("\nShard Health:");
for shard in &stats.shard_health {
    println!("  {}: healthy={}, keys={}", 
             shard.shard_name, shard.is_healthy, shard.key_count);
}
```

## Best Practices

### 1. Regular Synchronization

For production systems, implement regular sync intervals:

```rust
use std::time::Duration;
use std::thread;

// Sync every 5 minutes
loop {
    thread::sleep(Duration::from_secs(300));
    manager.sync_all_shards()?;
}
```

### 2. Cache Management Strategy

```rust
// Clear caches before major operations
manager.clear_caches();

// Perform batch operations
for i in 0..10000 {
    manager.put(&format!("batch_key_{}", i), b"data", None)?;
}

// Sync after batch operations
manager.flush_and_sync()?;
```

### 3. Health Monitoring

```rust
// Periodic health checks
let health = manager.get_shard_health();
let unhealthy_shards: Vec<_> = health.iter()
    .filter(|h| !h.is_healthy)
    .collect();

if !unhealthy_shards.is_empty() {
    println!("Warning: Unhealthy shards detected: {:?}", unhealthy_shards);
    // Implement recovery logic
}
```

### 4. Graceful Shutdown

```rust
fn graceful_shutdown(manager: &DataDistributionManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting graceful shutdown...");
    
    // Clear all caches
    manager.clear_caches();
    
    // Sync all shards
    manager.sync_all_shards()?;
    
    // Final optimization
    manager.optimize_all_shards()?;
    
    println!("Shutdown complete");
    Ok(())
}
```

## Complete Examples

### Example 1: Production-Ready Setup

```rust
use bund_blobstore::{DataDistributionManager, DistributionStrategy};
use std::time::Duration;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize manager
    let manager = DataDistributionManager::new(
        "production_data",
        DistributionStrategy::RoundRobin,
    )?;
    
    // Initial sync to ensure all shards are ready
    manager.sync_all_shards()?;
    
    // Monitor health in background thread
    let manager_clone = manager.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(60));
            
            let health = manager_clone.get_shard_health();
            let stats = manager_clone.get_system_stats();
            
            println!("[Health Check] Total records: {}", stats.total_records);
            println!("[Health Check] Load balance: {:.3}", stats.load_balance_score);
            
            for shard in health {
                if !shard.is_healthy {
                    eprintln!("[WARNING] Shard {} is unhealthy!", shard.shard_name);
                }
            }
        }
    });
    
    // Perform operations
    for i in 0..1000 {
        manager.put(&format!("key_{}", i), b"data", None)?;
        
        // Sync every 100 operations
        if i % 100 == 0 {
            manager.flush_and_sync()?;
        }
    }
    
    // Graceful shutdown
    manager.flush_and_sync()?;
    manager.optimize_all_shards()?;
    
    Ok(())
}
```

### Example 2: Cache Management for Performance

```rust
fn batch_import(manager: &DataDistributionManager, data: Vec<(String, Vec<u8>)>) -> Result<(), Box<dyn std::error::Error>> {
    // Disable caching for bulk import (if configurable)
    manager.clear_caches();
    
    // Batch insert
    for (key, value) in data {
        manager.put(&key, &value, None)?;
    }
    
    // Rebuild caches after import
    manager.flush_and_sync()?;
    
    Ok(())
}
```

### Example 3: Maintenance Routine

```rust
fn maintenance_routine(manager: &DataDistributionManager) -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting maintenance routine...");
    
    // 1. Sync all data
    manager.sync_all_shards()?;
    
    // 2. Get pre-optimization stats
    let before_stats = manager.get_system_stats();
    println!("Before optimization: {} records", before_stats.total_records);
    
    // 3. Optimize storage
    manager.optimize_all_shards()?;
    
    // 4. Clear caches
    manager.clear_caches();
    
    // 5. Verify health
    let after_health = manager.get_shard_health();
    for shard in after_health {
        if !shard.is_healthy {
            return Err(format!("Shard {} is unhealthy after maintenance", shard.shard_name).into());
        }
    }
    
    println!("Maintenance completed successfully");
    Ok(())
}
```

## Error Handling

Always handle potential errors from sync operations:

```rust
match manager.sync_all_shards() {
    Ok(_) => println!("Sync successful"),
    Err(e) => {
        eprintln!("Sync failed: {}", e);
        // Implement retry logic
        thread::sleep(Duration::from_secs(5));
        manager.sync_all_shards()?;
    }
}
```

## Performance Considerations

1. **Cache Clearing**: Clear caches before large batch operations to prevent memory bloat
2. **Sync Frequency**: Balance between data durability and performance
3. **Health Checks**: Run health checks asynchronously to avoid blocking
4. **Optimization**: Run optimization during low-traffic periods

## Troubleshooting

### Common Issues and Solutions

| Issue | Solution |
|-------|----------|
| Cache not clearing | Verify you're using the correct CacheType |
| Sync operation timeout | Increase timeout or reduce batch size |
| Unhealthy shards | Check disk space and file permissions |
| High memory usage | Clear caches more frequently |

## API Reference

### CacheType Enum
```rust
pub enum CacheType {
    TimeBucket,  // Time-based distribution cache
    KeyCluster,  // Key similarity cluster cache
    All,         // All cache types
}
```

### CacheStats Struct
```rust
pub struct CacheStats {
    pub time_bucket_cache_size: usize,
    pub key_cluster_cache_size: usize,
    pub total_cache_size: usize,
}
```

### ShardHealth Struct
```rust
pub struct ShardHealth {
    pub shard_name: String,
    pub is_healthy: bool,
    pub key_count: usize,
    pub last_sync: DateTime<Utc>,
}
```

### SystemStats Struct
```rust
pub struct SystemStats {
    pub shard_health: Vec<ShardHealth>,
    pub cache_stats: CacheStats,
    pub total_records: usize,
    pub shard_count: usize,
    pub distribution_entropy: f64,
    pub load_balance_score: f64,
}
```

## Conclusion

The close and sync operations provide essential tools for maintaining data consistency, monitoring system health, and optimizing performance in production environments. Use these features according to your application's requirements for data durability and performance.
