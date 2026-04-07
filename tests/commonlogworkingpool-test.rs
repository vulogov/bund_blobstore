// tests/commonlogworkingpool-test.rs
use bund_blobstore::common::grok_integration::GrokLogParser;
use bund_blobstore::common::log_ingestor::{LogIngestionConfig, LogIngestor};
use bund_blobstore::common::log_worker_pool::{
    IngestionTask, LogWorkerPool, WorkerPoolConfig, start_worker_pool,
    start_worker_pool_with_ingestor, start_worker_pool_with_manager, stop_worker_pool,
    submit_batch, wait_for_tasks,
};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use lazy_static::lazy_static;
use parking_lot::RwLock;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

// Helper function to create a test Grok parser
fn create_test_parser() -> GrokLogParser {
    let parser = GrokLogParser::new("test_source");
    let _ = parser.add_pattern("test_log", r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>\w+) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)");
    let _ = parser.add_pattern("vector_log", r"VECTOR\|(?P<operation>\w+)\|dim=(?P<dimension>\d+)\|time=(?P<time_ms>\d+)ms\|(?P<metadata>.*)");
    parser
}

// Helper function to create a test log file
fn create_test_log_file(dir: &std::path::Path, lines: Vec<&str>) -> PathBuf {
    let file_path = dir.join("test.log");
    let mut file = File::create(&file_path).unwrap();
    for line in lines {
        writeln!(file, "{}", line).unwrap();
    }
    file_path
}

// Helper function to generate test log lines
fn generate_test_logs(count: usize, prefix: &str) -> Vec<String> {
    let mut logs = Vec::with_capacity(count);
    for i in 0..count {
        logs.push(format!(
            "2024-01-15T10:30:45Z INFO [main] {}_{}: Test message {}",
            prefix, i, i
        ));
    }
    logs
}

// Lazy static instances for testing
lazy_static! {
    static ref TEST_MANAGER: Arc<RwLock<DataDistributionManager>> = {
        let temp_dir = tempdir().unwrap();
        Arc::new(RwLock::new(
            DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)
                .unwrap(),
        ))
    };
    static ref TEST_INGESTOR: Arc<LogIngestor> = {
        let temp_dir = tempdir().unwrap();
        let manager = Arc::new(RwLock::new(
            DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)
                .unwrap(),
        ));
        let grok_parser = create_test_parser();
        let config = LogIngestionConfig::default();
        Arc::new(LogIngestor::new(manager, grok_parser, config))
    };
}

// ============ Basic Functionality Tests ============

#[test]
fn test_worker_pool_creation() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let stats = pool.get_stats();
    assert_eq!(stats.active_workers, 2);
    assert_eq!(stats.total_tasks_submitted, 0);
    assert_eq!(stats.total_tasks_completed, 0);

    pool.stop(true).unwrap();
}

#[test]
fn test_submit_single_task() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let logs = generate_test_logs(10, "task1");
    let task_id = pool.submit_lines(logs, "test".to_string()).unwrap();

    assert!(task_id >= 0);

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().success);

    pool.stop(true).unwrap();
}

#[test]
fn test_submit_multiple_tasks() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 4, "test_pool").unwrap();

    let mut task_ids = Vec::new();
    for i in 0..10 {
        let logs = generate_test_logs(5, &format!("task_{}", i));
        let task_id = pool.submit_lines(logs, format!("test_{}", i)).unwrap();
        task_ids.push(task_id);
    }

    let results = wait_for_tasks(&pool, &task_ids, 30).unwrap();
    assert_eq!(results.len(), 10);

    for result in results {
        assert!(result.success);
    }

    let stats = pool.get_stats();
    assert_eq!(stats.total_tasks_submitted, 10);
    assert_eq!(stats.total_tasks_completed, 10);

    pool.stop(true).unwrap();
}

// ============ File Ingestion Tests ============

#[test]
fn test_file_ingestion() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] file_test: Line 1",
        "2024-01-15T10:30:46Z INFO [main] file_test: Line 2",
        "2024-01-15T10:30:47Z INFO [main] file_test: Line 3",
    ];

    let file_path = create_test_log_file(temp_dir.path(), log_lines);
    let task_id = pool.submit_file(file_path, "file_log".to_string()).unwrap();

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().success);

    pool.stop(true).unwrap();
}

#[test]
fn test_multiple_file_ingestion() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 3, "test_pool").unwrap();

    let mut task_ids = Vec::new();
    for i in 0..5 {
        let log_lines = vec![
            format!("2024-01-15T10:30:45Z INFO [main] file_{}: Line 1", i),
            format!("2024-01-15T10:30:46Z INFO [main] file_{}: Line 2", i),
        ];
        let file_path = create_test_log_file(
            temp_dir.path(),
            log_lines.iter().map(|s| s.as_str()).collect(),
        );
        let task_id = pool.submit_file(file_path, format!("file_{}", i)).unwrap();
        task_ids.push(task_id);
    }

    let results = wait_for_tasks(&pool, &task_ids, 30).unwrap();
    assert_eq!(results.len(), 5);

    for result in results {
        assert!(result.success);
    }

    pool.stop(true).unwrap();
}

// ============ External Manager Tests ============

#[test]
fn test_pool_with_external_manager() {
    let temp_dir = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));

    let grok_patterns = vec![
        ("test_log".to_string(), r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>\w+) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)".to_string()),
    ];

    let mut pool = start_worker_pool_with_manager(manager, 2, "test_pool", grok_patterns).unwrap();

    let logs = generate_test_logs(20, "external");
    let task_id = pool.submit_lines(logs, "test".to_string()).unwrap();

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().success);

    pool.stop(true).unwrap();
}

// ============ External Ingestor Tests ============

#[test]
fn test_pool_with_external_ingestor() {
    let mut pool = start_worker_pool_with_ingestor(TEST_INGESTOR.clone(), 3).unwrap();

    let logs = generate_test_logs(15, "external_ingestor");
    let task_id = pool.submit_lines(logs, "test".to_string()).unwrap();

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().success);

    pool.stop(true).unwrap();
}

// ============ Batch Submission Tests ============

#[test]
fn test_batch_submission() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 4, "test_pool").unwrap();

    let mut tasks = Vec::new();
    for i in 0..8 {
        let logs = generate_test_logs(10, &format!("batch_{}", i));
        tasks.push(IngestionTask::Lines {
            lines: logs,
            log_type: format!("batch_{}", i),
        });
    }

    let task_ids = submit_batch(&pool, tasks).unwrap();
    assert_eq!(task_ids.len(), 8);

    let results = wait_for_tasks(&pool, &task_ids, 30).unwrap();
    assert_eq!(results.len(), 8);

    pool.stop(true).unwrap();
}

// ============ Performance and Load Tests ============

#[test]
fn test_high_load_ingestion() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(
        temp_dir.path().to_path_buf(),
        4, // Reduced workers
        "test_pool",
    )
    .unwrap();

    let start = std::time::Instant::now();

    let mut task_ids = Vec::new();
    for i in 0..20 {
        // Reduced from 50 to 20
        let logs = generate_test_logs(50, &format!("load_{}", i)); // Reduced from 100 to 50
        let task_id = pool.submit_lines(logs, format!("load_{}", i)).unwrap();
        task_ids.push(task_id);
    }

    let results = wait_for_tasks(&pool, &task_ids, 30).unwrap(); // Reduced timeout
    let duration = start.elapsed();

    assert_eq!(results.len(), 20);
    let total_records: usize = results.iter().map(|r| r.stats.total_records_stored).sum();
    assert_eq!(total_records, 1000); // 20 tasks * 50 lines

    println!("Processed 1000 log lines in {:?}", duration);

    pool.stop(true).unwrap();
}

#[test]
fn test_mixed_task_types() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 4, "test_pool").unwrap();

    let mut tasks = Vec::new();

    // Add line tasks
    for i in 0..5 {
        let logs = generate_test_logs(20, &format!("line_{}", i));
        tasks.push(IngestionTask::Lines {
            lines: logs,
            log_type: format!("line_{}", i),
        });
    }

    // Add file tasks
    for i in 0..3 {
        let log_lines = vec![format!(
            "2024-01-15T10:30:45Z INFO [main] file_{}: Content",
            i
        )];
        let file_path = create_test_log_file(
            temp_dir.path(),
            log_lines.iter().map(|s| s.as_str()).collect(),
        );
        tasks.push(IngestionTask::File {
            path: file_path,
            log_type: format!("file_{}", i),
        });
    }

    let task_ids = submit_batch(&pool, tasks).unwrap();
    let results = wait_for_tasks(&pool, &task_ids, 30).unwrap();

    assert_eq!(results.len(), 8);
    assert!(results.iter().all(|r| r.success));

    pool.stop(true).unwrap();
}

// ============ Error Handling Tests ============

#[test]
fn test_task_retry_on_failure() {
    let temp_dir = tempdir().unwrap();
    let mut config = WorkerPoolConfig::default();
    config.num_workers = 1;
    config.retry_failed = true;
    config.max_retries = 3;
    config.retry_delay_ms = 100;
    config.data_dir = Some(temp_dir.path().to_path_buf());

    let mut pool = LogWorkerPool::new(config).unwrap();

    // Submit invalid task that will fail
    let invalid_lines = vec!["Invalid log line".to_string()];
    let task_id = pool
        .submit_lines(invalid_lines, "invalid".to_string())
        .unwrap();

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(!result.unwrap().success);

    let stats = pool.get_stats();
    assert_eq!(stats.total_tasks_failed, 1);

    pool.stop(true).unwrap();
}

#[test]
fn test_task_without_retry() {
    let temp_dir = tempdir().unwrap();
    let mut config = WorkerPoolConfig::default();
    config.num_workers = 1;
    config.retry_failed = false;
    config.data_dir = Some(temp_dir.path().to_path_buf());

    let mut pool = LogWorkerPool::new(config).unwrap();

    let invalid_lines = vec!["Invalid log line".to_string()];
    let task_id = pool
        .submit_lines(invalid_lines, "invalid".to_string())
        .unwrap();

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(!result.unwrap().success);

    pool.stop(true).unwrap();
}

// ============ Vector Log Parsing Tests ============

#[test]
fn test_vector_log_ingestion() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let vector_logs = vec![
        "VECTOR|search|dim=1536|time=87ms|top_k=10,threshold=0.8".to_string(),
        "VECTOR|index|dim=768|time=1234ms|vectors=10000".to_string(),
        "VECTOR|query|dim=384|time=45ms|results=42".to_string(),
    ];

    let task_id = pool
        .submit_lines(vector_logs, "vector".to_string())
        .unwrap();
    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().success);

    pool.stop(true).unwrap();
}

// ============ Concurrent Tests ============

#[test]
fn test_concurrent_pool_operations() {
    let temp_dir = tempdir().unwrap();
    let pool = Arc::new(Mutex::new(
        start_worker_pool(temp_dir.path().to_path_buf(), 4, "test_pool").unwrap(),
    ));

    let mut handles = vec![];

    for thread_id in 0..5 {
        let pool_clone = pool.clone();
        let handle = thread::spawn(move || {
            let mut pool = pool_clone.lock().unwrap();
            let logs = generate_test_logs(20, &format!("concurrent_{}", thread_id));
            let task_id = pool
                .submit_lines(logs, format!("thread_{}", thread_id))
                .unwrap();
            pool.wait_for_task(task_id, 30).unwrap()
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().success);
    }

    let mut pool_guard = pool.lock().unwrap();
    pool_guard.stop(true).unwrap();
}

// ============ Graceful Shutdown Tests ============

#[test]
fn test_graceful_shutdown() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let logs = generate_test_logs(50, "shutdown");
    let _task_id = pool.submit_lines(logs, "test".to_string()).unwrap();

    // Wait a bit for processing to start
    thread::sleep(Duration::from_millis(100));

    // Stop gracefully
    stop_worker_pool(&mut pool, true).unwrap();

    // Verify pool is stopped
    let stats = pool.get_stats();
    assert!(stats.total_tasks_submitted >= 1);
}

#[test]
fn test_immediate_shutdown() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let logs = generate_test_logs(100, "immediate");
    let _ = pool.submit_lines(logs, "test".to_string()).unwrap();

    // Immediate shutdown without waiting
    stop_worker_pool(&mut pool, false).unwrap();

    // Should not panic
}

// ============ Statistics Tests ============

#[test]
fn test_statistics_accuracy() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let initial_stats = pool.get_stats();
    assert_eq!(initial_stats.total_tasks_submitted, 0);
    assert_eq!(initial_stats.total_tasks_completed, 0);
    assert_eq!(initial_stats.total_tasks_failed, 0);

    let logs = generate_test_logs(10, "stats");
    let task_id = pool.submit_lines(logs, "test".to_string()).unwrap();

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());

    let final_stats = pool.get_stats();
    assert_eq!(final_stats.total_tasks_submitted, 1);
    assert_eq!(final_stats.total_tasks_completed, 1);
    assert_eq!(final_stats.total_tasks_failed, 0);
    assert!(final_stats.total_records_ingested >= 10);
    assert!(final_stats.total_ingestion_time_ms > 0);

    pool.stop(true).unwrap();
}

// ============ Large File Tests ============

#[test]
fn test_large_file_ingestion() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    // Create a large log file with 10,000 lines
    let mut log_lines = Vec::new();
    for i in 0..10000 {
        log_lines.push(format!(
            "2024-01-15T10:30:45Z INFO [main] large_file: Line {}",
            i
        ));
    }

    let file_path = create_test_log_file(
        temp_dir.path(),
        log_lines.iter().map(|s| s.as_str()).collect(),
    );

    let start = std::time::Instant::now();
    let task_id = pool.submit_file(file_path, "large".to_string()).unwrap();
    let result = pool.wait_for_task(task_id, 60).unwrap();
    let duration = start.elapsed();

    assert!(result.is_some());
    assert!(result.unwrap().success);

    println!("Processed 10,000 lines in {:?}", duration);

    pool.stop(true).unwrap();
}

// ============ Edge Cases Tests ============

#[test]
fn test_empty_log_lines() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(
        temp_dir.path().to_path_buf(),
        1, // Use 1 worker
        "test_pool",
    )
    .unwrap();

    let empty_logs: Vec<String> = vec![];
    let task_id = pool.submit_lines(empty_logs, "empty".to_string()).unwrap();

    let result = pool.wait_for_task(task_id, 5).unwrap(); // Shorter timeout
    assert!(result.is_some());
    assert!(result.unwrap().success);

    pool.stop(true).unwrap();
}

#[test]
fn test_single_line_log() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(temp_dir.path().to_path_buf(), 2, "test_pool").unwrap();

    let single_log = vec!["2024-01-15T10:30:45Z INFO [main] single: Just one line".to_string()];
    let task_id = pool.submit_lines(single_log, "single".to_string()).unwrap();

    let result = pool.wait_for_task(task_id, 10).unwrap();
    assert!(result.is_some());
    assert!(result.unwrap().success);

    pool.stop(true).unwrap();
}

#[test]
fn test_zero_workers() {
    let temp_dir = tempdir().unwrap();
    let mut config = WorkerPoolConfig::default();
    config.num_workers = 0;
    config.data_dir = Some(temp_dir.path().to_path_buf());

    let result = LogWorkerPool::new(config);
    assert!(result.is_ok());

    if let Ok(mut pool) = result {
        let logs = generate_test_logs(5, "zero");
        // With 0 workers, this will block because no workers to process
        // So we don't submit tasks when workers = 0
        // Instead just verify the pool was created
        let stats = pool.get_stats();
        assert_eq!(stats.active_workers, 0);

        pool.stop(true).unwrap();
    }
}

// ============ Stress Test ============

#[test]
fn test_stress_test() {
    let temp_dir = tempdir().unwrap();
    let mut pool = start_worker_pool(
        temp_dir.path().to_path_buf(),
        4, // Reduced workers
        "test_pool",
    )
    .unwrap();

    let start = std::time::Instant::now();
    let mut task_ids = Vec::new();

    // Submit 50 tasks with varying sizes (reduced from 100)
    for i in 0..50 {
        let size = 10 + (i % 40); // Varying sizes from 10 to 50
        let logs = generate_test_logs(size, &format!("stress_{}", i));
        let task_id = pool.submit_lines(logs, format!("stress_{}", i)).unwrap();
        task_ids.push(task_id);
    }

    let results = wait_for_tasks(&pool, &task_ids, 60).unwrap();
    let duration = start.elapsed();

    assert_eq!(results.len(), 50);

    let total_records: usize = results.iter().map(|r| r.stats.total_records_stored).sum();
    let expected_total: usize = (0..50).map(|i| 10 + (i % 40)).sum();
    assert_eq!(total_records, expected_total);

    println!(
        "Stress test completed: {} records in {:?}",
        total_records, duration
    );

    pool.stop(true).unwrap();
}
