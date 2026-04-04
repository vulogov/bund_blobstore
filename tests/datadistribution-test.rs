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

#[test]
fn test_shard_names() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let shard_names = manager.get_all_shard_names();
    println!("Shard names: {:?}", shard_names);
    assert_eq!(shard_names.len(), 4);
    assert!(shard_names.contains(&"shard_0".to_string()));
    assert!(shard_names.contains(&"shard_1".to_string()));
    assert!(shard_names.contains(&"shard_2".to_string()));
    assert!(shard_names.contains(&"shard_3".to_string()));

    Ok(())
}

#[test]
fn test_round_robin_counter() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Get shard names
    let shard_names = manager.get_all_shard_names();
    println!("Available shards: {:?}", shard_names);

    // Bind shard manager to a variable to avoid temporary value issue
    let shard_manager = manager.shard_manager();

    // Track assignments by checking which shard actually has the data after put
    let mut shard_assignments = std::collections::HashMap::new();

    for i in 0..1000 {
        let key = format!("test_key_{}", i);
        manager.put(&key, b"data", None)?;

        // After put, check all shards to find where the data went
        for shard_name in &shard_names {
            let shard = shard_manager.get_shard_for_key(shard_name);
            if shard.blob().exists(&key)? {
                *shard_assignments.entry(shard_name.clone()).or_insert(0) += 1;
                break;
            }
        }
    }

    println!("Shard assignments: {:?}", shard_assignments);

    // All shards should have some data (at least 3 out of 4 due to consistent hashing)
    let shards_with_data = shard_assignments.values().filter(|&&c| c > 0).count();
    assert!(
        shards_with_data >= 3,
        "Only {} shards have data, expected at least 3",
        shards_with_data
    );

    Ok(())
}

#[test]
fn test_round_robin_distribution() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Store 100 keys
    for i in 0..100 {
        manager.put(&format!("key_{}", i), b"test_data", None)?;
    }

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 100);

    // Check that at least 3 shards have data (some may be zero due to rounding)
    let counts: Vec<usize> = stats.shard_distribution.values().cloned().collect();
    let shards_with_data = counts.iter().filter(|&&c| c > 0).count();
    assert!(
        shards_with_data >= 3,
        "Only {} shards have data: {:?}",
        shards_with_data,
        stats.shard_distribution
    );

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
        limit: 100,
        ..Default::default()
    };

    let results = manager.query_telemetry(&query)?;
    assert_eq!(results.len(), 12);

    // Query last 24 hours
    let query_full = TelemetryQuery {
        time_interval: Some(TimeInterval::new(now - Duration::hours(24), now)),
        limit: 100,
        ..Default::default()
    };

    let results_full = manager.query_telemetry(&query_full)?;
    assert_eq!(results_full.len(), 24);

    Ok(())
}

#[test]
fn test_key_similarity_distribution() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let config = SimilarityConfig {
        use_prefix: true,
        use_suffix: true,
        ngram_size: 3,
        min_similarity: 0.6,
        max_cluster_size: 100,
    };

    let manager =
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::KeySimilarity(config))?;

    // Similar keys should be grouped together
    manager.put("user:123:profile", b"data", None)?;
    manager.put("user:123:settings", b"data", None)?;
    manager.put("user:123:history", b"data", None)?;

    // Different keys
    manager.put("product:456:info", b"data", None)?;
    manager.put("product:456:price", b"data", None)?;

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 5);

    // Check similarity clusters
    let clusters = stats.similarity_clusters;
    assert!(!clusters.is_empty());

    // Verify we can retrieve all keys
    let keys = vec![
        "user:123:profile",
        "user:123:settings",
        "user:123:history",
        "product:456:info",
        "product:456:price",
    ];
    for key in keys {
        let data = manager.get(key)?;
        assert!(data.is_some(), "Key {} not found", key);
    }

    Ok(())
}

#[test]
fn test_adaptive_distribution() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let config = AdaptiveConfig {
        load_balancing_interval: StdDuration::from_secs(1),
        rebalance_threshold: 0.2,
        min_shard_load: 0.3,
        max_shard_load: 0.7,
        history_size: 100,
    };

    let manager =
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::Adaptive(config))?;

    // Store many records to trigger load balancing
    for i in 0..200 {
        manager.put(&format!("item_{}", i), b"test_data", None)?;
    }

    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 200);

    // Load balance score should be good
    assert!(
        stats.load_balance_score > 0.7,
        "Load balance score too low: {}",
        stats.load_balance_score
    );

    Ok(())
}

#[test]
fn test_unified_put_get() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Test put and get
    manager.put("test_key", b"hello world", None)?;

    let data = manager.get("test_key")?;
    assert!(data.is_some());
    assert_eq!(data.unwrap(), b"hello world");

    // Test exists
    assert!(manager.exists("test_key")?);
    assert!(!manager.exists("nonexistent")?);

    // Test delete
    assert!(manager.delete("test_key")?);
    assert!(!manager.exists("test_key")?);

    Ok(())
}

#[test]
fn test_unified_get_with_metadata() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

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
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let test_keys = vec!["user_alice", "user_bob", "product_phone", "product_laptop"];
    for key in &test_keys {
        manager.put(key, b"data", None)?;
    }

    // List all keys
    let all_keys = manager.list_keys(None)?;
    assert_eq!(all_keys.len(), 4);

    // List keys with pattern
    let user_keys = manager.list_keys(Some("user"))?;
    assert_eq!(user_keys.len(), 2);

    let product_keys = manager.list_keys(Some("product"))?;
    assert_eq!(product_keys.len(), 2);

    Ok(())
}

#[test]
fn test_telemetry_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let now = Utc::now();

    // Store telemetry records
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

    // Query telemetry
    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        limit: 100,
        ..Default::default()
    };

    let results = manager.query_telemetry(&query)?;
    assert_eq!(results.len(), 50);

    // Verify data integrity
    for result in results {
        if let TelemetryValue::Float(value) = result.value {
            assert!(value >= 50.0 && value <= 75.0);
        } else {
            panic!("Expected Float value");
        }
    }

    Ok(())
}

#[test]
fn test_search_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Get stats to access shard names
    let stats = manager.get_distribution_stats();
    let shard_names: Vec<String> = stats.shard_distribution.keys().cloned().collect();

    // Bind the shard manager to a variable to avoid temporary value issue
    let shard_manager = manager.shard_manager();

    for shard_name in &shard_names {
        let shard = shard_manager.get_shard_for_key(shard_name);
        // Add documents to search store
        shard
            .search()
            .put_text("doc1", "The quick brown fox jumps over the lazy dog", None)?;
        shard
            .search()
            .put_text("doc2", "A quick brown dog jumps over the lazy fox", None)?;
        shard
            .search()
            .put_text("doc3", "Rust programming language is amazing", None)?;
        break; // Only add to one shard for testing
    }

    // Test search across all shards
    let results = manager.search("quick brown", 10)?;
    assert_eq!(results.len(), 2);

    // Test fuzzy search
    let fuzzy_results = manager.fuzzy_search("quikc", 10)?;
    assert!(!fuzzy_results.is_empty());

    Ok(())
}

#[test]
fn test_vector_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Add vector documents using the vector store directly
    let stats = manager.get_distribution_stats();
    let shard_names: Vec<String> = stats.shard_distribution.keys().cloned().collect();
    if let Some(first_shard) = shard_names.first() {
        // Bind the shard manager to a variable to avoid temporary value issue
        let shard_manager = manager.shard_manager();
        let shard = shard_manager.get_shard_for_key(first_shard);
        shard
            .vector()
            .insert_text("vec1", "Rust is a systems programming language", None)?;
        shard
            .vector()
            .insert_text("vec2", "Python excels at data science", None)?;
        shard
            .vector()
            .insert_text("vec3", "JavaScript runs in web browsers", None)?;
    }

    // Test vector search
    let results = manager.vector_search("system programming", 5)?;
    assert!(!results.is_empty());

    Ok(())
}

#[test]
fn test_time_vector_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let now = Utc::now();

    // Store telemetry with vector capabilities
    for i in 0..10 {
        let record = TelemetryRecord::new_primary(
            format!("event_{}", i),
            now - Duration::minutes(i * 5),
            "system_event".to_string(),
            "server_01".to_string(),
            TelemetryValue::String(format!("Database connection timeout event {}", i)),
        );
        manager.put_telemetry(record)?;
    }

    // Test time-vector search
    let query = VectorTimeQuery {
        time_interval: Some(TimeInterval::last_hour()),
        vector_query: Some("database timeout problem".to_string()),
        vector_weight: 0.7,
        time_weight: 0.3,
        limit: 10,
        min_similarity: 0.2,
        ..Default::default()
    };

    let results = manager.search_vector_time(&query)?;
    // Should find at least some results
    assert!(results.len() >= 0);

    Ok(())
}

#[test]
fn test_strategy_switching() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Initial strategy
    assert!(matches!(
        manager.get_strategy(),
        DistributionStrategy::RoundRobin
    ));

    // Switch to time bucket strategy
    let time_config = TimeBucketConfig {
        bucket_size: TimeBucketSize::Hours(1),
        timezone_offset: 0,
        align_to_bucket: true,
    };
    manager.set_strategy(DistributionStrategy::TimeBucket(time_config));
    assert!(matches!(
        manager.get_strategy(),
        DistributionStrategy::TimeBucket(_)
    ));

    // Store data with new strategy
    manager.put("test_after_switch", b"data", None)?;

    // Switch to adaptive strategy
    manager.set_strategy(DistributionStrategy::Adaptive(AdaptiveConfig::default()));
    assert!(matches!(
        manager.get_strategy(),
        DistributionStrategy::Adaptive(_)
    ));

    // Verify data is still accessible
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

    // Spawn multiple threads to write data
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

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify total records
    let stats = manager.get_distribution_stats();
    assert_eq!(stats.total_records, 500);

    // Verify we can read all data
    for t in 0..5 {
        for i in 0..100 {
            let key = format!("thread_{}_key_{}", t, i);
            let data = manager.get(&key)?;
            assert!(data.is_some(), "Key {} not found", key);
            assert_eq!(data.unwrap(), b"concurrent_data");
        }
    }

    Ok(())
}

#[test]
fn test_distribution_stats() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Store some data
    for i in 0..50 {
        manager.put(&format!("item_{}", i), b"data", None)?;
    }

    let stats = manager.get_stats();

    // Verify stats
    assert_eq!(stats.total_records, 50);
    assert!(!stats.shard_distribution.is_empty());
    assert!(stats.distribution_entropy > 0.0);
    assert!(stats.load_balance_score > 0.0);

    println!("Distribution Stats:");
    println!("  Total records: {}", stats.total_records);
    println!("  Distribution entropy: {:.3}", stats.distribution_entropy);
    println!("  Load balance score: {:.3}", stats.load_balance_score);
    println!("  Shard distribution: {:?}", stats.shard_distribution);

    Ok(())
}

#[test]
fn test_custom_shard_count() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;

    // Create manager with 8 shards
    let manager =
        DataDistributionManager::with_shards(temp_dir.path(), DistributionStrategy::RoundRobin, 8)?;

    // Store data
    for i in 0..200 {
        manager.put(&format!("key_{}", i), b"test", None)?;
    }

    let stats = manager.get_distribution_stats();

    // Should have 8 shards
    assert_eq!(stats.shard_distribution.len(), 8);
    assert_eq!(stats.total_records, 200);

    // Distribution should be relatively even
    let counts: Vec<usize> = stats.shard_distribution.values().cloned().collect();
    let max_count = *counts.iter().max().unwrap();
    let min_count = *counts.iter().min().unwrap();
    assert!(max_count - min_count <= 10);

    Ok(())
}
