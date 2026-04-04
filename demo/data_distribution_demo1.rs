use bund_blobstore::{
    DataDistributionManager, DistributionStrategy, TimeBucketConfig,
    TimeBucketSize, SimilarityConfig, AdaptiveConfig,
};
use chrono::Utc;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Example 1: Round Robin Distribution
    let rr_manager = DataDistributionManager::new(
        "round_robin_data",
        DistributionStrategy::RoundRobin,
    )?;

    for i in 0..100 {
        rr_manager.put(&format!("key_{}", i), b"data", None)?;
    }

    let stats = rr_manager.get_distribution_stats();
    println!("Round Robin - Total records: {}", stats.total_records);
    println!("Distribution entropy: {:.3}", stats.distribution_entropy);

    // Example 2: Time Bucket Distribution (1-hour buckets)
    let time_config = TimeBucketConfig {
        bucket_size: TimeBucketSize::Hours(1),
        timezone_offset: 0,
        align_to_bucket: true,
    };

    let time_manager = DataDistributionManager::new(
        "time_bucket_data",
        DistributionStrategy::TimeBucket(time_config),
    )?;

    for i in 0..48 {
        let record = bund_blobstore::TelemetryRecord::new_primary(
            format!("metric_{}", i),
            Utc::now() - chrono::Duration::hours(i),
            "test".to_string(),
            "source".to_string(),
            bund_blobstore::TelemetryValue::Float(i as f64),
        );
        time_manager.put_telemetry(record)?;
    }

    // Example 3: Key Similarity Distribution
    let similarity_config = SimilarityConfig {
        use_prefix: true,
        use_suffix: true,
        ngram_size: 3,
        min_similarity: 0.6,
        max_cluster_size: 100,
    };

    let sim_manager = DataDistributionManager::new(
        "similarity_data",
        DistributionStrategy::KeySimilarity(similarity_config),
    )?;

    // Similar keys will be grouped together
    sim_manager.put("user:123:profile", b"data", None)?;
    sim_manager.put("user:123:settings", b"data", None)?;
    sim_manager.put("user:123:history", b"data", None)?;

    // Example 4: Adaptive Distribution with Load Balancing
    let adaptive_config = AdaptiveConfig {
        load_balancing_interval: Duration::from_secs(60),
        rebalance_threshold: 0.2,
        min_shard_load: 0.3,
        max_shard_load: 0.7,
        history_size: 1000,
    };

    let adaptive_manager = DataDistributionManager::new(
        "adaptive_data",
        DistributionStrategy::Adaptive(adaptive_config),
    )?;

    for i in 0..1000 {
        adaptive_manager.put(&format!("item_{}", i), b"data", None)?;
    }

    // Change strategy at runtime
    adaptive_manager.set_strategy(DistributionStrategy::RoundRobin);

    // Get detailed statistics
    let final_stats = adaptive_manager.get_distribution_stats();
    println!("\nFinal Statistics:");
    println!("  Total records: {}", final_stats.total_records);
    println!("  Load balance score: {:.3}", final_stats.load_balance_score);
    println!("  Distribution entropy: {:.3}", final_stats.distribution_entropy);

    for (shard, count) in &final_stats.shard_distribution {
        println!("  {}: {} records", shard, count);
    }

    Ok(())
}
