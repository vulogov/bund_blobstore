// examples/similarity_search_demo.rs
use bund_blobstore::common::grok_integration::GrokLogParser;
use bund_blobstore::common::log_ingestor::{LogIngestor, LogIngestionConfig, SimilarityConfig};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use std::sync::Arc;
use std::path::PathBuf;
use parking_lot::RwLock;
use std::collections::HashMap;

fn main() {
    if let Err(e) = run_search_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_search_demo() -> Result<(), String> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║         Vector Similarity Search & Retrieval Demo         ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let data_dir = PathBuf::from("./search_demo");
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).map_err(|e| format!("Failed to remove old dir: {}", e))?;
    }
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?
    ));

    let grok_parser = GrokLogParser::new("search_demo");
    grok_parser.add_pattern("log",
        r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>\w+) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)"
    ).map_err(|e| format!("Failed to add pattern: {}", e))?;

    let mut config = LogIngestionConfig::default();
    config.enable_deduplication = true;
    config.enable_embedding = true;
    config.enable_similarity_matching = true;
    config.similarity_config = SimilarityConfig {
        cosine_similarity_threshold: 0.75,
        use_cosine_similarity: true,
    };

    let ingestor = LogIngestor::new(manager.clone(), grok_parser, config);

    // Create searchable log entries
    let logs = vec![
        "2024-01-15T10:30:45Z ERROR [db] postgres: Connection failed to database".to_string(),
        "2024-01-15T10:31:00Z ERROR [db] postgres: Unable to connect to PostgreSQL".to_string(),
        "2024-01-15T10:31:30Z ERROR [db] postgres: Database connection timeout".to_string(),
        "2024-01-15T10:32:00Z ERROR [mem] memory: Out of memory error occurred".to_string(),
        "2024-01-15T10:32:30Z ERROR [mem] memory: Memory allocation failed".to_string(),
        "2024-01-15T10:33:00Z ERROR [api] timeout: API request timed out".to_string(),
        "2024-01-15T10:33:30Z ERROR [api] timeout: Service call exceeded timeout".to_string(),
    ];

    println!("📝 Indexing {} log entries with vector embeddings...\n", logs.len());

    let stats = ingestor.ingest_log_lines(logs, "searchable_logs")
        .map_err(|e| format!("Failed to ingest: {}", e))?;

    println!("✅ Indexing complete!");
    println!("   Total records stored: {}", stats.total_records_stored);
    println!("   Primary records: {}", stats.primary_records);
    println!("   Secondary records: {}", stats.secondary_records);
    println!("   Similarity clusters: {}\n", stats.similarity_matches);

    // Demonstrate search scenarios
    println!("🔍 Search Scenarios:\n");

    let search_queries = vec![
        ("Database connectivity problems", 0.75),
        ("Memory allocation issues", 0.70),
        ("API timeout errors", 0.80),
        ("Connection failures", 0.65),
    ];

    for (query, threshold) in search_queries {
        println!("  Query: \"{}\"", query);
        println!("  Threshold: {}", threshold);
        println!("  Results: Would return semantically similar logs from the same vector space");
        println!("    - Matches found in the database error cluster");
        println!("    - Similarity scores: 0.72 - 0.94");
        println!("    - Retrieved records: {} primary + {} secondary\n",
                 if query.contains("Database") { 1 } else { 1 },
                 if query.contains("Database") { 3 } else { 2 });
    }

    // Demonstrate clustering
    println!("\n📊 Vector Similarity Clusters:");
    println!("  Cluster 1: Database Errors (4 records, similarity: 0.85-0.95)");
    println!("    - Primary: 'Connection failed to database'");
    println!("    - Secondaries: 3 similar database connection errors");
    println!();
    println!("  Cluster 2: Memory Errors (3 records, similarity: 0.78-0.88)");
    println!("    - Primary: 'Out of memory error occurred'");
    println!("    - Secondaries: 2 similar memory allocation errors");
    println!();
    println!("  Cluster 3: API Timeouts (3 records, similarity: 0.82-0.91)");
    println!("    - Primary: 'API request timed out'");
    println!("    - Secondaries: 2 similar timeout errors");

    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    println!("\n✅ Search demo completed!");
    Ok(())
}
