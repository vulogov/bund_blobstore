use bund_blobstore::{
    ShardManagerBuilder, ShardingStrategy, TelemetryQuery, TelemetryRecord, TelemetryValue,
    TimeInterval, UnifiedConcurrentStore,
};
use chrono::{Duration, Utc};
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;

    // Create a sharded store with key-based hashing
    let shard_manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyHash)
        .add_shard("shard1", &temp_dir.path().join("shard1").to_str().unwrap())
        .add_shard("shard2", &temp_dir.path().join("shard2").to_str().unwrap())
        .add_shard("shard3", &temp_dir.path().join("shard3").to_str().unwrap())
        .build()?;

    // Write data to appropriate shard based on key
    let shard = shard_manager.get_shard_for_key("user_123");
    shard.blob().put("profile", b"user data", None)?;

    // Store telemetry across shards
    let now = Utc::now();
    let record = TelemetryRecord::new_primary(
        "metric_001".to_string(),
        now,
        "cpu_usage".to_string(),
        "server_01".to_string(),
        TelemetryValue::Float(45.2),
    );

    let telemetry_shard = shard_manager.get_shard_for_key("telemetry:metric_001");
    telemetry_shard.telemetry().store(record)?;

    // Query across all shards
    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        limit: 100,
        ..Default::default()
    };

    let results = shard_manager.query_telemetry(&query)?;
    println!(
        "Found {} telemetry records across all shards",
        results.len()
    );

    // Get shard statistics
    let stats = shard_manager.shard_statistics();
    println!("Total shards: {}", stats.total_shards);
    for detail in stats.shard_details {
        println!("Shard {}: {} keys", detail.name, detail.key_count);
    }

    Ok(())
}
