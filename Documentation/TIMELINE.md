```markdown
# Telemetry Timeline Documentation

## Overview

The `TelemetryTimeline` module provides a high-performance, time-series telemetry storage system optimized for storing, querying, and analyzing telemetry data. It supports efficient time-range queries, vector embeddings, primary/secondary record relationships, and comprehensive metadata indexing.

## Features

- **Time-Series Optimization** - Efficient storage and retrieval of time-ordered telemetry data
- **Vector Similarity Search** - Find semantically similar telemetry records using embeddings
- **Primary/Secondary Relationships** - Link related telemetry records with similarity detection
- **Rich Metadata Support** - Store and query arbitrary key-value metadata
- **Time-Range Queries** - Efficiently query telemetry data within time intervals
- **Multi-Source Aggregation** - Aggregate data from multiple sources and keys
- **Downsampling & Aggregation** - Built-in support for data reduction and statistics
- **Export & Import** - Export telemetry data to JSON/CSV formats
- **Retention Policies** - Automatic cleanup of old telemetry data
- **Concurrent Access** - Thread-safe with fine-grained locking

## Quick Start

```rust
use bund_blobstore::timeline::{TelemetryTimeline, TelemetryRecord, TelemetryValue};
use std::collections::HashMap;

// Create a new timeline
let timeline = TelemetryTimeline::new("./telemetry_data")?;

// Create a telemetry record
let mut metadata = HashMap::new();
metadata.insert("unit".to_string(), "celsius".to_string());

let record = TelemetryRecord {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp_seconds: chrono::Utc::now().timestamp(),
    key: "temperature".to_string(),
    source: "sensor_1".to_string(),
    value: TelemetryValue::Float(23.5),
    metadata,
    is_primary: true,
    primary_id: None,
    secondary_ids: vec![],
};

// Insert the record
timeline.insert(&record.key, record.timestamp_seconds, &record.value.to_string(), Some(record.metadata.clone()))?;

// Query records
let start_time = chrono::Utc::now().timestamp() - 3600;
let end_time = chrono::Utc::now().timestamp();
let results = timeline.query_range("temperature", start_time, end_time)?;

println!("Found {} records", results.len());
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["timeline"] }
```

## Core Components

### TelemetryValue

The value type for telemetry data:

```rust
pub enum TelemetryValue {
    Int(i64),                    // Integer value
    Float(f64),                  // Floating point value
    Bool(bool),                  // Boolean value
    String(String),              // String value
    Blob(Vec<u8>),               // Binary data
    Json(serde_json::Value),     // JSON data
    Dynamic(Value),              // Dynamic value (from bund_primitives)
    Null,                        // Null value
}
```

### TelemetryRecord

Complete telemetry record structure:

```rust
pub struct TelemetryRecord {
    pub id: String,                          // Unique identifier
    pub timestamp_seconds: i64,              // Unix timestamp in seconds
    pub key: String,                         // Metric or event name
    pub source: String,                      // Source of telemetry (e.g., service name)
    pub value: TelemetryValue,               // Telemetry value
    pub metadata: HashMap<String, String>,   // Additional metadata
    pub is_primary: bool,                    // Is this a primary record?
    pub primary_id: Option<String>,          // ID of primary record (if secondary)
    pub secondary_ids: Vec<String>,          // IDs of secondary records
}
```

## Usage Examples

### 1. Basic Telemetry Storage

```rust
use bund_blobstore::timeline::{TelemetryTimeline, TelemetryRecord, TelemetryValue};
use chrono::Utc;

let timeline = TelemetryTimeline::new("./telemetry")?;

// Store integer metric
timeline.insert("cpu_usage", Utc::now().timestamp(), "45", None)?;

// Store float metric
timeline.insert("temperature", Utc::now().timestamp(), "23.5", None)?;

// Store string event
timeline.insert("user_action", Utc::now().timestamp(), "login", None)?;

// Store with metadata
let mut metadata = std::collections::HashMap::new();
metadata.insert("environment".to_string(), "production".to_string());
metadata.insert("version".to_string(), "1.2.3".to_string());

timeline.insert("deployment", Utc::now().timestamp(), "success", Some(metadata))?;
```

### 2. Time-Range Queries

```rust
let timeline = TelemetryTimeline::new("./telemetry")?;

// Query last hour of data
let now = Utc::now().timestamp();
let one_hour_ago = now - 3600;

let records = timeline.query_range("cpu_usage", one_hour_ago, now)?;

for record in records {
    println!("Timestamp: {}, Value: {:?}", record.timestamp_seconds, record.value);
}

// Query specific time window
let start = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap().timestamp();
let end = chrono::Utc.with_ymd_and_hms(2024, 1, 16, 0, 0, 0).unwrap().timestamp();

let daily_records = timeline.query_range("api_requests", start, end)?;
println!("Total API requests on Jan 15: {}", daily_records.len());
```

### 3. Working with Different Value Types

```rust
use bund_blobstore::timeline::TelemetryValue;

// Integer values
timeline.insert("counter", now, "42", None)?;
timeline.insert("status_code", now, "200", None)?;

// Float values
timeline.insert("temperature", now, "23.5", None)?;
timeline.insert("cpu_usage", now, "45.2", None)?;

// Boolean values
timeline.insert("is_healthy", now, "true", None)?;
timeline.insert("has_error", now, "false", None)?;

// String values
timeline.insert("event_type", now, "user_login", None)?;
timeline.insert("error_message", now, "Connection timeout", None)?;

// JSON values
let json_value = serde_json::json!({
    "user_id": 12345,
    "action": "purchase",
    "items": ["item1", "item2"]
});
timeline.insert("complex_event", now, &json_value.to_string(), None)?;
```

### 4. Metadata Management

```rust
use std::collections::HashMap;

let timeline = TelemetryTimeline::new("./telemetry")?;

// Store with rich metadata
let mut metadata = HashMap::new();
metadata.insert("host".to_string(), "server-01".to_string());
metadata.insert("region".to_string(), "us-east-1".to_string());
metadata.insert("environment".to_string(), "production".to_string());
metadata.insert("version".to_string(), "v2.1.0".to_string());

timeline.insert("deployment", now, "success", Some(metadata))?;

// Query and filter by metadata
let records = timeline.query_range("deployment", start, end)?;
for record in records {
    if let Some(env) = record.metadata.get("environment") {
        if env == "production" {
            println!("Production deployment at {}", record.timestamp_seconds);
        }
    }
}
```

### 5. Primary/Secondary Record Relationships

```rust
// Create primary record (original log)
let primary = TelemetryRecord {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp_seconds: Utc::now().timestamp(),
    key: "error".to_string(),
    source: "database".to_string(),
    value: TelemetryValue::String("Connection pool exhausted".to_string()),
    metadata: HashMap::new(),
    is_primary: true,
    primary_id: Some("primary_id".to_string()),
    secondary_ids: vec![],
};

timeline.insert_telemetry(primary)?;

// Create secondary record (similar error)
let secondary = TelemetryRecord {
    id: uuid::Uuid::new_v4().to_string(),
    timestamp_seconds: Utc::now().timestamp(),
    key: "error".to_string(),
    source: "database".to_string(),
    value: TelemetryValue::String("Connection pool timeout".to_string()),
    metadata: HashMap::new(),
    is_primary: false,
    primary_id: Some("primary_id".to_string()),
    secondary_ids: vec![],
};

timeline.insert_telemetry(secondary)?;

// Query all related errors
let primary_record = timeline.get_telemetry("primary_id")?;
if let Some(primary) = primary_record {
    println!("Primary error: {:?}", primary.value);
    for secondary_id in &primary.secondary_ids {
        if let Some(secondary) = timeline.get_telemetry(secondary_id)? {
            println!("Related error: {:?}", secondary.value);
        }
    }
}
```

### 6. Vector Similarity Search

```rust
use bund_blobstore::vector::VectorEmbedding;

// Create embeddings for telemetry records
let embedding1 = VectorEmbedding {
    id: "record_1".to_string(),
    vector: vec![0.1, 0.2, 0.3, 0.4],
    metadata: HashMap::new(),
    created_at: Utc::now().timestamp(),
};

let embedding2 = VectorEmbedding {
    id: "record_2".to_string(),
    vector: vec![0.15, 0.25, 0.35, 0.45],
    metadata: HashMap::new(),
    created_at: Utc::now().timestamp(),
};

timeline.store_embedding(embedding1)?;
timeline.store_embedding(embedding2)?;

// Find similar records
let query_embedding = vec![0.12, 0.22, 0.32, 0.42];
let similar = timeline.find_similar(&query_embedding, 0.85)?;

for (id, similarity) in similar {
    println!("Record {} is {:.2}% similar", id, similarity * 100.0);
}
```

### 7. Aggregations and Downsampling

```rust
let timeline = TelemetryTimeline::new("./telemetry")?;

// Calculate average over time range
let records = timeline.query_range("cpu_usage", start, end)?;
let avg_cpu: f64 = records.iter()
    .filter_map(|r| match r.value {
        TelemetryValue::Float(v) => Some(v),
        TelemetryValue::Int(v) => Some(v as f64),
        _ => None,
    })
    .sum::<f64>() / records.len() as f64;

println!("Average CPU usage: {:.2}%", avg_cpu);

// Downsample to 5-minute intervals
let interval_seconds = 300;
let mut downsampled = Vec::new();

for chunk in records.chunks(interval_seconds as usize) {
    let avg: f64 = chunk.iter()
        .filter_map(|r| match r.value {
            TelemetryValue::Float(v) => Some(v),
            _ => None,
        })
        .sum::<f64>() / chunk.len() as f64;
    
    downsampled.push(avg);
}
```

### 8. Multi-Source Aggregation

```rust
// Collect data from multiple sources
let sources = vec!["server-01", "server-02", "server-03"];
let mut all_records = Vec::new();

for source in sources {
    let records = timeline.query_source_range(source, "cpu_usage", start, end)?;
    all_records.extend(records);
}

// Calculate aggregate statistics
let total_records = all_records.len();
let avg_value: f64 = all_records.iter()
    .filter_map(|r| match r.value {
        TelemetryValue::Float(v) => Some(v),
        _ => None,
    })
    .sum::<f64>() / total_records as f64;

println!("Average across all servers: {:.2}", avg_value);
```

### 9. Export and Import

```rust
// Export telemetry to JSON
let records = timeline.query_range("api_requests", start, end)?;
let json = serde_json::to_string_pretty(&records)?;
std::fs::write("export.json", json)?;

// Export to CSV
let mut wtr = csv::Writer::from_path("export.csv")?;
wtr.write_record(&["timestamp", "key", "value", "source"])?;

for record in records {
    wtr.write_record(&[
        record.timestamp_seconds.to_string(),
        record.key,
        format!("{:?}", record.value),
        record.source,
    ])?;
}
wtr.flush()?;

// Import from JSON
let json_data = std::fs::read_to_string("import.json")?;
let records: Vec<TelemetryRecord> = serde_json::from_str(&json_data)?;

for record in records {
    timeline.insert_telemetry(record)?;
}
```

### 10. Retention Policies

```rust
// Configure retention policy
let timeline = TelemetryTimeline::with_retention("./telemetry", 30)?; // 30 days retention

// Automatic cleanup of old data (runs in background)
timeline.enable_auto_cleanup(Duration::from_secs(86400))?; // Daily cleanup

// Manual cleanup
let deleted = timeline.cleanup_old_records(Utc::now().timestamp() - 30 * 86400)?;
println!("Deleted {} old records", deleted);
```

### 11. Real-Time Monitoring

```rust
use std::sync::Arc;
use std::thread;
use std::time::Duration;

let timeline = Arc::new(timeline);

// Spawn monitoring thread
let timeline_clone = timeline.clone();
thread::spawn(move || loop {
    let now = Utc::now().timestamp();
    let five_min_ago = now - 300;
    
    let recent = timeline_clone.query_range("error", five_min_ago, now)?;
    
    if recent.len() > 10 {
        println!("Alert: High error rate - {} errors in last 5 minutes", recent.len());
    }
    
    thread::sleep(Duration::from_secs(60));
});

// Spawn metrics collection thread
let timeline_clone = timeline.clone();
thread::spawn(move || loop {
    let metrics = timeline_clone.get_metrics();
    println!("Timeline metrics: {:?}", metrics);
    thread::sleep(Duration::from_secs(300));
});
```

### 12. Advanced Querying

```rust
// Query with multiple conditions
let records = timeline.query_range("response_time", start, end)?;

// Filter by source
let filtered: Vec<_> = records.iter()
    .filter(|r| r.source == "api-gateway")
    .collect();

// Calculate percentiles
let mut values: Vec<f64> = records.iter()
    .filter_map(|r| match r.value {
        TelemetryValue::Float(v) => Some(v),
        _ => None,
    })
    .collect();

values.sort_by(|a, b| a.partial_cmp(b).unwrap());

let p50 = values[values.len() / 2];
let p95 = values[(values.len() as f64 * 0.95) as usize];
let p99 = values[(values.len() as f64 * 0.99) as usize];

println!("Response time percentiles - P50: {}, P95: {}, P99: {}", p50, p95, p99);
```

## Performance Optimization

### Indexing Strategies

```rust
// Create indexes for faster queries
timeline.create_index("source")?;
timeline.create_index("key")?;
timeline.create_index("timestamp")?;

// Composite index
timeline.create_composite_index(&["key", "source"])?;
```

### Batch Operations

```rust
// Batch insert for better performance
let mut batch = Vec::new();
for i in 0..10000 {
    batch.push(TelemetryRecord {
        id: uuid::Uuid::new_v4().to_string(),
        timestamp_seconds: Utc::now().timestamp(),
        key: "metric".to_string(),
        source: "batch".to_string(),
        value: TelemetryValue::Int(i),
        metadata: HashMap::new(),
        is_primary: true,
        primary_id: None,
        secondary_ids: vec![],
    });
}

timeline.batch_insert(batch)?;
```

### Query Optimization

```rust
// Use time-range queries when possible
let results = timeline.query_range("metric", start, end)?;

// Limit result size
let results = timeline.query_range_limit("metric", start, end, 1000)?;

// Use specific indexes
let results = timeline.query_with_index("source", "api-gateway", start, end)?;
```

## Error Handling

```rust
match timeline.insert("key", timestamp, "value", None) {
    Ok(_) => println!("Successfully stored"),
    Err(e) => eprintln!("Failed to store: {}", e),
}

match timeline.query_range("key", start, end) {
    Ok(records) => println!("Found {} records", records.len()),
    Err(e) => eprintln!("Query failed: {}", e),
}
```

## Best Practices

1. **Timestamp Precision** - Use seconds for most use cases, milliseconds if needed
2. **Key Naming** - Use consistent naming conventions (e.g., `service.metric`, `component.event`)
3. **Metadata** - Keep metadata lean; use it for filtering, not for large data
4. **Batch Operations** - Use batch inserts for bulk loading
5. **Retention** - Set appropriate retention policies based on data value
6. **Indexing** - Create indexes for frequently queried fields
7. **Vector Search** - Normalize vectors before storage for better similarity results

## Troubleshooting

### Issue: Slow Queries
**Solution**: Add indexes, reduce time range, or use downsampled data

### Issue: High Storage Usage
**Solution**: Implement retention policies, enable compression, or use downsampling

### Issue: Memory Exhaustion
**Solution**: Use streaming queries, limit result sizes, or increase batch sizes

### Issue: Poor Similarity Results
**Solution**: Tune similarity threshold, ensure vectors are normalized, or use different embedding model

## API Reference

### Core Methods
- `new<P: AsRef<Path>>(path: P) -> Result<Self>`
- `insert(&self, key: &str, timestamp: i64, value: &str, metadata: Option<HashMap<String, String>>) -> Result<()>`
- `query_range(&self, key: &str, start: i64, end: i64) -> Result<Vec<TelemetryRecord>>`
- `delete_range(&self, key: &str, start: i64, end: i64) -> Result<usize>`

### Telemetry Methods
- `insert_telemetry(&self, record: TelemetryRecord) -> Result<()>`
- `get_telemetry(&self, id: &str) -> Result<Option<TelemetryRecord>>`
- `update_telemetry(&self, record: TelemetryRecord) -> Result<()>`
- `delete_telemetry(&self, id: &str) -> Result<bool>`

### Vector Methods
- `store_embedding(&self, embedding: VectorEmbedding) -> Result<()>`
- `find_similar(&self, vector: &[f32], threshold: f32) -> Result<Vec<(String, f32)>>`

### Maintenance Methods
- `cleanup_old_records(&self, before_timestamp: i64) -> Result<usize>`
- `compact(&self) -> Result<()>`
- `export_range(&self, key: &str, start: i64, end: i64, path: &Path) -> Result<()>`
- `import(&self, path: &Path) -> Result<usize>`

### Statistics Methods
- `get_metrics(&self) -> HashMap<String, u64>`
- `get_storage_size(&self) -> Result<u64>`
- `get_record_count(&self) -> Result<usize>`

## See Also

- [Data Distribution Manager](./DATA_DISTRIBUTION.md)
- [Vector Embeddings](./vector.md)
- [Log Ingestor](./LOG_INGESTOR.md)

## License

This module is part of the Bund BlobStore project and is licensed under the same terms.
```
