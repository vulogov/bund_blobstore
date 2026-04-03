use bund_blobstore::{
    TelemetryRecord, TelemetryValue, TimeInterval, VectorTelemetryStore, VectorTimeQuery,
};
use chrono::Utc;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    mod tests {
        use super::*;
        use chrono::Duration;
        use tempfile::NamedTempFile;

        #[test]
        fn test_vector_time_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let temp_file = NamedTempFile::new()?;
            let mut store = VectorTelemetryStore::open(temp_file.path())?;

            let now = Utc::now();

            // Store telemetry with vectors
            let record1 = TelemetryRecord::new_primary(
                "event_1".to_string(),
                now,
                "cpu_usage".to_string(),
                "server_01".to_string(),
                TelemetryValue::String("High CPU usage detected".to_string()),
            );
            store.store_with_vector(record1, true)?;

            let record2 = TelemetryRecord::new_primary(
                "event_2".to_string(),
                now - Duration::minutes(30),
                "memory_usage".to_string(),
                "server_01".to_string(),
                TelemetryValue::String("Memory leak warning".to_string()),
            );
            store.store_with_vector(record2, true)?;

            // Search with time and vector constraints
            let query = VectorTimeQuery {
                time_interval: Some(TimeInterval::last_hour()),
                vector_query: Some("CPU problem".to_string()),
                vector_weight: 0.7,
                time_weight: 0.3,
                keys: None,
                sources: None,
                limit: 10,
                min_similarity: 0.2,
            };

            let results = store.search_vector_time(&query)?;
            assert!(!results.is_empty());

            // Find similar events
            let similar = store.find_similar_events("event_1", 2, 5)?;
            println!("Found {} similar events", similar.len());

            // Get temporal patterns
            let patterns = store.get_temporal_patterns("CPU", 24)?;
            println!("Found {} temporal patterns", patterns.len());

            Ok(())
        }
    }
}
