use bund_blobstore::{
    DataDistributionManager, DistributionStrategy, TelemetryQuery, TimeBucketConfig,
    TimeBucketSize, TimeInterval, VectorTimeQuery,
};
use chrono::Utc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize manager with round-robin distribution
    let manager = DataDistributionManager::new("unified_data", DistributionStrategy::RoundRobin)?;

    // 1. Simple PUT and GET operations
    manager.put("user:100", b"Alice's profile", None)?;
    manager.put("user:101", b"Bob's profile", None)?;

    if let Some(data) = manager.get("user:100")? {
        println!("Retrieved: {}", String::from_utf8_lossy(&data));
    }

    // 2. Check existence and delete
    if manager.exists("user:101")? {
        manager.delete("user:101")?;
    }

    // 3. List keys with pattern
    let keys = manager.list_keys(Some("user"))?;
    println!("Found {} user keys: {:?}", keys.len(), keys);

    // 4. Telemetry storage and query
    use bund_blobstore::{TelemetryRecord, TelemetryValue};

    for i in 0..100 {
        let record = TelemetryRecord::new_primary(
            format!("metric_{}", i),
            Utc::now(),
            "cpu_usage".to_string(),
            "server_01".to_string(),
            TelemetryValue::Float(50.0 + (i as f64) / 2.0),
        );
        manager.put_telemetry(record)?;
    }

    let telemetry_query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        limit: 20,
        ..Default::default()
    };

    let results = manager.query_telemetry(&telemetry_query)?;
    println!("Found {} telemetry records", results.len());

    // 5. Full-text search
    manager
        .search()
        .put_text("doc1", "The quick brown fox jumps over the lazy dog", None)?;
    manager
        .search()
        .put_text("doc2", "A quick brown dog jumps over the lazy fox", None)?;

    let search_results = manager.search("quick brown", 10)?;
    for result in search_results {
        println!("Search found: {} (score: {})", result.key, result.score);
    }

    // 6. Vector search
    manager
        .vector()
        .insert_text("vec1", "Rust programming language", None)?;
    manager
        .vector()
        .insert_text("vec2", "Python data science", None)?;

    let vector_results = manager.vector_search("systems programming", 5)?;
    for result in vector_results {
        println!(
            "Vector search: {} (similarity: {:.3})",
            result.key, result.score
        );
    }

    // 7. Time-vector search (combines temporal and semantic relevance)
    let time_vector_query = VectorTimeQuery {
        time_interval: Some(TimeInterval::last_hour()),
        vector_query: Some("database connection problem".to_string()),
        vector_weight: 0.7,
        time_weight: 0.3,
        limit: 10,
        min_similarity: 0.3,
        ..Default::default()
    };

    let hybrid_results = manager.search_vector_time(&time_vector_query)?;
    for result in hybrid_results {
        println!("Time-Vector result: {}", result.record.key);
        println!(
            "  Time score: {:.3}, Vector score: {:.3}",
            result.time_score, result.vector_score
        );
    }

    // 8. Get distribution statistics
    let stats = manager.get_stats();
    println!("\nDistribution Statistics:");
    println!("  Total records: {}", stats.total_records);
    println!("  Load balance score: {:.3}", stats.load_balance_score);
    println!("  Distribution entropy: {:.3}", stats.distribution_entropy);

    for (shard, count) in &stats.shard_distribution {
        println!("  {}: {} records", shard, count);
    }

    // 9. Change distribution strategy at runtime
    use bund_blobstore::TimeBucketConfig;

    let time_config = TimeBucketConfig {
        bucket_size: TimeBucketSize::Hours(1),
        timezone_offset: 0,
        align_to_bucket: true,
    };

    manager.set_strategy(DistributionStrategy::TimeBucket(time_config));
    println!("\nSwitched to time-bucket distribution strategy");

    // New records will use the new strategy
    manager.put("new_key", b"new data", None)?;

    Ok(())
}
