use bund_blobstore::{
    VectorTelemetryStore, TelemetryRecord, TelemetryValue,
    VectorTimeQuery, TimeInterval,
};
use chrono::Utc;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut store = VectorTelemetryStore::open("vector_telemetry.redb")?;

    // Store telemetry with vector embeddings
    let record = TelemetryRecord::new_primary(
        "incident_001".to_string(),
        Utc::now(),
        "system_error".to_string(),
        "api_server".to_string(),
        TelemetryValue::String("Database connection timeout after 30 seconds".to_string()),
    );
    store.store_with_vector(record, true)?;

    // Search with time range and semantic similarity
    let query = VectorTimeQuery {
        time_interval: Some(TimeInterval::last_hour()),
        vector_query: Some("database connection problem".to_string()),
        vector_weight: 0.7,
        time_weight: 0.3,
        limit: 10,
        min_similarity: 0.3,
        ..Default::default()
    };

    let results = store.search_vector_time(&query)?;
    for result in results {
        println!("Found: {}", result.record.key);
        println!("  Time score: {:.3}, Vector score: {:.3}",
                 result.time_score, result.vector_score);
        println!("  Combined: {:.3}", result.combined_score);
    }

    // Find events similar to a specific event within time window
    let similar = store.find_similar_events("incident_001", 24, 10)?;
    for event in similar {
        println!("Similar event: {} at {}",
                 event.record.key, event.record.timestamp());
        println!("  Similarity: {:.3}", event.similarity);
    }

    // Analyze temporal patterns for a concept
    let patterns = store.get_temporal_patterns("timeout error", 168)?; // Last week
    for pattern in patterns {
        println!("Hour {}: {} events, avg similarity {:.3}",
                 pattern.hour_timestamp, pattern.count, pattern.avg_similarity);
    }

    Ok(())
}
