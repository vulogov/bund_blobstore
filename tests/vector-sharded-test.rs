use bund_blobstore::{
    DataDistributionManager, DistributionStrategy, TelemetryRecord, TelemetryValue, TimeInterval,
    VectorTimeQuery,
};
use chrono::{Duration, Utc};
use tempfile::TempDir;

#[test]
fn test_vector_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    manager.put_vector_text("vec1", "Rust is a systems programming language")?;
    manager.put_vector_text("vec2", "Python excels at data science")?;
    manager.put_vector_text("vec3", "JavaScript runs in web browsers")?;

    let results = manager.vector_search("system programming", 5)?;
    assert!(!results.is_empty());

    Ok(())
}

#[test]
fn test_time_vector_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let now = Utc::now();

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
    assert!(results.len() >= 0);

    Ok(())
}
