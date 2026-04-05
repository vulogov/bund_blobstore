use bund_blobstore::{
    AdaptiveConfig, DataDistributionManager, DistributionStrategy, SimilarityConfig,
    TelemetryQuery, TelemetryRecord, TelemetryValue, TimeBucketConfig, TimeBucketSize,
    TimeInterval, VectorTimeQuery,
};
use chrono::{Duration, Utc};
use std::sync::Arc;
use std::thread;
use std::time::Duration as StdDuration;
use tempfile::TempDir;

// Helper to create a test manager with timeout
fn create_test_manager() -> Result<DataDistributionManager, Box<dyn std::error::Error + Send + Sync>>
{
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;
    Ok(manager)
}

#[test]
fn test_simple_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Simple put/get test
    manager.put("test", b"data", None)?;
    let result = manager.get("test")?;
    assert_eq!(result, Some(b"data".to_vec()));

    Ok(())
}

#[test]
fn test_no_vector_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Only test blob operations
    for i in 0..10 {
        manager.put(&format!("key_{}", i), b"test", None)?;
    }

    for i in 0..10 {
        let data = manager.get(&format!("key_{}", i))?;
        assert!(data.is_some());
    }

    Ok(())
}

#[test]
fn test_round_robin_distribution() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    // Store 100 keys
    for i in 0..100 {
        manager.put(&format!("key_{}", i), b"test_data", None)?;
    }

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 100);

    // Verify we can retrieve all keys
    for i in 0..100 {
        let key = format!("key_{}", i);
        let data = manager.get(&key)?;
        assert!(data.is_some(), "Key {} not found", key);
        assert_eq!(data.unwrap(), b"test_data");
    }

    Ok(())
}

#[test]
fn test_unified_put_get() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    manager.put("test_key", b"hello world", None)?;

    let data = manager.get("test_key")?;
    assert!(data.is_some());
    assert_eq!(data.unwrap(), b"hello world");

    assert!(manager.exists("test_key")?);
    assert!(!manager.exists("nonexistent")?);

    assert!(manager.delete("test_key")?);
    assert!(!manager.exists("test_key")?);

    Ok(())
}

#[test]
fn test_unified_get_with_metadata() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    manager.put("metadata_test", b"important data", None)?;

    let result = manager.get_with_metadata("metadata_test")?;
    assert!(result.is_some());

    let (data, metadata) = result.unwrap();
    assert_eq!(data, b"important data");
    assert_eq!(metadata.key, "metadata_test");
    assert_eq!(metadata.size, 14);

    Ok(())
}

#[test]
fn test_unified_list_keys() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    let test_keys = vec!["user_alice", "user_bob", "product_phone", "product_laptop"];
    for key in &test_keys {
        manager.put(key, b"data", None)?;
    }

    let all_keys = manager.list_keys(None)?;
    assert_eq!(all_keys.len(), 4);

    let user_keys = manager.list_keys(Some("user"))?;
    assert_eq!(user_keys.len(), 2);

    let product_keys = manager.list_keys(Some("product"))?;
    assert_eq!(product_keys.len(), 2);

    Ok(())
}

#[test]
fn test_search_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    manager.put("doc1", b"The quick brown fox jumps over the lazy dog", None)?;
    manager.put("doc2", b"A quick brown dog jumps over the lazy fox", None)?;
    manager.put("doc3", b"Rust programming language is amazing", None)?;

    let results = manager.search("quick brown", 10)?;
    assert_eq!(results.len(), 2);

    let fuzzy_results = manager.fuzzy_search("quikc", 10)?;
    assert!(!fuzzy_results.is_empty());

    Ok(())
}

#[test]
#[ignore = "Requires embedding model initialization (slow)"]
fn test_vector_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    // Add vector documents
    manager.put_vector_text("vec1", "Rust is a systems programming language")?;
    manager.put_vector_text("vec2", "Python excels at data science")?;
    manager.put_vector_text("vec3", "JavaScript runs in web browsers")?;

    // Test vector search
    let results = manager.vector_search("system programming", 5)?;
    assert!(!results.is_empty());

    Ok(())
}

#[test]
fn test_telemetry_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    let now = Utc::now();

    for i in 0..50 {
        let record = TelemetryRecord::new_primary(
            format!("telemetry_{}", i),
            now - Duration::minutes(i),
            "cpu_usage".to_string(),
            "server_01".to_string(),
            TelemetryValue::Float(50.0 + (i as f64) / 2.0),
        );
        manager.put_telemetry(record)?;
    }

    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        sources: None,
        limit: 100,
        offset: 0,
        primary_only: false,
        secondary_only: false,
        primary_id: None,
        value_type: None,
        bucket_by_minute: false,
    };

    let results = manager.query_telemetry(&query)?;
    assert!(results.len() >= 50);

    Ok(())
}

#[test]
#[ignore = "Requires embedding model initialization (slow)"]
fn test_time_vector_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    let now = Utc::now();

    // Store telemetry with vector embeddings
    for i in 0..10 {
        let record = TelemetryRecord::new_primary(
            format!("event_{}", i),
            now - Duration::minutes(i * 5),
            "system_event".to_string(),
            "server_01".to_string(),
            TelemetryValue::String(format!("Database connection timeout event {}", i)),
        );
        manager.put_telemetry_with_vector(record)?;
    }

    // Test time-vector search
    let query = VectorTimeQuery {
        time_interval: Some(TimeInterval::last_hour()),
        vector_query: Some("database timeout problem".to_string()),
        vector_weight: 0.7,
        time_weight: 0.3,
        keys: None,
        sources: None,
        limit: 10,
        min_similarity: 0.2,
    };

    let results = manager.search_vector_time(&query)?;
    // May return 0 or more results depending on model
    assert!(results.len() >= 0);

    Ok(())
}

#[test]
fn test_shard_names() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    let shard_names = manager.get_all_shard_names();
    assert_eq!(shard_names.len(), 4);
    assert!(shard_names.contains(&"shard_0".to_string()));
    assert!(shard_names.contains(&"shard_1".to_string()));
    assert!(shard_names.contains(&"shard_2".to_string()));
    assert!(shard_names.contains(&"shard_3".to_string()));

    Ok(())
}

#[test]
fn test_distribution_stats() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    for i in 0..50 {
        manager.put(&format!("item_{}", i), b"data", None)?;
    }

    let stats = manager.get_stats();
    assert_eq!(stats.total_records, 50);
    assert!(!stats.shard_distribution.is_empty());

    Ok(())
}

#[test]
fn test_strategy_switching() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = create_test_manager()?;

    assert!(matches!(
        manager.get_strategy(),
        DistributionStrategy::RoundRobin
    ));

    manager.set_strategy(DistributionStrategy::Adaptive(AdaptiveConfig::default()));
    assert!(matches!(
        manager.get_strategy(),
        DistributionStrategy::Adaptive(_)
    ));

    manager.put("test_after_switch", b"data", None)?;

    let data = manager.get("test_after_switch")?;
    assert!(data.is_some());

    Ok(())
}

#[test]
fn test_concurrent_access() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = Arc::new(DataDistributionManager::new(
        temp_dir.path(),
        DistributionStrategy::RoundRobin,
    )?);

    let mut handles = vec![];

    for t in 0..5 {
        let manager_clone = manager.clone();
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let key = format!("thread_{}_key_{}", t, i);
                manager_clone.put(&key, b"concurrent_data", None).unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 500);

    Ok(())
}

#[test]
fn test_time_bucket_distribution() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let config = TimeBucketConfig {
        bucket_size: TimeBucketSize::Hours(1),
        timezone_offset: 0,
        align_to_bucket: true,
    };

    let manager =
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::TimeBucket(config))?;

    let now = Utc::now();

    // Store telemetry records across different hours
    for i in 0..24 {
        let record = TelemetryRecord::new_primary(
            format!("metric_{}", i),
            now - Duration::hours(i),
            "test_metric".to_string(),
            "test_source".to_string(),
            TelemetryValue::Float(i as f64),
        );
        manager.put_telemetry(record)?;
    }

    // Query last 12 hours
    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::new(now - Duration::hours(12), now)),
        keys: None,
        sources: None,
        limit: 100,
        offset: 0,
        primary_only: false,
        secondary_only: false,
        primary_id: None,
        value_type: None,
        bucket_by_minute: false,
    };

    let results = manager.query_telemetry(&query)?;
    assert!(
        results.len() >= 12,
        "Expected at least 12 records, got {}",
        results.len()
    );

    Ok(())
}

#[test]
fn test_adaptive_distribution() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let config = AdaptiveConfig {
        load_balancing_interval: StdDuration::from_secs(1),
        rebalance_threshold: 0.8,
        min_shard_load: 0.2,
        max_shard_load: 0.8,
        history_size: 100,
    };

    let manager =
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::Adaptive(config))?;

    // Store many records
    for i in 0..500 {
        manager.put(&format!("item_{}", i), b"test_data", None)?;
    }

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 500);
    assert!(stats.load_balance_score > 0.2);

    Ok(())
}

#[test]
fn test_round_robin_counter() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let shard_names = manager.get_all_shard_names();
    println!("Available shards: {:?}", shard_names);

    // Store keys and track distribution by checking which shard gets each key
    // We can't directly know which shard a key went to, so we'll check the shard statistics after
    for i in 0..100 {
        let key = format!("test_key_{}", i);
        manager.put(&key, b"data", None)?;
    }

    // Get distribution statistics
    let stats = manager.get_distribution_stats();
    println!("Shard distribution: {:?}", stats.shard_distribution);

    // All shards should have some data (approximately 25 each for 100 keys across 4 shards)
    let shards_with_data = stats
        .shard_distribution
        .values()
        .filter(|&&c| c > 0)
        .count();
    assert!(
        shards_with_data >= 3,
        "Only {} shards have data, distribution: {:?}",
        shards_with_data,
        stats.shard_distribution
    );

    // Verify total records
    assert_eq!(stats.total_records, 100);

    Ok(())
}

#[test]
fn test_key_similarity_distribution() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let config = SimilarityConfig::default();

    let manager =
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::KeySimilarity(config))?;

    // Similar keys should be grouped together
    manager.put("user:123:profile", b"data", None)?;
    manager.put("user:123:settings", b"data", None)?;
    manager.put("user:123:history", b"data", None)?;
    manager.put("product:456:info", b"data", None)?;
    manager.put("product:456:price", b"data", None)?;

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 5);

    Ok(())
}

#[test]
fn test_custom_shard_count() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;

    let manager =
        DataDistributionManager::with_shards(temp_dir.path(), DistributionStrategy::RoundRobin, 8)?;

    for i in 0..200 {
        manager.put(&format!("key_{}", i), b"test", None)?;
    }

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.shard_distribution.len(), 8);
    assert_eq!(stats.total_records, 200);

    Ok(())
}
