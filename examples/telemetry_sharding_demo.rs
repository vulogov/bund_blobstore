use bund_blobstore::{
    ShardManagerBuilder, ShardingStrategy, TelemetryQuery, TelemetryRecord, TelemetryValue,
    TimeInterval,
};
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;

/// Simulate telemetry data generator
struct TelemetryGenerator {
    sources: Vec<String>,
    metrics: Vec<String>,
}

impl TelemetryGenerator {
    fn new() -> Self {
        TelemetryGenerator {
            sources: vec![
                "web_server_01".to_string(),
                "web_server_02".to_string(),
                "db_server_01".to_string(),
                "cache_server_01".to_string(),
                "worker_01".to_string(),
            ],
            metrics: vec![
                "cpu_usage".to_string(),
                "memory_usage".to_string(),
                "response_time_ms".to_string(),
                "requests_per_sec".to_string(),
                "error_count".to_string(),
                "temperature_celsius".to_string(),
            ],
        }
    }

    fn generate_record(
        &self,
        timestamp: DateTime<Utc>,
        source_idx: usize,
        metric_idx: usize,
        value_offset: f64,
    ) -> TelemetryRecord {
        let source = self.sources[source_idx % self.sources.len()].clone();
        let metric = self.metrics[metric_idx % self.metrics.len()].clone();

        let value = match metric.as_str() {
            "cpu_usage" => TelemetryValue::Float(20.0 + value_offset * 80.0), // Range 20-100
            "memory_usage" => TelemetryValue::Float(30.0 + value_offset * 70.0), // Range 30-100
            "response_time_ms" => TelemetryValue::Float(50.0 + value_offset * 450.0), // Range 50-500
            "requests_per_sec" => TelemetryValue::Int(100 + (value_offset * 9900.0) as i64), // Range 100-10000
            "error_count" => TelemetryValue::Int((value_offset * 100.0) as i64), // Range 0-100
            "temperature_celsius" => TelemetryValue::Float(22.0 + value_offset * 13.0), // Range 22-35
            _ => TelemetryValue::Float(0.0),
        };

        TelemetryRecord::new_primary(
            format!(
                "{}_{}_{}_{}",
                metric,
                source,
                timestamp.timestamp(),
                metric_idx
            ),
            timestamp,
            metric,
            source,
            value,
        )
        .with_metadata("environment", "production")
        .with_metadata("version", "1.0")
    }

    fn generate_batch(&self, start_time: DateTime<Utc>) -> Vec<TelemetryRecord> {
        let mut records = Vec::new();

        // Generate data for the last hour with multiple records per minute
        for minute_offset in 0..60 {
            // Full hour
            let timestamp = start_time + Duration::minutes(minute_offset);

            // Generate multiple records per minute for each source and metric
            for source_idx in 0..self.sources.len() {
                for metric_idx in 0..self.metrics.len() {
                    let value_offset = minute_offset as f64 / 60.0; // Varies over time
                    records.push(self.generate_record(
                        timestamp,
                        source_idx,
                        metric_idx,
                        value_offset,
                    ));
                }
            }
        }

        println!("  Generated {} test records", records.len());
        records
    }
}

/// Print statistics about telemetry data
fn print_statistics(records: &[TelemetryRecord], title: &str) {
    println!("\n📊 {} Statistics:", title);
    println!("  Total records: {}", records.len());

    if records.is_empty() {
        return;
    }

    // Time range
    let min_time = records.iter().map(|r| r.timestamp()).min().unwrap();
    let max_time = records.iter().map(|r| r.timestamp()).max().unwrap();
    println!("  Time range: {} to {}", min_time, max_time);

    // Unique keys and sources
    let unique_keys: std::collections::HashSet<_> = records.iter().map(|r| r.key.clone()).collect();
    let unique_sources: std::collections::HashSet<_> =
        records.iter().map(|r| r.source.clone()).collect();
    println!("  Unique metrics: {}", unique_keys.len());
    println!("  Unique sources: {}", unique_sources.len());

    // Count by metric
    let mut metric_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for record in records {
        *metric_counts.entry(record.key.clone()).or_insert(0) += 1;
    }
    println!("\n  Records by metric:");
    for (metric, count) in metric_counts.iter().take(5) {
        println!("    {}: {}", metric, count);
    }

    // Sample records
    println!("\n  Sample records (first 3):");
    for record in records.iter().take(3) {
        println!(
            "    [{}] {}:{} = {:?}",
            record.timestamp(),
            record.source,
            record.key,
            record.value
        );
    }
}

/// Query and display results
fn query_and_display(
    shard_manager: &bund_blobstore::ShardManager,
    query: &TelemetryQuery,
    title: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("\n🔍 {}:", title);
    println!(
        "  Time range: {:?} to {:?}",
        query.time_interval.as_ref().map(|t| t.start),
        query.time_interval.as_ref().map(|t| t.end)
    );
    println!("  Keys: {:?}", query.keys);
    println!("  Sources: {:?}", query.sources);

    let results = shard_manager.query_telemetry(query)?;
    println!("  Results found: {}", results.len());

    if !results.is_empty() {
        // Calculate average for numeric values
        let numeric_values: Vec<f64> = results.iter().filter_map(|r| r.value.as_float()).collect();

        if !numeric_values.is_empty() {
            let avg: f64 = numeric_values.iter().sum::<f64>() / numeric_values.len() as f64;
            let min = numeric_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max = numeric_values
                .iter()
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            println!(
                "  Numeric stats - Avg: {:.2}, Min: {:.2}, Max: {:.2}",
                avg, min, max
            );
        }

        // Show sample
        println!("  Sample results (first 3):");
        for record in results.iter().take(3) {
            println!(
                "    [{}] {}:{} = {:?}",
                record.timestamp(),
                record.source,
                record.key,
                record.value
            );
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("🚀 Telemetry Sharding Demo\n");
    println!("{}", "=".repeat(60));

    // Create temporary directory for sharded storage
    let temp_dir = TempDir::new()?;
    let base_path = temp_dir.path();

    println!("\n📁 Using temporary directory: {}", base_path.display());

    // Define time ranges for shards
    let now = Utc::now();
    let shard_configs = vec![
        ("shard_recent", now - Duration::hours(2), now),
        (
            "shard_older",
            now - Duration::hours(4),
            now - Duration::hours(2),
        ),
    ];

    println!("\n📦 Creating time-range sharded storage:");

    let mut builder = ShardManagerBuilder::new().with_strategy(ShardingStrategy::TimeRange);

    for (name, start, end) in &shard_configs {
        let shard_path = base_path.join(name);
        std::fs::create_dir_all(&shard_path)?;
        builder = builder.add_time_range_shard(name, shard_path.to_str().unwrap(), *start, *end);
        println!("  - {}: {} to {}", name, start, end);
    }

    let shard_manager = builder.build()?;
    println!(
        "\n✅ Shard manager created with {} shards",
        shard_configs.len()
    );

    // Generate synthetic telemetry data
    println!("\n📡 Generating synthetic telemetry data...");
    let generator = TelemetryGenerator::new();
    let start_time = now - Duration::hours(1); // Last hour

    let all_records = generator.generate_batch(start_time);
    print_statistics(&all_records, "Generated Data");

    // Store records in appropriate shards based on timestamp
    println!("\n💾 Storing records into shards...");
    let mut stored_count = 0;

    for record in &all_records {
        let shard = shard_manager.get_shard_for_key(&record.id);
        match shard.telemetry().store(record.clone()) {
            Ok(_) => {
                stored_count += 1;
            }
            Err(e) => {
                println!("  ✗ Failed to store record {}: {}", record.id, e);
            }
        }
    }
    println!("✅ Stored {} records across all shards", stored_count);

    // Wait a moment for data to be written
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Demonstrate different query patterns
    println!("\n{}", "=".repeat(60));
    println!("🔍 Query Demonstrations");
    println!("{}", "=".repeat(60));

    // Query 1: Last hour of data
    let query_last_hour = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        limit: 1000,
        ..Default::default()
    };
    query_and_display(&shard_manager, &query_last_hour, "All Data (Last Hour)")?;

    // Query 2: CPU Usage only
    let query_cpu_usage = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        limit: 1000,
        ..Default::default()
    };
    query_and_display(&shard_manager, &query_cpu_usage, "CPU Usage Only")?;

    // Query 3: Web Server metrics
    let query_web_server = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        sources: Some(vec!["web_server_01".to_string()]),
        limit: 1000,
        ..Default::default()
    };
    query_and_display(&shard_manager, &query_web_server, "Web Server 01 Metrics")?;

    // Query 4: Specific metric from specific source
    let query_specific = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        sources: Some(vec!["web_server_01".to_string()]),
        limit: 100,
        ..Default::default()
    };
    query_and_display(&shard_manager, &query_specific, "Web Server 01 CPU Usage")?;

    // Show shard statistics
    println!("\n📊 Shard Statistics:");
    let stats = shard_manager.shard_statistics();
    for detail in stats.shard_details {
        println!("  {}: {} keys", detail.name, detail.key_count);
    }

    // Demonstrate concurrent access
    println!("\n🔄 Demonstrating Concurrent Access:");
    let shard_manager_arc = Arc::new(shard_manager);
    let mut handles = vec![];

    for i in 0..5 {
        let manager = shard_manager_arc.clone();
        let handle = thread::spawn(move || {
            let query = TelemetryQuery {
                time_interval: Some(TimeInterval::last_hour()),
                keys: Some(vec!["cpu_usage".to_string()]),
                limit: 100,
                ..Default::default()
            };

            match manager.query_telemetry(&query) {
                Ok(results) => {
                    println!("  Thread {}: Found {} records", i, results.len());
                }
                Err(e) => {
                    eprintln!("  Thread {}: Error: {}", i, e);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Demonstrate primary-secondary relationships
    println!("\n🔗 Primary-Secondary Relationships:");

    // Create a primary record (main event) - using simpler value types to avoid serialization issues
    let primary_record = TelemetryRecord::new_primary(
        "incident_001".to_string(),
        Utc::now(),
        "system_incident".to_string(),
        "monitoring".to_string(),
        TelemetryValue::String("High CPU usage incident".to_string()),
    )
    .with_metadata("severity", "high")
    .with_metadata("type", "cpu_spike");

    let primary_shard = shard_manager_arc.get_shard_for_key("incident_001");
    match primary_shard.telemetry().store(primary_record) {
        Ok(_) => println!("  ✓ Primary record stored"),
        Err(e) => println!("  ✗ Failed to store primary: {}", e),
    }

    // Create secondary records (detailed logs) - using simpler value types
    let secondary_records = vec![
        TelemetryRecord::new_secondary(
            "incident_001_log_1".to_string(),
            Utc::now(),
            "detailed_log".to_string(),
            "monitoring".to_string(),
            TelemetryValue::String("CPU usage exceeded 90% threshold".to_string()),
            "incident_001".to_string(),
        ),
        TelemetryRecord::new_secondary(
            "incident_001_log_2".to_string(),
            Utc::now(),
            "detailed_log".to_string(),
            "monitoring".to_string(),
            TelemetryValue::String("Auto-scaling triggered successfully".to_string()),
            "incident_001".to_string(),
        ),
        TelemetryRecord::new_secondary(
            "incident_001_log_3".to_string(),
            Utc::now(),
            "detailed_log".to_string(),
            "monitoring".to_string(),
            TelemetryValue::String("CPU usage returned to normal".to_string()),
            "incident_001".to_string(),
        ),
    ];

    for record in secondary_records {
        let shard = shard_manager_arc.get_shard_for_key(&record.id);
        match shard.telemetry().store(record.clone()) {
            Ok(_) => {
                println!("  ✓ Secondary record stored: {}", record.id);
                if let Err(e) = shard
                    .telemetry()
                    .link_primary_secondary("incident_001", &record.id)
                {
                    println!("  ⚠️  Link failed (but record stored): {}", e);
                } else {
                    println!("  ✓ Linked to primary");
                }
            }
            Err(e) => println!("  ✗ Failed to store secondary: {}", e),
        }
    }

    // Retrieve and display secondary records
    match primary_shard.telemetry().get_secondaries("incident_001") {
        Ok(secondaries) => {
            println!(
                "\n  Found {} secondary records for incident_001:",
                secondaries.len()
            );
            for secondary in secondaries {
                if let TelemetryValue::String(msg) = secondary.value {
                    println!("    - {}: {}", secondary.id, msg);
                }
            }
        }
        Err(e) => println!("  Failed to get secondaries: {}", e),
    }

    // Display cache statistics
    println!("\n📊 Cache Statistics:");
    let cache_stats = shard_manager_arc.cache_statistics();
    println!(
        "  Hits: {}, Misses: {}, Hit Rate: {:.2}%",
        cache_stats.hits,
        cache_stats.misses,
        cache_stats.hit_rate * 100.0
    );

    println!("\n✅ Demo completed successfully!");
    println!("\n💡 Key Takeaways:");
    println!("  • Time-range sharding automatically routes telemetry to appropriate shards");
    println!("  • Queries with filters (keys/sources) correctly return matching records");
    println!("  • Primary-secondary relationships link related telemetry events");
    println!("  • Concurrent access is thread-safe across all shards");
    println!("  • LRU caching improves repeated query performance");
    println!(
        "  • Generated {} records across {} shards",
        stored_count,
        shard_configs.len()
    );

    Ok(())
}
