// examples/deduplication_demo.rs - Fixed version
use bund_blobstore::common::grok_integration::GrokLogParser;
use bund_blobstore::common::log_ingestor::{LogIngestionConfig, LogIngestor, SimilarityConfig};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    if let Err(e) = run_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_demo() -> Result<(), String> {
    println!("=== Telemetry Deduplication and Similarity Detection Demo ===\n");

    // Create data directory
    let data_dir = PathBuf::from("./demo_data");
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))?;
    }

    // Initialize the distribution manager
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?,
    ));

    // Initialize Grok parser with common log patterns
    let grok_parser = GrokLogParser::new("demo_app");

    // Add pattern for error logs
    grok_parser.add_pattern("error_log",
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>ERROR) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)"
    ).map_err(|e| format!("Failed to add pattern: {}", e))?;

    // Configure similarity detection
    let similarity_config = SimilarityConfig {
        cosine_similarity_threshold: 0.75,
        use_cosine_similarity: true,
    };

    // Configure the ingestor
    let mut ingestor_config = LogIngestionConfig::default();
    ingestor_config.enable_deduplication = true;
    ingestor_config.enable_embedding = true;
    ingestor_config.enable_similarity_matching = true;
    ingestor_config.similarity_config = similarity_config;
    ingestor_config.batch_size = 10;
    ingestor_config.batch_delay_ms = 100;

    // Create the ingestor
    let ingestor = LogIngestor::new(distribution_manager.clone(), grok_parser, ingestor_config);

    println!("Creating test log entries with duplicates and similar patterns...\n");

    // Create test log entries with duplicates and similar errors
    let test_logs = vec![
        // Primary error - Database connection issue
        "2024-01-15T10:30:45Z ERROR [db-pool] database_connection: Failed to connect to database: Connection timeout after 30 seconds".to_string(),

        // Similar error - Same issue, slightly different wording
        "2024-01-15T10:31:00Z ERROR [db-pool] database_connection: Unable to establish database connection: Timeout occurred".to_string(),

        // Very similar error - Almost identical
        "2024-01-15T10:31:15Z ERROR [db-pool] database_connection: Database connection failed: Connection timeout".to_string(),

        // Different error - Memory issue (should be separate primary)
        "2024-01-15T10:32:00Z ERROR [memory] memory_allocation: Failed to allocate 1GB of memory: Out of memory".to_string(),

        // Similar memory error (should be secondary to above)
        "2024-01-15T10:32:30Z ERROR [memory] memory_allocation: Memory allocation failed: Cannot allocate 1GB".to_string(),

        // Duplicate of the first error (should be deduplicated)
        "2024-01-15T10:33:00Z ERROR [db-pool] database_connection: Failed to connect to database: Connection timeout after 30 seconds".to_string(),
    ];

    println!("Total log entries: {}\n", test_logs.len());
    for (i, log) in test_logs.iter().enumerate() {
        println!("{}. {}", i + 1, log);
    }
    println!("\n");

    // Ingest the logs
    println!("Ingesting logs with deduplication and similarity detection...\n");
    let stats = ingestor
        .ingest_log_lines(test_logs, "demo_logs")
        .map_err(|e| format!("Failed to ingest: {}", e))?;

    // Display statistics
    println!("=== Ingestion Statistics ===");
    println!("Total lines read: {}", stats.total_lines_read);
    println!("Records parsed: {}", stats.total_records_parsed);
    println!("Records stored: {}", stats.total_records_stored);
    println!("Duplicates filtered: {}", stats.duplicates_filtered);
    println!("Primary records: {}", stats.primary_records);
    println!("Secondary records: {}", stats.secondary_records);
    println!("Similarity matches: {}", stats.similarity_matches);
    println!("Embeddings computed: {}", stats.embeddings_computed);
    println!("Duration: {}ms\n", stats.ingestion_duration_ms);

    // Demonstrate effectiveness
    println!("=== Analysis ===");
    println!("Deduplication effectiveness:");
    println!("  Original entries: {}", stats.total_lines_read);
    println!("  After deduplication: {}", stats.total_records_stored);
    println!("  Duplicates removed: {}", stats.duplicates_filtered);

    if stats.total_lines_read > 0 {
        let reduction = (stats.duplicates_filtered as f64 / stats.total_lines_read as f64) * 100.0;
        println!("  Reduction rate: {:.1}%\n", reduction);
    }

    println!("Similarity matching effectiveness:");
    println!("  Primary records created: {}", stats.primary_records);
    println!("  Secondary records linked: {}", stats.secondary_records);
    println!("  Similarity matches found: {}", stats.similarity_matches);

    if stats.primary_records > 0 {
        let avg_secondaries = stats.secondary_records as f64 / stats.primary_records as f64;
        println!(
            "  Average secondaries per primary: {:.1}\n",
            avg_secondaries
        );
    }

    // Cleanup
    println!("Cleaning up...\n");
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    println!("Demo completed successfully!");
    Ok(())
}
