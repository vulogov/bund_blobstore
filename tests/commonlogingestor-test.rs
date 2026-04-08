// tests/logingestor-test.rs
use bund_blobstore::common::grok_integration::GrokLogParser;
use bund_blobstore::common::log_ingestor::{LogIngestionConfig, LogIngestor};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use tempfile::tempdir;

fn create_test_parser() -> GrokLogParser {
    let parser = GrokLogParser::new("test_source");
    let _ = parser.add_pattern("test_log", r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>\w+) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)");
    let _ = parser.add_pattern("vector_log", r"VECTOR\|(?P<operation>\w+)\|dim=(?P<dimension>\d+)\|time=(?P<time_ms>\d+)ms\|(?P<metadata>.*)");
    let _ = parser.add_pattern("search_log", r"SEARCH\|(?P<index>\w+)\|query=(?P<query>.*)\|results=(?P<results>\d+)\|time=(?P<time_ms>\d+)ms");
    parser
}

fn create_test_log_file(dir: &std::path::Path, lines: Vec<&str>) -> std::path::PathBuf {
    let file_path = dir.join("test.log");
    let mut file = File::create(&file_path).unwrap();
    for line in lines {
        writeln!(file, "{}", line).unwrap();
    }
    file_path
}

#[test]
fn test_basic_ingestion() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
        "2024-01-15T10:30:46Z ERROR [worker] db_error: Connection failed".to_string(),
        "2024-01-15T10:30:47Z DEBUG [cache] cache_hit: Key found".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_lines_read, 3);
    assert_eq!(stats.total_records_parsed, 3);
    assert_eq!(stats.total_records_stored, 3);
    assert_eq!(stats.failed_parses, 0);
    assert!(stats.ingestion_duration_ms > 0);
}

#[test]
fn test_deduplication() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let mut config = LogIngestionConfig::default();
    config.enable_deduplication = true;
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
        "2024-01-15T10:30:46Z ERROR [worker] db_error: Connection failed".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_lines_read, 4);
    assert_eq!(stats.total_records_parsed, 4);
    assert_eq!(stats.duplicates_filtered, 2);
    assert_eq!(stats.total_records_stored, 2);
}

#[test]
fn test_deduplication_disabled() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let mut config = LogIngestionConfig::default();
    config.enable_deduplication = false;
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_lines_read, 2);
    assert_eq!(stats.total_records_parsed, 2);
    assert_eq!(stats.duplicates_filtered, 0);
    assert_eq!(stats.total_records_stored, 2);
}

#[test]
fn test_batch_processing() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let mut config = LogIngestionConfig::default();
    config.batch_size = 2;
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] event1: First".to_string(),
        "2024-01-15T10:30:46Z INFO [main] event2: Second".to_string(),
        "2024-01-15T10:30:47Z INFO [main] event3: Third".to_string(),
        "2024-01-15T10:30:48Z INFO [main] event4: Fourth".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_lines_read, 4);
    assert_eq!(stats.total_records_parsed, 4);
    assert_eq!(stats.total_records_stored, 4);
    assert!(stats.batches_processed >= 2);
}

#[test]
fn test_auto_sharding() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let mut config = LogIngestionConfig::default();
    config.auto_sharding = true;
    config.shard_interval_seconds = 3600;
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] event1: First".to_string(),
        "2024-01-15T11:30:45Z INFO [main] event2: Second".to_string(),
        "2024-01-15T12:30:45Z INFO [main] event3: Third".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_records_stored, 3);
    assert!(stats.shards_created >= 1 && stats.shards_created <= 3);
}

#[test]
fn test_auto_sharding_disabled() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let mut config = LogIngestionConfig::default();
    config.auto_sharding = false;
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] event1: First".to_string(),
        "2024-01-15T11:30:45Z INFO [main] event2: Second".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_records_stored, 2);
    assert_eq!(stats.shards_created, 1);
}

#[test]
fn test_vector_log_parsing() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "VECTOR|search|dim=1536|time=87ms|top_k=10,threshold=0.8".to_string(),
        "VECTOR|index|dim=768|time=1234ms|vectors=10000".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "vector_logs").unwrap();

    assert_eq!(stats.total_lines_read, 2);
    assert_eq!(stats.total_records_parsed, 2);
    assert_eq!(stats.total_records_stored, 2);
}

#[test]
fn test_search_log_parsing() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "SEARCH|users|query=john doe|results=42|time=125ms".to_string(),
        "SEARCH|documents|query=vector search|results=10|time=89ms".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "search_logs").unwrap();

    assert_eq!(stats.total_lines_read, 2);
    assert_eq!(stats.total_records_parsed, 2);
    assert_eq!(stats.total_records_stored, 2);
}

#[test]
fn test_mixed_log_types() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
        "VECTOR|search|dim=1536|time=87ms|top_k=10".to_string(),
        "SEARCH|users|query=test|results=5|time=45ms".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "mixed_logs").unwrap();

    assert_eq!(stats.total_lines_read, 3);
    assert_eq!(stats.total_records_parsed, 3);
    assert_eq!(stats.total_records_stored, 3);
}

#[test]
fn test_failed_parsing() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "Invalid log line".to_string(),
        "2024-01-15T10:30:45Z INFO [main] valid: This is valid".to_string(),
        "Another invalid line".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_lines_read, 3);
    assert_eq!(stats.total_records_parsed, 1);
    assert_eq!(stats.failed_parses, 2);
    assert_eq!(stats.total_records_stored, 1);
}

#[test]
fn test_ingest_from_log_file() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] line1: First line",
        "2024-01-15T10:30:46Z ERROR [worker] line2: Second line",
        "2024-01-15T10:30:47Z DEBUG [cache] line3: Third line",
    ];

    let file_path = create_test_log_file(temp_dir.path(), log_lines);
    let stats = ingestor.ingest_log_file(&file_path, "file_logs").unwrap();

    assert_eq!(stats.total_lines_read, 3);
    assert_eq!(stats.total_records_parsed, 3);
    assert_eq!(stats.total_records_stored, 3);
}

#[test]
fn test_concurrent_ingestion() {
    use std::thread;

    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let config = LogIngestionConfig::default();

    let mut handles = vec![];

    for thread_id in 0..5 {
        let dm = distribution_manager.clone();
        let config_clone = config.clone();

        let handle = thread::spawn(move || {
            let parser = create_test_parser();
            let ingestor = LogIngestor::new(dm, parser, config_clone);
            let mut log_lines = Vec::new();
            for i in 0..100 {
                log_lines.push(format!(
                    "2024-01-15T10:30:45Z INFO [main] thread_{}_event_{}: Test",
                    thread_id, i
                ));
            }
            ingestor
                .ingest_log_lines(log_lines, &format!("concurrent_{}", thread_id))
                .unwrap()
        });
        handles.push(handle);
    }

    let mut total_stored = 0;
    for handle in handles {
        let stats = handle.join().unwrap();
        total_stored += stats.total_records_stored;
    }

    assert_eq!(total_stored, 500);
}

#[test]
fn test_large_batch_processing() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let mut config = LogIngestionConfig::default();
    config.batch_size = 100;
    config.batch_delay_ms = 10;
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let mut log_lines = Vec::new();
    for i in 0..500 {
        log_lines.push(format!(
            "2024-01-15T10:30:45Z INFO [main] event_{}: Test event {}",
            i, i
        ));
    }

    let stats = ingestor.ingest_log_lines(log_lines, "large_batch").unwrap();

    assert_eq!(stats.total_lines_read, 500);
    assert_eq!(stats.total_records_parsed, 500);
    assert_eq!(stats.total_records_stored, 500);
    assert!(stats.batches_processed >= 5);
}

#[test]
fn test_empty_log_lines() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines: Vec<String> = vec![];
    let stats = ingestor.ingest_log_lines(log_lines, "empty_logs").unwrap();

    assert_eq!(stats.total_lines_read, 0);
    assert_eq!(stats.total_records_parsed, 0);
    assert_eq!(stats.total_records_stored, 0);
    assert_eq!(stats.batches_processed, 0);
}

#[test]
fn test_statistics_accuracy() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let mut config = LogIngestionConfig::default();
    config.enable_deduplication = true;
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] valid1: First valid".to_string(),
        "Invalid line that won't parse".to_string(),
        "2024-01-15T10:30:46Z INFO [main] valid2: Second valid".to_string(),
        "2024-01-15T10:30:45Z INFO [main] valid1: First valid".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "stats_test").unwrap();

    assert_eq!(stats.total_lines_read, 4);
    assert_eq!(stats.total_records_parsed, 3);
    assert_eq!(stats.failed_parses, 1);
    assert_eq!(stats.duplicates_filtered, 1);
    assert_eq!(stats.total_records_stored, 2);
}

#[test]
fn test_ingest_log_lines_with_embeddings() {
    let temp_dir = tempdir().unwrap();
    let distribution_manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let parser = create_test_parser();
    let config = LogIngestionConfig::default();
    let ingestor = LogIngestor::new(distribution_manager, parser, config);

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] user_login: User logged in".to_string(),
        "2024-01-15T10:30:46Z ERROR [worker] db_error: Connection failed".to_string(),
    ];

    let stats = ingestor.ingest_log_lines(log_lines, "test_logs").unwrap();

    assert_eq!(stats.total_lines_read, 2);
    assert_eq!(stats.total_records_parsed, 2);
    assert!(stats.total_records_stored >= 2);
}
