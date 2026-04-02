use bund_blobstore::{
    UnifiedConcurrentStore, TelemetryRecord, TelemetryValue, TelemetryQuery,
    TimeInterval,
};
use chrono::Utc;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Open unified concurrent store
    let store = UnifiedConcurrentStore::open("unified_telemetry.redb")?;

    // Clone for multiple threads
    let store1 = store.clone();
    let store2 = store.clone();

    // Thread 1: Write telemetry data
    let handle1 = thread::spawn(move || {
        let record = TelemetryRecord::new_primary(
            "cpu_001".to_string(),
            Utc::now(),
            "cpu_usage".to_string(),
            "server_01".to_string(),
            TelemetryValue::Float(45.2),
        );
        store1.telemetry().store(record).unwrap();
    });

    // Thread 2: Query telemetry data
    let handle2 = thread::spawn(move || {
        let query = TelemetryQuery {
            time_interval: Some(TimeInterval::last_hour()),
            keys: Some(vec!["cpu_usage".to_string()]),
            ..Default::default()
        };
        let results = store2.telemetry().query(&query).unwrap();
        println!("Found {} records", results.len());
    });

    handle1.join().unwrap();
    handle2.join().unwrap();

    // Use read/write guards for complex operations
    let read_guard = store.telemetry().read();
    let time_range = read_guard.get_time_range()?;
    if let Some((start, end)) = time_range {
        println!("Telemetry time range: {} to {}", start, end);
    }

    let mut write_guard = store.telemetry().write();
    let secondary = TelemetryRecord::new_secondary(
        "cpu_001_detail".to_string(),
        Utc::now(),
        "core_usage".to_string(),
        "server_01".to_string(),
        TelemetryValue::Json(serde_json::json!({
            "core_0": 45.0,
            "core_1": 52.0,
            "core_2": 38.0,
            "core_3": 41.0
        })),
        "cpu_001".to_string(),
    );
    write_guard.store(secondary)?;
    write_guard.link_primary_secondary("cpu_001", "cpu_001_detail")?;

    Ok(())
}
