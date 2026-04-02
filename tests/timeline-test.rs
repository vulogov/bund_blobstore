use bund_blobstore::{
    TelemetryQuery, TelemetryRecord, TelemetryStore, TelemetryValue, TimeInterval,
};
use chrono::{Duration, Utc};
use tempfile::NamedTempFile;

#[test]
fn test_telemetry_store() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_file = NamedTempFile::new()?;
    let mut store = TelemetryStore::open(temp_file.path())?;

    let now = Utc::now();

    // Store primary record
    let primary = TelemetryRecord::new_primary(
        "primary_1".to_string(),
        now,
        "temperature".to_string(),
        "sensor_1".to_string(),
        TelemetryValue::Float(23.5),
    );
    store.store(primary)?;

    // Store secondary record with different key
    let secondary = TelemetryRecord::new_secondary(
        "secondary_1".to_string(),
        now + Duration::seconds(1),
        "humidity".to_string(), // Different key
        "sensor_1".to_string(),
        TelemetryValue::Float(45.2),
        "primary_1".to_string(),
    );
    store.store(secondary)?;

    // Store another record with same key but different timestamp
    let primary2 = TelemetryRecord::new_primary(
        "primary_2".to_string(),
        now - Duration::hours(2), // 2 hours ago
        "temperature".to_string(),
        "sensor_1".to_string(),
        TelemetryValue::Float(22.0),
    );
    store.store(primary2)?;

    // Link primary and secondary
    store.link_primary_secondary("primary_1", "secondary_1")?;

    // Query for temperature records in the last hour
    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["temperature".to_string()]),
        ..Default::default()
    };

    let results = store.query(&query)?;

    // Should only find primary_1 (within last hour) not primary_2 (2 hours ago)
    assert_eq!(
        results.len(),
        1,
        "Expected 1 temperature record in last hour, got {}",
        results.len()
    );
    assert_eq!(results[0].id, "primary_1");
    assert_eq!(results[0].key, "temperature");

    // Test query without time filter
    let query_all = TelemetryQuery {
        keys: Some(vec!["temperature".to_string()]),
        ..Default::default()
    };

    let all_results = store.query(&query_all)?;
    assert_eq!(all_results.len(), 2, "Expected 2 temperature records total");

    // Test query by source
    let source_query = TelemetryQuery {
        sources: Some(vec!["sensor_1".to_string()]),
        ..Default::default()
    };

    let source_results = store.query(&source_query)?;
    assert_eq!(source_results.len(), 3, "Expected 3 records from sensor_1");

    // Test getting secondaries
    let secondaries = store.get_secondaries("primary_1")?;
    assert_eq!(secondaries.len(), 1);
    assert_eq!(secondaries[0].id, "secondary_1");

    // Test getting primary from secondary
    let primary_from_secondary = store.get_primary("secondary_1")?;
    assert!(primary_from_secondary.is_some());
    assert_eq!(primary_from_secondary.unwrap().id, "primary_1");

    // Test bucketed query
    let bucketed_query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_day()),
        bucket_by_minute: true,
        ..Default::default()
    };

    let bucketed = store.query_bucketed(&bucketed_query)?;
    assert!(!bucketed.is_empty());

    Ok(())
}
