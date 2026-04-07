```markdown
# Log Worker Pool Documentation

## Overview

The `LogWorkerPool` module provides a high-performance, multi-threaded worker pool for ingesting log files into the Bund BlobStore telemetry system. It features configurable worker threads, automatic task distribution, retry logic, and comprehensive statistics tracking.

## Features

- **Multi-threaded Processing** - Configurable number of worker threads for parallel log ingestion
- **Three Initialization Modes** - Own resources, external manager, or external ingestor
- **Task Types** - File, URL, and direct line ingestion
- **Automatic Retry** - Configurable retry logic with exponential backoff
- **Task Tracking** - Unique task IDs with result waiting
- **Batch Operations** - Submit and wait for multiple tasks
- **Graceful Shutdown** - Controlled shutdown with timeout
- **Comprehensive Statistics** - Track all pool operations
- **Thread-Safe** - Safe for concurrent task submission

## Quick Start

```rust
use bund_blobstore::common::log_worker_pool::start_worker_pool;
use std::path::PathBuf;

// Create a worker pool with 4 workers
let mut pool = start_worker_pool(
    PathBuf::from("/data/bund"),
    4,
    "my_application"
)?;

// Submit a task
let task_id = pool.submit_lines(
    vec!["2024-01-15T10:30:45Z INFO [main] test: Message".to_string()],
    "app_logs".to_string()
)?;

// Wait for completion
if let Some(result) = pool.wait_for_task(task_id, 30)? {
    println!("Task completed in {}ms", result.duration_ms);
}

// Shutdown
pool.stop(true)?;
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["log-worker-pool"] }
```

## Core Components

### WorkerPoolConfig

Configuration structure for the worker pool:

```rust
pub struct WorkerPoolConfig {
    pub num_workers: usize,              // Number of worker threads
    pub queue_capacity: usize,           // Task queue capacity (0 for unbounded)
    pub retry_failed: bool,              // Retry failed tasks
    pub max_retries: u32,                // Maximum retry attempts
    pub retry_delay_ms: u64,             // Delay between retries in milliseconds
    pub shutdown_timeout_seconds: u64,   // Timeout for graceful shutdown
    pub grok_patterns: Vec<(String, String)>, // Custom Grok patterns
    pub default_source: String,          // Default source for logs
    pub distribution_strategy: DistributionStrategy, // Sharding strategy
    pub data_dir: Option<PathBuf>,       // Data directory for own resources
}
```

### PoolStats

Statistics collected during pool operation:

```rust
pub struct PoolStats {
    pub total_tasks_submitted: u64,      // Total tasks submitted
    pub total_tasks_completed: u64,      // Total tasks completed
    pub total_tasks_failed: u64,         // Total tasks failed
    pub total_tasks_retried: u64,        // Total retry attempts
    pub active_workers: usize,           // Currently active workers
    pub queue_size: usize,               // Pending tasks in queue
    pub total_records_ingested: u64,     // Total records stored
    pub total_ingestion_time_ms: u64,    // Total processing time
}
```

### IngestionTask

Task types that can be submitted:

```rust
pub enum IngestionTask {
    File { path: PathBuf, log_type: String },
    Url { url: String, log_type: String },
    Lines { lines: Vec<String>, log_type: String },
}
```

### TaskResult

Result returned after task completion:

```rust
pub struct TaskResult {
    pub task_id: u64,
    pub stats: IngestionStats,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}
```

## Usage Examples

### 1. Basic Worker Pool with Own Resources

```rust
use bund_blobstore::common::log_worker_pool::start_worker_pool;
use std::path::PathBuf;

let mut pool = start_worker_pool(
    PathBuf::from("/var/lib/bund"),
    4,  // 4 worker threads
    "production"
)?;

// Submit multiple tasks
for i in 0..10 {
    let logs = vec![
        format!("2024-01-15T10:30:45Z INFO [main] event_{}: Started", i),
        format!("2024-01-15T10:30:46Z INFO [main] event_{}: Completed", i),
    ];
    let task_id = pool.submit_lines(logs, format!("events_{}", i))?;
    println!("Submitted task {}", task_id);
}

// Wait for all tasks
let stats = pool.get_stats();
println!("Tasks completed: {}/{}", stats.total_tasks_completed, stats.total_tasks_submitted);

pool.stop(true)?;
```

### 2. Using External DataDistributionManager

```rust
use bund_blobstore::common::log_worker_pool::start_worker_pool_with_manager;
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use std::sync::Arc;
use parking_lot::RwLock;

// Create shared manager
let manager = Arc::new(RwLock::new(
    DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin)?
));

// Define custom Grok patterns
let grok_patterns = vec![
    ("custom_log".to_string(), r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>\w+) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)".to_string()),
];

// Create pool with external manager
let mut pool = start_worker_pool_with_manager(
    manager,
    8,  // 8 workers
    "shared_app",
    grok_patterns,
)?;

// Submit tasks (they'll use the shared manager)
pool.submit_file("app.log".into(), "application".to_string())?;

pool.stop(true)?;
```

### 3. Using External LogIngestor

```rust
use bund_blobstore::common::log_worker_pool::start_worker_pool_with_ingestor;
use bund_blobstore::common::log_ingestor::{LogIngestor, LogIngestionConfig};
use bund_blobstore::common::grok_integration::GrokLogParser;

// Create shared ingestor
let grok_parser = GrokLogParser::new("my_app");
let config = LogIngestionConfig::default();
let ingestor = Arc::new(LogIngestor::new(manager, grok_parser, config));

// Create pool with external ingestor
let mut pool = start_worker_pool_with_ingestor(ingestor, 6)?;

// Submit tasks
pool.submit_url("https://example.com/logs/app.log".into(), "remote".to_string())?;

pool.stop(true)?;
```

### 4. Batch Task Submission

```rust
use bund_blobstore::common::log_worker_pool::{submit_batch, wait_for_tasks, IngestionTask};

let mut tasks = Vec::new();

// Add file tasks
tasks.push(IngestionTask::File {
    path: "app1.log".into(),
    log_type: "app1".to_string(),
});

tasks.push(IngestionTask::File {
    path: "app2.log".into(),
    log_type: "app2".to_string(),
});

// Add line tasks
tasks.push(IngestionTask::Lines {
    lines: vec!["2024-01-15T10:30:45Z INFO [main] test: Message".to_string()],
    log_type: "direct".to_string(),
});

// Submit all tasks at once
let task_ids = submit_batch(&pool, tasks)?;
println!("Submitted {} tasks", task_ids.len());

// Wait for all tasks to complete
let results = wait_for_tasks(&pool, &task_ids, 60)?;

for result in results {
    if result.success {
        println!("Task {} succeeded in {}ms", result.task_id, result.duration_ms);
    } else {
        println!("Task {} failed: {:?}", result.task_id, result.error);
    }
}
```

### 5. Ingesting Different Log Types

```rust
// Ingest from local file
let task_id = pool.submit_file("logs/app.log".into(), "application".to_string())?;

// Ingest from URL
let task_id = pool.submit_url("https://example.com/logs/system.log".into(), "system".to_string())?;

// Ingest raw log lines
let logs = vec![
    "2024-01-15T10:30:45Z INFO [api] request: GET /users".to_string(),
    "2024-01-15T10:30:46Z INFO [db] query: SELECT * FROM users".to_string(),
];
let task_id = pool.submit_lines(logs, "direct".to_string())?;
```

### 6. Custom Configuration for High Throughput

```rust
use bund_blobstore::common::log_worker_pool::{LogWorkerPool, WorkerPoolConfig};

let config = WorkerPoolConfig {
    num_workers: 16,
    queue_capacity: 10000,
    retry_failed: true,
    max_retries: 5,
    retry_delay_ms: 100,
    shutdown_timeout_seconds: 10,
    grok_patterns: Vec::new(),
    default_source: "high_throughput".to_string(),
    distribution_strategy: DistributionStrategy::RoundRobin,
    data_dir: Some("/data/bund".into()),
};

let mut pool = LogWorkerPool::create(config)?;

// Submit thousands of tasks
for i in 0..1000 {
    let logs = generate_logs(100);
    pool.submit_lines(logs, format!("batch_{}", i))?;
}

let stats = pool.get_stats();
println!("Active workers: {}", stats.active_workers);
println!("Tasks completed: {}", stats.total_tasks_completed);

pool.stop(true)?;
```

### 7. Error Handling with Retries

```rust
// Configure retry behavior
let mut config = WorkerPoolConfig::default();
config.retry_failed = true;
config.max_retries = 3;
config.retry_delay_ms = 1000;

let mut pool = LogWorkerPool::create(config)?;

// Submit task that might fail (e.g., network issue)
let task_id = pool.submit_url("https://unstable-server.com/logs/app.log".into(), "remote".to_string())?;

// Wait for result (will retry automatically)
match pool.wait_for_task(task_id, 60)? {
    Some(result) => {
        if result.success {
            println!("Task succeeded after retries");
        } else {
            println!("Task failed after {} retries: {:?}", 
                     config.max_retries, result.error);
        }
    }
    None => println!("Task timeout"),
}
```

### 8. Graceful Shutdown

```rust
let mut pool = start_worker_pool("/data/bund".into(), 4, "my_app")?;

// Submit long-running tasks
for i in 0..100 {
    let logs = generate_large_logs(1000);
    pool.submit_lines(logs, format!("batch_{}", i))?;
}

// Allow tasks to complete gracefully
println!("Waiting for tasks to complete...");
std::thread::sleep(std::time::Duration::from_secs(10));

// Graceful shutdown (waits for running tasks)
pool.stop(true)?;
println!("All tasks completed and pool stopped");

// Or immediate shutdown (doesn't wait)
// pool.stop(false)?;
```

### 9. Monitoring Pool Statistics

```rust
let mut pool = start_worker_pool("/data/bund".into(), 8, "monitored_app")?;

// Submit tasks
for i in 0..50 {
    pool.submit_lines(generate_logs(100), format!("batch_{}", i))?;
}

// Monitor progress
while pool.get_stats().total_tasks_completed < 50 {
    let stats = pool.get_stats();
    println!("Progress: {}/{} tasks completed", 
             stats.total_tasks_completed, 
             stats.total_tasks_submitted);
    println!("Active workers: {}", stats.active_workers);
    println!("Records ingested: {}", stats.total_records_ingested);
    std::thread::sleep(std::time::Duration::from_secs(1));
}

let final_stats = pool.get_stats();
println!("\n=== Final Statistics ===");
println!("Total tasks: {}", final_stats.total_tasks_submitted);
println!("Completed: {}", final_stats.total_tasks_completed);
println!("Failed: {}", final_stats.total_tasks_failed);
println!("Records ingested: {}", final_stats.total_records_ingested);
println!("Total time: {}ms", final_stats.total_ingestion_time_ms);

pool.stop(true)?;
```

### 10. Concurrent Task Submission

```rust
use std::sync::Arc;
use std::thread;

let pool = Arc::new(pool);
let mut handles = vec![];

// Submit tasks from multiple threads
for thread_id in 0..10 {
    let pool_clone = pool.clone();
    let handle = thread::spawn(move || {
        for i in 0..100 {
            let logs = generate_logs(10);
            let task_id = pool_clone.submit_lines(logs, format!("thread_{}_batch_{}", thread_id, i)).unwrap();
            println!("Thread {} submitted task {}", thread_id, task_id);
        }
    });
    handles.push(handle);
}

// Wait for all submission threads
for handle in handles {
    handle.join().unwrap();
}

// Wait for all tasks to complete
let mut pool_guard = pool.lock().unwrap();
while pool_guard.get_stats().total_tasks_completed < 1000 {
    std::thread::sleep(std::time::Duration::from_millis(500));
}
```

### 11. Custom Grok Patterns for Specialized Logs

```rust
let mut config = WorkerPoolConfig::default();
config.grok_patterns = vec![
    ("nginx_access".to_string(), 
     r"(?P<client>\d+\.\d+\.\d+\.\d+) - - \[(?P<timestamp>[^\]]+)\] \"(?P<method>\w+) (?P<path>[^\s]+) HTTP/\d+\.\d+\" (?P<status>\d+) (?P<bytes>\d+)".to_string()),
    ("postgres_log".to_string(),
     r"(?P<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}.\d{3}) \[(?P<pid>\d+)\] LOG:  (?P<message>.*)".to_string()),
];

let mut pool = LogWorkerPool::create(config)?;

// These logs will be parsed with the custom patterns
pool.submit_lines(nginx_logs, "nginx".to_string())?;
pool.submit_lines(postgres_logs, "postgres".to_string())?;
```

## Performance Tuning

### For Maximum Throughput

```rust
let config = WorkerPoolConfig {
    num_workers: num_cpus::get(),  // Use all CPU cores
    queue_capacity: 100000,         // Large queue
    retry_failed: false,            // Disable retries for speed
    ..Default::default()
};
```

### For Memory-Constrained Environments

```rust
let config = WorkerPoolConfig {
    num_workers: 2,                 // Fewer workers
    queue_capacity: 100,            // Small queue
    retry_failed: true,
    max_retries: 2,
    ..Default::default()
};
```

### For Real-Time Processing

```rust
let config = WorkerPoolConfig {
    num_workers: 1,                 // Single worker for ordering
    queue_capacity: 10,             // Small queue for low latency
    retry_failed: false,            // No retries
    shutdown_timeout_seconds: 1,    // Quick shutdown
    ..Default::default()
};
```

## Error Handling

```rust
match pool.submit_lines(logs, "test".to_string()) {
    Ok(task_id) => {
        match pool.wait_for_task(task_id, 30) {
            Ok(Some(result)) => {
                if result.success {
                    println!("Task succeeded");
                } else {
                    eprintln!("Task failed: {:?}", result.error);
                }
            }
            Ok(None) => eprintln!("Task timeout"),
            Err(e) => eprintln!("Error waiting: {}", e),
        }
    }
    Err(e) => eprintln!("Failed to submit: {}", e),
}
```

## Best Practices

1. **Worker Count** - Set to number of CPU cores for CPU-bound tasks, higher for I/O-bound
2. **Queue Capacity** - Monitor queue size; increase if tasks are being rejected
3. **Retry Configuration** - Use retries for transient failures (network, temporary unavailability)
4. **Graceful Shutdown** - Always use graceful shutdown in production to avoid data loss
5. **Statistics Monitoring** - Regularly check pool statistics to detect bottlenecks
6. **Task Size** - Balance task size; very small tasks add overhead, very large tasks cause imbalance

## Troubleshooting

### Issue: Tasks Timeout
**Solution**: Increase `shutdown_timeout_seconds` or check worker health

### Issue: High Memory Usage
**Solution**: Reduce `queue_capacity` or decrease `num_workers`

### Issue: Tasks Not Completing
**Solution**: Check worker count; ensure `retry_failed` is enabled for transient failures

### Issue: Slow Processing
**Solution**: Increase `num_workers` or optimize Grok patterns

## API Reference

### Pool Creation
- `start_worker_pool(data_dir, num_workers, default_source)` - Create pool with own resources
- `start_worker_pool_with_manager(manager, num_workers, default_source, grok_patterns)` - Use external manager
- `start_worker_pool_with_ingestor(ingestor, num_workers)` - Use external ingestor
- `LogWorkerPool::create(config)` - Create with full configuration
- `LogWorkerPool::with_external_manager(config, manager)` - Create with external manager
- `LogWorkerPool::with_external_ingestor(config, ingestor)` - Create with external ingestor

### Task Submission
- `submit_task(task)` - Submit any task type
- `submit_lines(lines, log_type)` - Submit raw log lines
- `submit_file(path, log_type)` - Submit log file
- `submit_url(url, log_type)` - Submit URL
- `submit_batch(pool, tasks)` - Submit multiple tasks
- `wait_for_tasks(pool, task_ids, timeout)` - Wait for multiple tasks

### Pool Management
- `get_stats()` - Get current statistics
- `wait_for_task(task_id, timeout)` - Wait for specific task
- `stop(graceful)` - Stop the pool

## See Also

- [Log Ingestor Documentation](./LOG_INGESTION.md)
- [Grok Integration Documentation](./GROK_INTEGRFATION.md)
- [Data Distribution Manager](./DATA_DISTRIBUTION.md)

## License

This module is part of the Bund BlobStore project and is licensed under the same terms.
```
