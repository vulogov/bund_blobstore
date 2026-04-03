use bund_blobstore::{
    TelemetryRecord, TelemetryValue, TimeInterval, UnifiedConcurrentStore, VectorTimeQuery,
};
use chrono::Utc;
use std::thread;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let store = UnifiedConcurrentStore::open("unified_telemetry.redb")?;

    // Store telemetry with vector embeddings
    let record = TelemetryRecord::new_primary(
        "incident_001".to_string(),
        Utc::now(),
        "system_error".to_string(),
        "api_server".to_string(),
        TelemetryValue::String("Database connection timeout after 30 seconds".to_string()),
    );

    store.vector_telemetry().store_with_vector(record, true)?;

    // Search with time and vector constraints
    let query = VectorTimeQuery {
        time_interval: Some(TimeInterval::last_hour()),
        vector_query: Some("database connection problem".to_string()),
        vector_weight: 0.7,
        time_weight: 0.3,
        keys: None,
        sources: None,
        limit: 10,
        min_similarity: 0.3,
    };

    let results = store.vector_telemetry().search_vector_time(&query)?;
    for result in results {
        println!("Found: {}", result.record.key);
        println!(
            "  Time score: {:.3}, Vector score: {:.3}",
            result.time_score, result.vector_score
        );
        println!("  Combined: {:.3}", result.combined_score);
    }

    // Concurrent access example
    let store_clone = store.clone();
    let handle = thread::spawn(move || {
        let query = VectorTimeQuery {
            time_interval: Some(TimeInterval::last_hour()),
            vector_query: Some("error".to_string()),
            ..Default::default()
        };
        let results = store_clone
            .vector_telemetry()
            .search_vector_time(&query)
            .unwrap();
        println!("Found {} similar events", results.len());
    });

    handle.join().unwrap();

    Ok(())
}
