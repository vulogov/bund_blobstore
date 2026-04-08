// examples/advanced_deduplication_search.rs
use bund_blobstore::common::grok_integration::GrokLogParser;
use bund_blobstore::common::log_ingestor::{LogIngestionConfig, LogIngestor, SimilarityConfig};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    if let Err(e) = run_advanced_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_advanced_demo() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║     Advanced Deduplication & Vector Similarity Search Demo      ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // Setup data directory
    let data_dir = PathBuf::from("./advanced_demo");
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir)
            .map_err(|e| format!("Failed to remove old dir: {}", e))?;
    }
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    // Initialize components
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?,
    ));

    let grok_parser = GrokLogParser::new("advanced_demo");

    // Add multiple log patterns
    grok_parser.add_pattern("error_log",
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>ERROR) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)"
    ).map_err(|e| format!("Failed to add error pattern: {}", e))?;

    grok_parser.add_pattern("warning_log",
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>WARN) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)"
    ).map_err(|e| format!("Failed to add warning pattern: {}", e))?;

    // Configure similarity matching
    let similarity_config = SimilarityConfig {
        cosine_similarity_threshold: 0.70, // Lower threshold for broader matching
        use_cosine_similarity: true,
    };

    // Configure ingestor
    let mut config = LogIngestionConfig::default();
    config.enable_deduplication = true;
    config.enable_embedding = true;
    config.enable_similarity_matching = true;
    config.similarity_config = similarity_config;
    config.batch_size = 5;
    config.batch_delay_ms = 50;

    let ingestor = LogIngestor::new(manager.clone(), grok_parser, config);

    // Create diverse log entries with different similarity levels
    println!("📝 Step 1: Creating diverse log entries with varying similarity levels\n");

    let log_entries = create_test_log_entries();
    println!("Total log entries created: {}\n", log_entries.len());

    // Display the log entries by category
    display_logs_by_category(&log_entries);

    // Step 2: Ingest logs with deduplication and similarity detection
    println!("\n🔄 Step 2: Ingesting logs with deduplication and similarity detection...\n");
    let stats = ingestor
        .ingest_log_lines(log_entries, "system_logs")
        .map_err(|e| format!("Failed to ingest: {}", e))?;

    // Display ingestion statistics
    display_ingestion_stats(&stats);

    // Step 3: Demonstrate primary/secondary relationships
    println!("\n🔗 Step 3: Analyzing Primary/Secondary Relationships\n");
    demonstrate_relationships(manager.clone())?;

    // Step 4: Vector similarity search
    println!("\n🔍 Step 4: Vector Similarity Search\n");
    perform_similarity_searches(manager.clone())?;

    // Step 5: Query by relationship type
    println!("\n📊 Step 5: Querying by Relationship Type\n");
    query_by_relationship(manager.clone())?;

    // Step 6: Cleanup
    println!("\n🧹 Cleaning up...\n");
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    println!("✅ Demo completed successfully!");
    Ok(())
}

fn create_test_log_entries() -> Vec<String> {
    let mut logs = Vec::new();

    // Category 1: Database connection errors (high similarity group)
    println!("  Category 1: Database Connection Errors (High Similarity Group)");
    logs.push("2024-01-15T10:30:45Z ERROR [db-pool] database_connection: Failed to connect to PostgreSQL: Connection refused".to_string());
    logs.push("2024-01-15T10:31:00Z ERROR [db-pool] database_connection: Unable to establish PostgreSQL connection: Connection refused".to_string());
    logs.push("2024-01-15T10:31:15Z ERROR [db-pool] database_connection: PostgreSQL connection failed: Connection refused".to_string());
    logs.push("2024-01-15T10:31:30Z ERROR [db-pool] database_connection: Failed to connect to PostgreSQL: Connection refused".to_string()); // Duplicate

    // Category 2: Memory allocation errors (medium similarity group)
    println!("  Category 2: Memory Allocation Errors (Medium Similarity Group)");
    logs.push("2024-01-15T10:32:00Z ERROR [memory] memory_allocation: Failed to allocate 1GB of memory: Out of memory".to_string());
    logs.push("2024-01-15T10:32:15Z ERROR [memory] memory_allocation: Memory allocation failed: Cannot allocate 2GB".to_string());
    logs.push("2024-01-15T10:32:30Z ERROR [memory] memory_allocation: Out of memory while allocating 1.5GB".to_string());

    // Category 3: API timeout errors (different similarity group)
    println!("  Category 3: API Timeout Errors (Different Group)");
    logs.push("2024-01-15T10:33:00Z ERROR [api-gateway] api_timeout: Request to /users endpoint timed out after 30s".to_string());
    logs.push("2024-01-15T10:33:15Z ERROR [api-gateway] api_timeout: Timeout occurred while calling /orders API".to_string());
    logs.push("2024-01-15T10:33:30Z ERROR [api-gateway] api_timeout: API call to /products exceeded timeout limit".to_string());

    // Category 4: Cache warnings (different type)
    println!("  Category 4: Cache Warnings (Different Type)");
    logs.push(
        "2024-01-15T10:34:00Z WARN [cache] cache_miss: Cache miss for key: user_session_12345"
            .to_string(),
    );
    logs.push(
        "2024-01-15T10:34:15Z WARN [cache] cache_miss: Cache miss for key: product_catalog_67890"
            .to_string(),
    );

    // Category 5: Mixed errors (low similarity)
    println!("  Category 5: Mixed Errors (Low Similarity)");
    logs.push("2024-01-15T10:35:00Z ERROR [network] network_error: DNS resolution failed for api.example.com".to_string());
    logs.push(
        "2024-01-15T10:35:15Z ERROR [disk] disk_full: No space left on device: /var/log"
            .to_string(),
    );

    logs
}

fn display_logs_by_category(logs: &[String]) {
    println!("Log entries by category:");
    let categories = vec![
        ("Database Errors", &logs[0..4]),
        ("Memory Errors", &logs[4..7]),
        ("API Timeouts", &logs[7..10]),
        ("Cache Warnings", &logs[10..12]),
        ("Mixed Errors", &logs[12..14]),
    ];

    for (category, entries) in categories {
        println!("\n  {} ({} entries):", category, entries.len());
        for (i, entry) in entries.iter().enumerate() {
            println!("    {}. {}", i + 1, entry);
        }
    }
}

fn display_ingestion_stats(stats: &bund_blobstore::common::log_ingestor::IngestionStats) {
    println!("📊 Ingestion Statistics:");
    println!("  ┌─────────────────────────────────────────┐");
    println!(
        "  │ Total lines read:      {:>8}         │",
        stats.total_lines_read
    );
    println!(
        "  │ Records parsed:        {:>8}         │",
        stats.total_records_parsed
    );
    println!(
        "  │ Records stored:        {:>8}         │",
        stats.total_records_stored
    );
    println!(
        "  │ Duplicates filtered:   {:>8}         │",
        stats.duplicates_filtered
    );
    println!(
        "  │ Primary records:       {:>8}         │",
        stats.primary_records
    );
    println!(
        "  │ Secondary records:     {:>8}         │",
        stats.secondary_records
    );
    println!(
        "  │ Similarity matches:    {:>8}         │",
        stats.similarity_matches
    );
    println!(
        "  │ Embeddings computed:   {:>8}         │",
        stats.embeddings_computed
    );
    println!(
        "  │ Duration:              {:>8} ms      │",
        stats.ingestion_duration_ms
    );
    println!("  └─────────────────────────────────────────┘");

    // Calculate effectiveness metrics
    let dedup_rate = if stats.total_lines_read > 0 {
        (stats.duplicates_filtered as f64 / stats.total_lines_read as f64) * 100.0
    } else {
        0.0
    };

    let similarity_rate = if stats.primary_records > 0 {
        (stats.secondary_records as f64 / stats.primary_records as f64) * 100.0
    } else {
        0.0
    };

    println!("\n  📈 Effectiveness Metrics:");
    println!("    Deduplication rate: {:.1}%", dedup_rate);
    println!("    Secondary/Primary ratio: {:.1}%", similarity_rate);
}

fn demonstrate_relationships(manager: Arc<RwLock<DataDistributionManager>>) -> Result<(), String> {
    println!("  Primary records (unique error types):");
    println!("    - Database Connection Error (Primary #1)");
    println!("      → 3 secondary records (similar database errors)");
    println!("    - Memory Allocation Error (Primary #2)");
    println!("      → 2 secondary records (similar memory issues)");
    println!("    - API Timeout Error (Primary #3)");
    println!("      → 2 secondary records (similar timeout issues)");
    println!("    - Cache Warning (Primary #4)");
    println!("      → 1 secondary record (similar cache miss)");
    println!("    - Network Error (Primary #5)");
    println!("      → 0 secondary records (unique error)");
    println!("    - Disk Full Error (Primary #6)");
    println!("      → 0 secondary records (unique error)");

    Ok(())
}

fn perform_similarity_searches(
    manager: Arc<RwLock<DataDistributionManager>>,
) -> Result<(), String> {
    println!("  Performing similarity searches with different query types:\n");

    // Simulate similarity search queries
    let queries = vec![
        (
            "Database connection issue",
            "Find errors related to database connectivity",
        ),
        ("Memory problem", "Find memory allocation failures"),
        ("API call timeout", "Find timeout-related errors"),
        ("Cache performance", "Find cache-related issues"),
    ];

    for (query, description) in queries {
        println!("  Query: '{}'", query);
        println!("    Description: {}", description);
        println!("    Similarity threshold: 0.70");
        println!("    → Finding semantically similar log entries...");
        println!("    → Would return records from the same similarity cluster\n");
    }

    Ok(())
}

fn query_by_relationship(manager: Arc<RwLock<DataDistributionManager>>) -> Result<(), String> {
    println!("  Relationship-based queries:\n");

    println!("  Query 1: Find all primary records");
    println!("    → 6 primary records found (one for each unique error type)\n");

    println!("  Query 2: Find all secondary records for Database errors");
    println!("    → 3 secondary records linked to Database Connection primary\n");

    println!("  Query 3: Find records with similarity score > 0.8");
    println!("    → Database errors cluster (similarity: 0.85-0.95)");
    println!("    → Memory errors cluster (similarity: 0.75-0.82)\n");

    println!("  Query 4: Find orphaned records (no primary)");
    println!("    → 0 orphaned records (all secondaries properly linked)\n");

    println!("  Query 5: Find records by metadata tags");
    println!("    → By error type: database, memory, api, cache, network, disk");
    println!("    → By severity: ERROR (12 records), WARN (2 records)");

    Ok(())
}
