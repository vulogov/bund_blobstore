Here's the comprehensive documentation for the Log Ingestor module:

```markdown
# Log Ingestor Documentation

## Overview

The `LogIngestor` module provides a high-performance, production-ready solution for ingesting log files into the Bund BlobStore telemetry system. It features intelligent batching, efficient deduplication using Bloom filters, automatic sharding, and similarity-based relationship detection between log entries.

## Features

- **URL & Local File Support** - Ingest logs from HTTP/HTTPS URLs or local files
- **Batch Processing** - Process logs in configurable batches to avoid prolonged write locks
- **Intelligent Deduplication** - Memory-efficient Bloom filter for duplicate detection
- **Automatic Sharding** - Time-based sharding with configurable intervals
- **Primary/Secondary Relationships** - Detect similar log entries using embeddings
- **Comprehensive Statistics** - Track every aspect of the ingestion process
- **Thread-Safe** - Safe for concurrent ingestion operations
- **Configurable Delays** - Adjustable delays between batches to prevent resource contention

## Quick Start

```rust
use bund_blobstore::common::grok_integration::GrokLogParser;
use bund_blobstore::common::log_ingestor::{LogIngestor, LogIngestionConfig};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use std::sync::Arc;
use parking_lot::RwLock;

// Initialize components
let temp_dir = std::path::PathBuf::from("/tmp/bund_data");
let distribution_manager = Arc::new(RwLock::new(
    DataDistributionManager::new(&temp_dir, DistributionStrategy::RoundRobin).unwrap()
));
let grok_parser = GrokLogParser::new("my_app");
let config = LogIngestionConfig::default();
let ingestor = LogIngestor::new(distribution_manager, grok_parser, config);

// Ingest log lines
let log_lines = vec![
    "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
    "2024-01-15T10:30:46Z ERROR [worker] db_error: Connection failed".to_string(),
];

let stats = ingestor.ingest_log_lines(log_lines, "application_logs")?;
println!("Stored {} records", stats.total_records_stored);
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["log-ingestor"] }
```

## Core Components

### LogIngestionConfig

Configuration structure for fine-tuning ingestion behavior:

```rust
pub struct LogIngestionConfig {
    pub batch_size: usize,                      // Lines per batch (default: 1000)
    pub shard_interval_seconds: i64,            // Time interval for sharding (default: 3600)
    pub max_retries: u32,                       // Download retry attempts (default: 3)
    pub download_timeout_seconds: u64,          // HTTP timeout (default: 30)
    pub delete_after_ingest: bool,              // Delete temp files (default: true)
    pub temp_dir: PathBuf,                      // Temp directory for downloads
    pub batch_delay_ms: u64,                    // Delay between batches (default: 100)
    pub auto_sharding: bool,                    // Enable automatic sharding (default: true)
    pub enable_deduplication: bool,             // Enable duplicate detection (default: true)
    pub enable_embedding: bool,                 // Enable vector embeddings (default: true)
    pub enable_similarity_matching: bool,       // Enable similarity detection (default: true)
    pub bloom_filter_size: usize,               // Bloom filter size (default: 1,000,000)
    pub bloom_filter_false_positive_rate: f64,  // False positive rate (default: 0.01)
    pub similarity_config: SimilarityConfig,    // Similarity thresholds
    pub embedding_model: EmbeddingModel,        // Embedding model to use
}
```

### IngestionStats

Statistics returned after each ingestion operation:

```rust
pub struct IngestionStats {
    pub total_lines_read: usize,        // Total lines processed
    pub total_records_parsed: usize,    // Successfully parsed records
    pub total_records_stored: usize,    // Records stored in database
    pub failed_parses: usize,           // Failed parsing attempts
    pub batches_processed: usize,       // Number of batches processed
    pub shards_created: usize,          // Shards created
    pub download_size_bytes: u64,       // Bytes downloaded (for URL ingestion)
    pub ingestion_duration_ms: u64,     // Total ingestion time
    pub duplicates_filtered: usize,     // Duplicates removed
    pub primary_records: usize,         // Primary records stored
    pub secondary_records: usize,       // Secondary records stored
    pub embeddings_computed: usize,     // Embeddings generated
    pub similarity_matches: usize,      // Similarity matches found
}
```

## Usage Examples

### 1. Basic Log File Ingestion

```rust
use bund_blobstore::common::log_ingestor::{LogIngestor, LogIngestionConfig};

let ingestor = setup_ingestor();
let stats = ingestor.ingest_log_file("path/to/app.log", "application")?;

println!("Processed {} lines", stats.total_lines_read);
println!("Stored {} records", stats.total_records_stored);
println!("Filtered {} duplicates", stats.duplicates_filtered);
```

### 2. URL-Based Ingestion

```rust
let ingestor = setup_ingestor();
let stats = ingestor.ingest_from_url(
    "https://example.com/logs/application.log",
    "remote_logs"
)?;

println!("Downloaded {} bytes", stats.download_size_bytes);
println!("Ingested {} records", stats.total_records_stored);
```

### 3. Ingesting Vector Operation Logs

```rust
let log_lines = vec![
    "VECTOR|search|dim=1536|time=87ms|top_k=10,threshold=0.8".to_string(),
    "VECTOR|index|dim=768|time=1234ms|vectors=10000".to_string(),
];

let stats = ingestor.ingest_log_lines(log_lines, "vector_operations")?;
assert_eq!(stats.total_records_stored, 2);
```

### 4. Custom Configuration for High Throughput

```rust
use bund_blobstore::common::log_ingestor::{LogIngestionConfig, SimilarityConfig};

let mut config = LogIngestionConfig::default();
config.batch_size = 5000;                    // Larger batches
config.batch_delay_ms = 50;                  // Shorter delays
config.auto_sharding = true;
config.shard_interval_seconds = 86400;       // Daily shards
config.enable_deduplication = true;
config.bloom_filter_size = 10_000_000;       // 10M items
config.bloom_filter_false_positive_rate = 0.001; // 0.1% false positive

let ingestor = LogIngestor::new(distribution_manager, grok_parser, config);
```

### 5. Disabling Deduplication for Testing

```rust
let mut config = LogIngestionConfig::default();
config.enable_deduplication = false;
config.enable_similarity_matching = false;

let ingestor = LogIngestor::new(distribution_manager, grok_parser, config);
let stats = ingestor.ingest_log_lines(log_lines, "test_logs")?;
assert_eq!(stats.duplicates_filtered, 0);
```

### 6. Working with Primary/Secondary Relationships

```rust
let mut config = LogIngestionConfig::default();
config.enable_similarity_matching = true;
config.similarity_config = SimilarityConfig {
    cosine_similarity_threshold: 0.85,
    use_cosine_similarity: true,
};

let ingestor = LogIngestor::new(distribution_manager, grok_parser, config);

// First log becomes primary
let log1 = "ERROR [database] connection_pool: Timeout after 30s".to_string();
// Similar log becomes secondary
let log2 = "ERROR [database] connection_pool: Timeout after 31s".to_string();

let stats = ingestor.ingest_log_lines(vec![log1, log2], "errors")?;
assert_eq!(stats.primary_records, 1);
assert_eq!(stats.secondary_records, 1);
assert_eq!(stats.similarity_matches, 1);
```

### 7. Batch Processing with Delays

```rust
let mut config = LogIngestionConfig::default();
config.batch_size = 100;
config.batch_delay_ms = 100;  // 100ms delay between batches

let ingestor = LogIngestor::new(distribution_manager, grok_parser, config);

// Process 10,000 log lines in 100-line batches with delays
let large_log = generate_log_lines(10000);
let stats = ingestor.ingest_log_lines(large_log, "large_dataset")?;

println!("Processed {} batches", stats.batches_processed);
assert!(stats.ingestion_duration_ms >= 10000); // At least 10 seconds with delays
```

### 8. Concurrent Ingestion from Multiple Sources

```rust
use std::thread;

let ingestor = Arc::new(ingestor);
let mut handles = vec![];

for source in &["app1.log", "app2.log", "app3.log"] {
    let ingestor_clone = ingestor.clone();
    let source = source.to_string();
    
    let handle = thread::spawn(move || {
        ingestor_clone.ingest_log_file(&source, &source)
    });
    handles.push(handle);
}

for handle in handles {
    let stats = handle.join().unwrap()?;
    println!("Ingested: {:?}", stats);
}
```

### 9. Custom Grok Patterns for Specialized Logs

```rust
// Add custom pattern for RAG operations
let grok_parser = GrokLogParser::new("rag_system");
grok_parser.add_pattern(
    "rag_operation",
    r"RAG\|(?P<operation>\w+)\|query=(?P<query>.*)\|chunks=(?P<chunks>\d+)\|time=(?P<time_ms>\d+)ms"
)?;

let ingestor = LogIngestor::new(distribution_manager, grok_parser, config);

let log = "RAG|search|query=vector database|chunks=10|time=245ms".to_string();
let stats = ingestor.ingest_log_lines(vec![log], "rag_logs")?;
```

### 10. Monitoring and Statistics

```rust
let ingestor = setup_ingestor();
let stats = ingestor.ingest_log_lines(log_lines, "monitoring")?;

println!("=== Ingestion Report ===");
println!("Total lines read: {}", stats.total_lines_read);
println!("Records parsed: {}", stats.total_records_parsed);
println!("Records stored: {}", stats.total_records_stored);
println!("Failed parses: {}", stats.failed_parses);
println!("Duplicates filtered: {}", stats.duplicates_filtered);
println!("Primary records: {}", stats.primary_records);
println!("Secondary records: {}", stats.secondary_records);
println!("Similarity matches: {}", stats.similarity_matches);
println!("Shards created: {}", stats.shards_created);
println!("Duration: {}ms", stats.ingestion_duration_ms);

if stats.total_records_stored > 0 {
    let success_rate = (stats.total_records_stored as f64 / stats.total_records_parsed as f64) * 100.0;
    println!("Success rate: {:.2}%", success_rate);
}
```

## Log Format Examples

### Common Log Format
```
2024-01-15T10:30:45Z INFO [main] user_login: User logged in successfully
```

### Vector Operation Log
```
VECTOR|search|dim=1536|time=87ms|top_k=10,threshold=0.8
```

### Search Query Log
```
SEARCH|users|query=john doe|results=42|time=125ms
```

### Graph Operation Log
```
GRAPH|shortest_path|nodes=1000|edges=5000|time=345ms
```

### Bund Telemetry Format
```
2024-01-15T10:30:45Z|vector_db|similarity_search|duration=87ms|top_k=10,threshold=0.8
```

## Performance Tuning

### For High Throughput

```rust
config.batch_size = 10000;           // Larger batches
config.batch_delay_ms = 0;           // No delay between batches
config.auto_sharding = true;         // Distribute across shards
config.enable_deduplication = true;
config.bloom_filter_size = 50_000_000; // Larger Bloom filter
```

### For Memory-Constrained Environments

```rust
config.batch_size = 100;             // Smaller batches
config.batch_delay_ms = 50;          // Allow GC to catch up
config.enable_deduplication = true;
config.bloom_filter_size = 1_000_000;  // Smaller Bloom filter
```

### For Real-Time Processing

```rust
config.batch_size = 10;              // Very small batches
config.batch_delay_ms = 10;          // Minimal delay
config.auto_sharding = false;        // Single shard for speed
config.enable_deduplication = false; // Disable for speed
config.enable_similarity_matching = false;
```

## Error Handling

```rust
match ingestor.ingest_log_lines(log_lines, "test") {
    Ok(stats) => {
        println!("Successfully ingested {} records", stats.total_records_stored);
    }
    Err(e) => {
        eprintln!("Ingestion failed: {}", e);
        // Handle error appropriately
    }
}
```

## Best Practices

1. **Batch Size Selection**
   - Small batches (100-500): Real-time processing, low memory
   - Medium batches (1000-5000): Balanced performance
   - Large batches (10000+): High throughput, more memory

2. **Bloom Filter Sizing**
   - For 1M unique logs: size = 10,000,000, false_positive = 0.01
   - For 10M unique logs: size = 100,000,000, false_positive = 0.001

3. **Sharding Strategy**
   - Use auto-sharding for time-series data
   - Disable for small datasets (< 100K records)
   - Adjust interval based on query patterns

4. **Similarity Thresholds**
   - 0.9+ for exact matches
   - 0.7-0.9 for similar but not identical
   - 0.5-0.7 for loosely related

5. **Resource Management**
   - Monitor memory usage with large Bloom filters
   - Use batch delays for I/O-bound systems
   - Clean up temporary files with `delete_after_ingest`

## Troubleshooting

### Issue: High Memory Usage
**Solution**: Reduce `bloom_filter_size` or decrease `batch_size`

### Issue: Slow Ingestion
**Solution**: Increase `batch_size` or disable similarity matching

### Issue: Too Many Duplicates Missed
**Solution**: Increase `bloom_filter_size` or decrease `bloom_filter_false_positive_rate`

### Issue: No Similarity Matches
**Solution**: Lower `cosine_similarity_threshold` or switch to Euclidean distance

### Issue: Failed Downloads
**Solution**: Increase `max_retries` and `download_timeout_seconds`

## Integration Examples

### With Actix-Web REST API

```rust
#[post("/ingest")]
async fn ingest_logs(
    body: web::Json<IngestRequest>,
    ingestor: web::Data<Arc<LogIngestor>>,
) -> impl Responder {
    let stats = ingestor.ingest_log_lines(body.logs.clone(), &body.log_type);
    match stats {
        Ok(stats) => HttpResponse::Ok().json(stats),
        Err(e) => HttpResponse::InternalServerError().body(e),
    }
}
```

### With Tokio Task System

```rust
tokio::spawn(async move {
    let stats = ingestor.ingest_log_file("continuous.log", "stream");
    if let Err(e) = stats {
        eprintln!("Background ingestion failed: {}", e);
    }
});
```

## API Reference

### `LogIngestor::new()`
Creates a new log ingestor instance.

```rust
pub fn new(
    distribution_manager: Arc<RwLock<DataDistributionManager>>,
    grok_parser: GrokLogParser,
    config: LogIngestionConfig,
) -> Self
```

### `ingest_log_lines()`
Ingests log lines directly from a vector.

```rust
pub fn ingest_log_lines(
    &self,
    log_lines: Vec<String>,
    log_type: &str,
) -> Result<IngestionStats>
```

### `ingest_log_file()`
Ingests logs from a local file.

```rust
pub fn ingest_log_file(
    &self,
    file_path: &Path,
    log_type: &str,
) -> Result<IngestionStats>
```

### `ingest_from_url()`
Downloads and ingests logs from a URL.

```rust
pub fn ingest_from_url(
    &self,
    url: &str,
    log_type: &str,
) -> Result<IngestionStats>
```

## See Also

- [Grok Integration Documentation](./GROK_INTEGRATION.md)

## License

This module is part of the Bund BlobStore project and is licensed under the same terms.
```

This comprehensive documentation covers:
- Complete API reference
- Multiple usage examples
- Performance tuning guidelines
- Troubleshooting common issues
- Integration patterns
- Best practices
- Configuration options
- Statistics explanation
- Error handling patterns
