use bund_blobstore::{DataDistributionManager, DistributionStrategy};
use chrono::{Duration, Utc};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create initial manager with 2 shards
    let manager =
        DataDistributionManager::with_shards("dynamic_data", DistributionStrategy::RoundRobin, 2)?;

    println!("Initial shard count: {}", manager.shard_count());

    // Add a new regular shard
    manager.add_shard("shard_2", "/tmp/dynamic_data/shard_2")?;
    println!("After adding shard: {}", manager.shard_count());

    // Add a key-range based shard
    manager.add_key_range_shard("shard_range_a_m", "/tmp/dynamic_data/shard_range", "a", "m")?;
    println!("After adding key-range shard: {}", manager.shard_count());

    // Add a time-range based shard
    let now = Utc::now();
    manager.add_time_range_shard(
        "shard_time_q1",
        "/tmp/dynamic_data/shard_time",
        now - Duration::days(90),
        now,
    )?;
    println!("After adding time-range shard: {}", manager.shard_count());

    // View all shard details
    println!("\nShard Details:");
    for detail in manager.get_shard_details() {
        println!("  {}: {} keys", detail.name, detail.key_count);
    }

    // Store some data
    for i in 0..100 {
        manager.put(&format!("key_{}", i), b"data", None)?;
    }

    // View shard loads
    println!("\nShard Loads:");
    for (shard, load) in manager.get_shard_loads() {
        println!("  {}: {:.2}%", shard, load * 100.0);
    }

    // Check if a shard exists
    if manager.shard_exists("shard_2") {
        println!("\nShard 'shard_2' exists");
    }

    // Remove a shard
    if manager.remove_shard("shard_2")? {
        println!("Shard 'shard_2' removed");
        println!("Current shard count: {}", manager.shard_count());
    }

    // Rebalance after changes
    manager.rebalance()?;

    // Change strategy at runtime
    manager.set_strategy(DistributionStrategy::Adaptive(AdaptiveConfig::default()));
    println!("\nStrategy changed to Adaptive");

    Ok(())
}
