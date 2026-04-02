use bund_blobstore::{ShardManagerBuilder, ShardingStrategy};
use chrono::{Duration, Utc};
use tempfile::TempDir;

#[test]
fn test_shard_manager_key_hash() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;

    // Each shard needs its own directory
    let shard1_dir = temp_dir.path().join("shard1");
    let shard2_dir = temp_dir.path().join("shard2");
    let shard3_dir = temp_dir.path().join("shard3");

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyHash)
        .add_shard("shard1", shard1_dir.to_str().unwrap())
        .add_shard("shard2", shard2_dir.to_str().unwrap())
        .add_shard("shard3", shard3_dir.to_str().unwrap())
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
fn test_shard_manager_time_range() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let shard1_dir = temp_dir.path().join("shard1");
    let shard2_dir = temp_dir.path().join("shard2");
    let now = Utc::now();

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::TimeRange)
        .add_time_range_shard(
            "shard1",
            shard1_dir.to_str().unwrap(),
            now - Duration::days(30),
            now,
        )
        .add_time_range_shard(
            "shard2",
            shard2_dir.to_str().unwrap(),
            now - Duration::days(60),
            now - Duration::days(31),
        )
        .build()?;

    let shards = manager.get_shards_for_time_interval(now - Duration::hours(1), now);
    assert_eq!(shards.len(), 1);

    // Test storing telemetry in the appropriate shard
    use bund_blobstore::{TelemetryQuery, TelemetryRecord, TelemetryValue, TimeInterval};

    let record = TelemetryRecord::new_primary(
        "test_001".to_string(),
        now,
        "test_metric".to_string(),
        "test_source".to_string(),
        TelemetryValue::Float(42.0),
    );

    // Store in the correct shard (should go to shard1 based on time)
    if let Some(shard) = shards.first() {
        shard.telemetry().store(record)?;

        // Query back the data
        let query = TelemetryQuery {
            time_interval: Some(TimeInterval::last_hour()),
            keys: Some(vec!["test_metric".to_string()]),
            ..Default::default()
        };

        let results = shard.telemetry().query(&query)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "test_metric");
    }

    Ok(())
}

#[test]
fn test_shard_manager_key_prefix() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let shard1_dir = temp_dir.path().join("shard1");
    let shard2_dir = temp_dir.path().join("shard2");

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyPrefix)
        .add_key_range_shard("shard1", shard1_dir.to_str().unwrap(), "a", "m")
        .add_key_range_shard("shard2", shard2_dir.to_str().unwrap(), "n", "z")
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
    let shard1_dir = temp_dir.path().join("shard1");
    let shard2_dir = temp_dir.path().join("shard2");
    let shard3_dir = temp_dir.path().join("shard3");
    let shard4_dir = temp_dir.path().join("shard4");

    let mut manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::ConsistentHash)
        .add_shard("shard1", shard1_dir.to_str().unwrap())
        .add_shard("shard2", shard2_dir.to_str().unwrap())
        .add_shard("shard3", shard3_dir.to_str().unwrap())
        .build()?;

    // Test key distribution
    let keys = vec!["key1", "key2", "key3", "key4", "key5"];

    for key in &keys {
        let shard = manager.get_shard_for_key(key);
        assert!(shard.blob().len().is_ok());
    }

    // Add a new shard dynamically
    manager.add_shard(bund_blobstore::ShardConfig {
        name: "shard4".to_string(),
        db_path: shard4_dir,
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
fn test_shard_manager_query_all() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let shard1_dir = temp_dir.path().join("shard1");
    let shard2_dir = temp_dir.path().join("shard2");

    let manager = ShardManagerBuilder::new()
        .with_strategy(ShardingStrategy::KeyHash)
        .add_shard("shard1", shard1_dir.to_str().unwrap())
        .add_shard("shard2", shard2_dir.to_str().unwrap())
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
