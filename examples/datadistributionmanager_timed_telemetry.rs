use bund_blobstore::{
    DataDistributionManager, DistributionStrategy, TelemetryQuery, TelemetryRecord, TelemetryValue,
    TimeBucketConfig, TimeBucketSize, TimeInterval, VectorTimeQuery,
};
use chrono::{Duration, Utc};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let storage_path = "./bund_data";
    let now = Utc::now();

    // 1. Initialize Manager with Time Partitioning (1-hour buckets)
    // This physically separates data on disk by time, speeding up interval queries.
    let strategy = DistributionStrategy::TimeBucket(TimeBucketConfig {
        bucket_size: TimeBucketSize::Hours(1),
        align_to_bucket: true,
        ..Default::default()
    });

    let manager = DataDistributionManager::new(storage_path, strategy)?;

    // 2. STORE PRIMARY (The Substation)
    let substation_id = "substation_east_7";
    let primary = TelemetryRecord::new_primary(
        substation_id.to_string(),
        now,
        "grid_node".to_string(),
        "utility_main".to_string(),
        TelemetryValue::String("East Sector Primary Substation - High Capacity".to_string()),
    );
    manager.put_telemetry_with_vector(primary)?;

    // 3. STORE SECONDARIES (The Sensors linked to the Substation)
    // We vary the time slightly to demonstrate the time-interval search
    let sensor_data = vec![
        (
            "trans_01",
            "Transformer core temperature reaching critical levels",
            5,
        ), // 5 mins ago
        ("volt_02", "Unstable voltage fluctuations on line 2", 10), // 10 mins ago
        (
            "gate_03",
            "Security perimeter breach detected at south gate",
            15,
        ), // 15 mins ago
    ];

    for (s_id, s_desc, mins_ago) in sensor_data {
        let timestamp = now - Duration::minutes(mins_ago);
        let mut secondary = TelemetryRecord::new_primary(
            s_id.to_string(),
            timestamp,
            "sensor_alert".to_string(),
            "hardware_layer".to_string(),
            TelemetryValue::String(s_desc.to_string()),
        );

        // LINKING: Connect this sensor to the parent substation
        secondary.is_primary = false;
        secondary.primary_id = Some(substation_id.to_string());

        manager.put_telemetry_with_vector(secondary)?;
    }

    // --- SCENARIO: HYBRID VECTOR + TIME SEARCH ---
    // Objective: Find "Electrical issues" that happened in the last 12 minutes.
    // This should skip the "Security breach" (15 mins ago) and focus on technical alerts.

    println!("\n🔍 Executing Hybrid Search (Vector + Time Interval)...");

    let query = VectorTimeQuery {
        vector_query: Some("electrical overheating and voltage stability".to_string()),
        time_interval: Some(TimeInterval::new(
            now - Duration::minutes(12), // Start
            now,                         // End
        )),
        vector_weight: 0.7,
        time_weight: 0.3,
        min_similarity: 0.2,
        limit: 5,
        ..Default::default()
    };

    let results = manager.search_vector_time(&query)?;

    if results.is_empty() {
        println!("❌ No results matched both semantic and time criteria.");
    } else {
        println!(
            "🎉 Found {} relevant alerts within the time window:",
            results.len()
        );
        for res in results {
            let rec = res.record;
            println!("---");
            println!("Timestamp:  {}", rec.timestamp());
            println!("Sensor ID:  {}", rec.id);
            println!("Similarity: {:.4}", res.similarity);
            println!("Content:    {:?}", rec.value);

            // DISCOVER PRIMARY
            if let Some(parent) = rec.primary_id {
                println!("🔗 Impacting Parent Asset: {}", parent);
            }
        }
    }

    Ok(())
}
