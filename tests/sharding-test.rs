use bund_blobstore::{
    ShardConfig, ShardManagerBuilder, ShardingStrategy, TelemetryQuery, TelemetryRecord,
    TelemetryValue, TimeInterval,
};
use chrono::{Duration, Utc};
use std::path::PathBuf;
use tempfile::TempDir;

fn create_unique_shard_dirs(base_dir: &TempDir, count: usize) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for i in 0..count {
        let dir = base_dir.path().join(format!("shard_{}", i));
        dirs.push(dir);
    }
    dirs
}

#[test]
fn test_shard_manager_key_hash() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let dirs = create_unique_shard_dirs(&temp_dir, 3);

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyHash)
        .add_shard("shard1", dirs[0].to_str().unwrap())
        .add_shard("shard2", dirs[1].to_str().unwrap())
        .add_shard("shard3", dirs[2].to_str().unwrap())
        .build()?;

    let shard = manager.get_shard_for_key("test_key");
    assert!(shard.blob().len().is_ok());

    // Test writing to shard
    shard.blob().put("test_key", b"test_value", None)?;

    // Test reading from shard
    let value = shard.blob().get("test_key")?;
    assert_eq!(value, Some(b"test_value".to_vec()));

    Ok(())
}

#[test]
fn test_shard_manager_key_prefix() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let dirs = create_unique_shard_dirs(&temp_dir, 2);

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyPrefix)
        .add_key_range_shard("shard1", dirs[0].to_str().unwrap(), "a", "m")
        .add_key_range_shard("shard2", dirs[1].to_str().unwrap(), "n", "z")
        .build()?;

    // Keys starting with 'c' should go to shard1
    let shard1 = manager.get_shard_for_key("cat");
    shard1.blob().put("cat", b"cat data", None)?;

    // Keys starting with 'r' should go to shard2
    let shard2 = manager.get_shard_for_key("rabbit");
    shard2.blob().put("rabbit", b"rabbit data", None)?;

    // Verify data is in correct shards
    assert_eq!(shard1.blob().get("cat")?, Some(b"cat data".to_vec()));
    assert_eq!(shard2.blob().get("rabbit")?, Some(b"rabbit data".to_vec()));

    Ok(())
}

#[test]
fn test_shard_manager_consistent_hash() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let dirs = create_unique_shard_dirs(&temp_dir, 4);

    let mut manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::ConsistentHash)
        .add_shard("shard1", dirs[0].to_str().unwrap())
        .add_shard("shard2", dirs[1].to_str().unwrap())
        .add_shard("shard3", dirs[2].to_str().unwrap())
        .build()?;

    // Test key distribution
    let keys = vec!["key1", "key2", "key3", "key4", "key5"];

    for key in &keys {
        let shard = manager.get_shard_for_key(key);
        assert!(shard.blob().len().is_ok());
    }

    // Add a new shard dynamically
    manager.add_shard(ShardConfig {
        name: "shard4".to_string(),
        db_path: dirs[3].clone(),
        strategy: ShardingStrategy::ConsistentHash,
        key_range: None,
        time_range: None,
    })?;

    // Verify we can still get shards
    let shard = manager.get_shard_for_key("test_key");
    assert!(shard.blob().len().is_ok());

    Ok(())
}

#[test]
fn test_shard_manager_time_range() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let dirs = create_unique_shard_dirs(&temp_dir, 3);
    let now = Utc::now();

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::TimeRange)
        .add_time_range_shard(
            "shard_recent",
            dirs[0].to_str().unwrap(),
            now - Duration::hours(2),
            now,
        )
        .add_time_range_shard(
            "shard_mid",
            dirs[1].to_str().unwrap(),
            now - Duration::hours(4),
            now - Duration::hours(2),
        )
        .add_time_range_shard(
            "shard_old",
            dirs[2].to_str().unwrap(),
            now - Duration::hours(6),
            now - Duration::hours(4),
        )
        .build()?;

    // Test storing telemetry in the appropriate shard
    let record = TelemetryRecord::new_primary(
        "test_001".to_string(),
        now,
        "test_metric".to_string(),
        "test_source".to_string(),
        TelemetryValue::Float(42.0),
    );

    let shard = manager.get_shard_for_key(&record.id);
    shard.telemetry().store(record)?;

    // Query back the data
    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["test_metric".to_string()]),
        limit: 100,
        ..Default::default()
    };

    let results = shard.telemetry().query(&query)?;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "test_metric");

    Ok(())
}

#[test]
fn test_shard_manager_query_all() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let dirs = create_unique_shard_dirs(&temp_dir, 2);

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyHash)
        .add_shard("shard1", dirs[0].to_str().unwrap())
        .add_shard("shard2", dirs[1].to_str().unwrap())
        .build()?;

    // Write data to different shards
    for i in 0..10 {
        let key = format!("key_{}", i);
        let shard = manager.get_shard_for_key(&key);
        shard.blob().put(&key, b"data", None)?;
    }

    // Query all shards
    let total_keys = manager.query_all_shards(|shard| Ok(shard.blob().list_keys()?))?;

    assert_eq!(total_keys.len(), 10);

    Ok(())
}
