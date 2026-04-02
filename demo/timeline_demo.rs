use bund_blobstore::{
    MinuteBucket, TelemetryQuery, TelemetryRecord, TelemetryStore, TelemetryValue, TimeInterval,
};
use chrono::{Duration, Utc};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut telemetry = TelemetryStore::open("telemetry.redb")?;

    // Store primary telemetry record
    let primary = TelemetryRecord::new_primary(
        "cpu_001".to_string(),
        Utc::now(),
        "cpu_usage".to_string(),
        "server_01".to_string(),
        TelemetryValue::Float(45.2),
    )
    .with_metadata("unit", "%");
    telemetry.store(primary)?;

    // Store secondary records (detailed metrics)
    let secondary = TelemetryRecord::new_secondary(
        "cpu_001_detail".to_string(),
        Utc::now(),
        "cpu_cores".to_string(),
        "server_01".to_string(),
        TelemetryValue::Json(serde_json::json!({
            "cores": 8,
            "threads": 16,
            "frequency": 3.2
        })),
        "cpu_001".to_string(),
    );
    telemetry.store(secondary)?;

    // Link primary and secondary
    telemetry.link_primary_secondary("cpu_001", "cpu_001_detail")?;

    // Query last hour of data
    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        sources: Some(vec!["server_01".to_string()]),
        limit: 100,
        ..Default::default()
    };

    let results = telemetry.query(&query)?;
    for record in results {
        println!("[{}] {}: {:?}", record.timestamp, record.key, record.value);
    }

    // Get minute-grade bucketed results
    let bucketed = telemetry.query_bucketed(&query)?;
    for bucket in bucketed {
        println!(
            "Bucket: {:?}, Avg: {:?}, Count: {}",
            bucket.bucket, bucket.avg_value, bucket.count
        );
    }

    // Get secondaries for a primary
    let secondaries = telemetry.get_secondaries("cpu_001")?;
    println!("Found {} secondary records", secondaries.len());

    Ok(())
}
