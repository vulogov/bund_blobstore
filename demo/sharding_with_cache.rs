use bund_blobstore::{CacheConfig, ShardManagerBuilder, ShardingStrategy};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Configure cache
    let cache_config = CacheConfig {
        enabled: true,
        max_size: 5000,
        default_ttl: Duration::from_secs(300),
        key_cache_ttl: Duration::from_secs(600),
        time_cache_ttl: Duration::from_secs(300),
    };

    // Create shard manager with caching
    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyHash)
        .with_cache_config(cache_config)
        .add_shard("shard1", "/tmp/shard1.redb")
        .add_shard("shard2", "/tmp/shard2.redb")
        .build()?;

    // First access - cache miss
    let shard1 = manager.get_shard_for_key("user_123");

    // Second access - cache hit
    let shard2 = manager.get_shard_for_key("user_123");

    // Get cache statistics
    let stats = manager.cache_statistics();
    println!(
        "Cache hits: {}, misses: {}, hit rate: {:.2}%",
        stats.hits,
        stats.misses,
        stats.hit_rate * 100.0
    );

    // Preload cache with common keys
    let common_keys = vec!["user_100".to_string(), "user_101".to_string()];
    manager.preload_cache(&common_keys);

    Ok(())
}
