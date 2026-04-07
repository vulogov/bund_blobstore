// src/common/log_worker_pool.rs - FIXED ORIGINAL VERSION

use crate::common::grok_integration::GrokLogParser;
use crate::common::log_ingestor::{IngestionStats, LogIngestor};
use crate::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone)]
pub enum IngestionTask {
    File {
        path: PathBuf,
        log_type: String,
    },
    Url {
        url: String,
        log_type: String,
    },
    Lines {
        lines: Vec<String>,
        log_type: String,
    },
}

#[derive(Debug)]
pub struct TaskResult {
    pub task_id: u64,
    pub stats: IngestionStats,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct WorkerPoolConfig {
    pub num_workers: usize,
    pub queue_capacity: usize,
    pub retry_failed: bool,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub shutdown_timeout_seconds: u64,
    pub grok_patterns: Vec<(String, String)>,
    pub default_source: String,
    pub distribution_strategy: DistributionStrategy,
    pub data_dir: Option<PathBuf>,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            num_workers: 4,
            queue_capacity: 100,
            retry_failed: true,
            max_retries: 3,
            retry_delay_ms: 100,
            shutdown_timeout_seconds: 5,
            grok_patterns: Vec::new(),
            default_source: "worker_pool".to_string(),
            distribution_strategy: DistributionStrategy::RoundRobin,
            data_dir: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PoolStats {
    pub total_tasks_submitted: u64,
    pub total_tasks_completed: u64,
    pub total_tasks_failed: u64,
    pub total_tasks_retried: u64,
    pub active_workers: usize,
    pub queue_size: usize,
    pub total_records_ingested: u64,
    pub total_ingestion_time_ms: u64,
}

struct WorkerMessage {
    task: IngestionTask,
    task_id: u64,
    response_tx: mpsc::Sender<TaskResult>,
}

pub struct LogWorkerPool {
    workers: Vec<JoinHandle<()>>,
    task_tx: mpsc::SyncSender<WorkerMessage>,
    stats: Arc<RwLock<PoolStats>>,
    running: Arc<AtomicBool>,
    task_counter: Arc<Mutex<u64>>,
    pending_results: Arc<Mutex<HashMap<u64, mpsc::Receiver<TaskResult>>>>,
    #[allow(dead_code)]
    external_manager: Option<Arc<RwLock<DataDistributionManager>>>,
    #[allow(dead_code)]
    external_ingestor: Option<Arc<LogIngestor>>,
    config: WorkerPoolConfig,
}

impl LogWorkerPool {
    pub fn create(config: WorkerPoolConfig) -> Result<Self> {
        let data_dir = config
            .data_dir
            .clone()
            .ok_or_else(|| "data_dir required".to_string())?;

        let (task_tx, task_rx) = mpsc::sync_channel(config.queue_capacity);
        let task_rx = Arc::new(Mutex::new(task_rx));

        let stats = Arc::new(RwLock::new(PoolStats::default()));
        let running = Arc::new(AtomicBool::new(true));
        let task_counter = Arc::new(Mutex::new(0u64));
        let pending_results = Arc::new(Mutex::new(HashMap::new()));

        let mut workers = Vec::with_capacity(config.num_workers);

        for worker_id in 0..config.num_workers {
            let task_rx = task_rx.clone();
            let stats_clone = stats.clone();
            let running_clone = running.clone();
            let config_clone = config.clone();
            let data_dir_clone = data_dir.clone();
            let pending_results_clone = pending_results.clone();

            let handle = thread::spawn(move || {
                Self::worker_loop_with_own_resources(
                    worker_id,
                    task_rx,
                    stats_clone,
                    running_clone,
                    config_clone,
                    data_dir_clone,
                    pending_results_clone,
                );
            });
            workers.push(handle);
        }

        Ok(Self {
            workers,
            task_tx,
            stats,
            running,
            task_counter,
            pending_results,
            external_manager: None,
            external_ingestor: None,
            config,
        })
    }

    pub fn with_external_manager(
        config: WorkerPoolConfig,
        manager: Arc<RwLock<DataDistributionManager>>,
    ) -> Result<Self> {
        let (task_tx, task_rx) = mpsc::sync_channel(config.queue_capacity);
        let task_rx = Arc::new(Mutex::new(task_rx));

        let stats = Arc::new(RwLock::new(PoolStats::default()));
        let running = Arc::new(AtomicBool::new(true));
        let task_counter = Arc::new(Mutex::new(0u64));
        let pending_results = Arc::new(Mutex::new(HashMap::new()));

        let mut workers = Vec::with_capacity(config.num_workers);

        for worker_id in 0..config.num_workers {
            let task_rx = task_rx.clone();
            let stats_clone = stats.clone();
            let running_clone = running.clone();
            let config_clone = config.clone();
            let manager_clone = manager.clone();
            let pending_results_clone = pending_results.clone();

            let handle = thread::spawn(move || {
                Self::worker_loop_with_external_manager(
                    worker_id,
                    task_rx,
                    stats_clone,
                    running_clone,
                    config_clone,
                    manager_clone,
                    pending_results_clone,
                );
            });
            workers.push(handle);
        }

        Ok(Self {
            workers,
            task_tx,
            stats,
            running,
            task_counter,
            pending_results,
            external_manager: Some(manager),
            external_ingestor: None,
            config,
        })
    }

    pub fn with_external_ingestor(
        config: WorkerPoolConfig,
        ingestor: Arc<LogIngestor>,
    ) -> Result<Self> {
        let (task_tx, task_rx) = mpsc::sync_channel(config.queue_capacity);
        let task_rx = Arc::new(Mutex::new(task_rx));

        let stats = Arc::new(RwLock::new(PoolStats::default()));
        let running = Arc::new(AtomicBool::new(true));
        let task_counter = Arc::new(Mutex::new(0u64));
        let pending_results = Arc::new(Mutex::new(HashMap::new()));

        let mut workers = Vec::with_capacity(config.num_workers);

        for worker_id in 0..config.num_workers {
            let task_rx = task_rx.clone();
            let stats_clone = stats.clone();
            let running_clone = running.clone();
            let config_clone = config.clone();
            let ingestor_clone = ingestor.clone();
            let pending_results_clone = pending_results.clone();

            let handle = thread::spawn(move || {
                Self::worker_loop_with_external_ingestor(
                    worker_id,
                    task_rx,
                    stats_clone,
                    running_clone,
                    config_clone,
                    ingestor_clone,
                    pending_results_clone,
                );
            });
            workers.push(handle);
        }

        Ok(Self {
            workers,
            task_tx,
            stats,
            running,
            task_counter,
            pending_results,
            external_manager: None,
            external_ingestor: Some(ingestor),
            config,
        })
    }

    fn worker_loop_with_own_resources(
        worker_id: usize,
        task_rx: Arc<Mutex<mpsc::Receiver<WorkerMessage>>>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        data_dir: PathBuf,
        pending_results: Arc<Mutex<HashMap<u64, mpsc::Receiver<TaskResult>>>>,
    ) {
        let distribution_manager =
            match DataDistributionManager::new(&data_dir, config.distribution_strategy.clone()) {
                Ok(dm) => Arc::new(RwLock::new(dm)),
                Err(e) => {
                    error!("Worker {} failed to create manager: {}", worker_id, e);
                    return;
                }
            };

        let grok_parser = GrokLogParser::new(&config.default_source);
        for (name, pattern) in &config.grok_patterns {
            let _ = grok_parser.add_pattern(name, pattern);
        }

        let ingestor = LogIngestor::new(distribution_manager, grok_parser, Default::default());

        Self::worker_loop_common(
            worker_id,
            task_rx,
            stats,
            running,
            config,
            &ingestor,
            pending_results,
        );
    }

    fn worker_loop_with_external_manager(
        worker_id: usize,
        task_rx: Arc<Mutex<mpsc::Receiver<WorkerMessage>>>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        manager: Arc<RwLock<DataDistributionManager>>,
        pending_results: Arc<Mutex<HashMap<u64, mpsc::Receiver<TaskResult>>>>,
    ) {
        let grok_parser = GrokLogParser::new(&config.default_source);
        for (name, pattern) in &config.grok_patterns {
            let _ = grok_parser.add_pattern(name, pattern);
        }

        let ingestor = LogIngestor::new(manager, grok_parser, Default::default());

        Self::worker_loop_common(
            worker_id,
            task_rx,
            stats,
            running,
            config,
            &ingestor,
            pending_results,
        );
    }

    fn worker_loop_with_external_ingestor(
        worker_id: usize,
        task_rx: Arc<Mutex<mpsc::Receiver<WorkerMessage>>>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        ingestor: Arc<LogIngestor>,
        pending_results: Arc<Mutex<HashMap<u64, mpsc::Receiver<TaskResult>>>>,
    ) {
        Self::worker_loop_common(
            worker_id,
            task_rx,
            stats,
            running,
            config,
            &ingestor,
            pending_results,
        );
    }

    fn worker_loop_common(
        worker_id: usize,
        task_rx: Arc<Mutex<mpsc::Receiver<WorkerMessage>>>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        ingestor: &LogIngestor,
        _pending_results: Arc<Mutex<HashMap<u64, mpsc::Receiver<TaskResult>>>>,
    ) {
        info!("Worker {} started", worker_id);

        while running.load(Ordering::Relaxed) {
            let msg = {
                let rx = task_rx.lock().unwrap();
                match rx.recv_timeout(Duration::from_millis(10)) {
                    Ok(msg) => msg,
                    Err(_) => continue,
                }
            };

            debug!("Worker {} processing task {}", worker_id, msg.task_id);

            let start = std::time::Instant::now();
            let (success, stats_result, error) = Self::execute_task(ingestor, msg.task, &config);
            let duration_ms = start.elapsed().as_millis() as u64;

            let result = TaskResult {
                task_id: msg.task_id,
                stats: stats_result.clone(),
                success,
                error,
                duration_ms,
            };

            if let Err(e) = msg.response_tx.send(result) {
                error!("Worker {} failed to send result: {}", worker_id, e);
            }

            let mut pool_stats = stats.write();
            pool_stats.total_tasks_completed += 1;
            if !success {
                pool_stats.total_tasks_failed += 1;
            }
            pool_stats.total_records_ingested += stats_result.total_records_stored as u64;
            pool_stats.total_ingestion_time_ms += duration_ms;
        }

        info!("Worker {} stopped", worker_id);
    }

    fn execute_task(
        ingestor: &LogIngestor,
        task: IngestionTask,
        config: &WorkerPoolConfig,
    ) -> (bool, IngestionStats, Option<String>) {
        let mut retries = 0;

        loop {
            let result = match &task {
                IngestionTask::File { path, log_type } => ingestor.ingest_log_file(path, log_type),
                IngestionTask::Url { url, log_type } => ingestor.ingest_from_url(url, log_type),
                IngestionTask::Lines { lines, log_type } => {
                    ingestor.ingest_log_lines(lines.clone(), log_type)
                }
            };

            match result {
                Ok(stats) => return (true, stats, None),
                Err(e) => {
                    retries += 1;
                    if !config.retry_failed || retries >= config.max_retries {
                        return (false, IngestionStats::default(), Some(e));
                    }
                    warn!(
                        "Task failed (attempt {}/{}): {}, retrying...",
                        retries, config.max_retries, e
                    );
                    thread::sleep(Duration::from_millis(config.retry_delay_ms));
                }
            }
        }
    }

    pub fn submit_task(&self, task: IngestionTask) -> Result<u64> {
        if !self.running.load(Ordering::Relaxed) {
            return Err("Worker pool is not running".to_string());
        }

        let task_id = {
            let mut counter = self.task_counter.lock().unwrap();
            let id = *counter;
            *counter += 1;
            id
        };

        let (tx, rx) = mpsc::channel();

        let msg = WorkerMessage {
            task,
            task_id,
            response_tx: tx,
        };

        self.task_tx
            .send(msg)
            .map_err(|e| format!("Failed to submit task: {}", e))?;

        {
            let mut pending = self.pending_results.lock().unwrap();
            pending.insert(task_id, rx);
        }

        let mut stats = self.stats.write();
        stats.total_tasks_submitted += 1;

        Ok(task_id)
    }

    pub fn submit_lines(&self, lines: Vec<String>, log_type: String) -> Result<u64> {
        self.submit_task(IngestionTask::Lines { lines, log_type })
    }

    pub fn submit_file(&self, path: PathBuf, log_type: String) -> Result<u64> {
        self.submit_task(IngestionTask::File { path, log_type })
    }

    pub fn submit_url(&self, url: String, log_type: String) -> Result<u64> {
        self.submit_task(IngestionTask::Url { url, log_type })
    }

    // In src/common/log_worker_pool.rs, ensure wait_for_task timeout is reasonable
    pub fn wait_for_task(&self, task_id: u64, timeout_seconds: u64) -> Result<Option<TaskResult>> {
        let rx = {
            let mut pending = self.pending_results.lock().unwrap();
            pending.remove(&task_id)
        };

        match rx {
            Some(rx) => {
                // Use a reasonable timeout for receiving
                match rx.recv_timeout(Duration::from_secs(timeout_seconds)) {
                    Ok(result) => Ok(Some(result)),
                    Err(_) => Ok(None),
                }
            }
            None => Ok(None),
        }
    }

    pub fn get_stats(&self) -> PoolStats {
        let stats = self.stats.read();
        PoolStats {
            total_tasks_submitted: stats.total_tasks_submitted,
            total_tasks_completed: stats.total_tasks_completed,
            total_tasks_failed: stats.total_tasks_failed,
            total_tasks_retried: stats.total_tasks_retried,
            active_workers: self.workers.len(),
            queue_size: 0,
            total_records_ingested: stats.total_records_ingested,
            total_ingestion_time_ms: stats.total_ingestion_time_ms,
        }
    }

    pub fn stop(&mut self, graceful: bool) -> Result<()> {
        info!("Stopping worker pool, graceful={}", graceful);
        self.running.store(false, Ordering::Relaxed);

        if graceful {
            thread::sleep(Duration::from_secs(
                self.config.shutdown_timeout_seconds as u64,
            ));
        }

        for handle in self.workers.drain(..) {
            let _ = handle.join();
        }

        info!("Worker pool stopped");
        Ok(())
    }
}

// Helper functions
pub fn start_worker_pool(
    data_dir: PathBuf,
    num_workers: usize,
    default_source: &str,
) -> Result<LogWorkerPool> {
    let config = WorkerPoolConfig {
        num_workers,
        default_source: default_source.to_string(),
        data_dir: Some(data_dir),
        queue_capacity: 100,
        ..Default::default()
    };
    LogWorkerPool::create(config)
}

pub fn start_worker_pool_with_manager(
    manager: Arc<RwLock<DataDistributionManager>>,
    num_workers: usize,
    default_source: &str,
    grok_patterns: Vec<(String, String)>,
) -> Result<LogWorkerPool> {
    let config = WorkerPoolConfig {
        num_workers,
        default_source: default_source.to_string(),
        grok_patterns,
        queue_capacity: 100,
        ..Default::default()
    };
    LogWorkerPool::with_external_manager(config, manager)
}

pub fn start_worker_pool_with_ingestor(
    ingestor: Arc<LogIngestor>,
    num_workers: usize,
) -> Result<LogWorkerPool> {
    let config = WorkerPoolConfig {
        num_workers,
        queue_capacity: 100,
        ..Default::default()
    };
    LogWorkerPool::with_external_ingestor(config, ingestor)
}

pub fn stop_worker_pool(pool: &mut LogWorkerPool, graceful: bool) -> Result<()> {
    pool.stop(graceful)
}

pub fn submit_batch(pool: &LogWorkerPool, tasks: Vec<IngestionTask>) -> Result<Vec<u64>> {
    let mut task_ids = Vec::with_capacity(tasks.len());
    for task in tasks {
        task_ids.push(pool.submit_task(task)?);
    }
    Ok(task_ids)
}

pub fn wait_for_tasks(
    pool: &LogWorkerPool,
    task_ids: &[u64],
    timeout_seconds: u64,
) -> Result<Vec<TaskResult>> {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_seconds);
    let mut results = Vec::with_capacity(task_ids.len());
    let mut remaining: Vec<u64> = task_ids.to_vec();

    while !remaining.is_empty() && start.elapsed() < timeout {
        let mut completed_indices = Vec::new();
        for (idx, &task_id) in remaining.iter().enumerate() {
            // Use a very short timeout for checking each task
            match pool.wait_for_task(task_id, 1) {
                Ok(Some(result)) => {
                    results.push(result);
                    completed_indices.push(idx);
                }
                Ok(None) => continue,
                Err(e) => {
                    warn!("Error waiting for task {}: {}", task_id, e);
                }
            }
        }
        // Remove completed tasks (from highest index to lowest to avoid shifting issues)
        for &idx in completed_indices.iter().rev() {
            remaining.remove(idx);
        }

        if !remaining.is_empty() {
            thread::sleep(Duration::from_millis(50));
        }
    }

    if !remaining.is_empty() {
        return Err(format!("Timeout waiting for tasks: {:?}", remaining));
    }

    Ok(results)
}
