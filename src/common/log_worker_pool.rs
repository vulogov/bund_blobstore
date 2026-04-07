// src/common/log_worker_pool.rs
use crate::common::grok_integration::GrokLogParser;
use crate::common::log_ingestor::{IngestionStats, LogIngestor};
use crate::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
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
    Stop,
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
    pub use_external_manager: bool,
    pub use_external_ingestor: bool,
    pub grok_patterns: Vec<(String, String)>,
    pub default_source: String,
    pub distribution_strategy: DistributionStrategy,
    pub data_dir: Option<PathBuf>,
}

impl Default for WorkerPoolConfig {
    fn default() -> Self {
        Self {
            num_workers: 4,
            queue_capacity: 1000,
            retry_failed: true,
            max_retries: 3,
            retry_delay_ms: 1000,
            shutdown_timeout_seconds: 30,
            use_external_manager: false,
            use_external_ingestor: false,
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

enum WorkerMessage {
    Task(IngestionTask, u64),
    Shutdown,
}

pub struct LogWorkerPool {
    workers: Vec<JoinHandle<()>>,
    task_sender: mpsc::SyncSender<WorkerMessage>,
    stats: Arc<RwLock<PoolStats>>,
    running: Arc<AtomicBool>,
    task_counter: Arc<Mutex<u64>>,
    result_receiver: Arc<Mutex<mpsc::Receiver<TaskResult>>>,
    #[allow(dead_code)]
    result_sender: mpsc::Sender<TaskResult>,
    #[allow(dead_code)]
    external_manager: Option<Arc<RwLock<DataDistributionManager>>>,
    #[allow(dead_code)]
    external_ingestor: Option<Arc<LogIngestor>>,
}

impl LogWorkerPool {
    pub fn new(config: WorkerPoolConfig) -> Result<Self> {
        if config.use_external_manager || config.use_external_ingestor {
            return Err("Cannot create pool with external resources using new()".to_string());
        }

        let data_dir = config
            .data_dir
            .clone()
            .ok_or_else(|| "data_dir required".to_string())?;

        let (task_sender, task_receiver) = mpsc::sync_channel(config.queue_capacity);
        let (result_sender, result_receiver) = mpsc::channel();

        let stats = Arc::new(RwLock::new(PoolStats::default()));
        let running = Arc::new(AtomicBool::new(true));
        let task_counter = Arc::new(Mutex::new(0u64));

        let mut worker_senders = Vec::with_capacity(config.num_workers);
        let mut workers = Vec::with_capacity(config.num_workers);

        for worker_id in 0..config.num_workers {
            let (worker_sender, worker_receiver) = mpsc::channel();
            worker_senders.push(worker_sender);

            let result_sender_clone = result_sender.clone();
            let stats_clone = stats.clone();
            let running_clone = running.clone();
            let config_clone = config.clone();
            let data_dir_clone = data_dir.clone();

            let handle = thread::spawn(move || {
                Self::worker_loop_with_own_resources(
                    worker_id,
                    worker_receiver,
                    result_sender_clone,
                    stats_clone,
                    running_clone,
                    config_clone,
                    data_dir_clone,
                );
            });
            workers.push(handle);
        }

        let dispatcher_handle =
            Self::spawn_dispatcher(task_receiver, worker_senders, config.num_workers);
        workers.push(dispatcher_handle);

        Ok(Self {
            workers,
            task_sender,
            stats,
            running,
            task_counter,
            result_receiver: Arc::new(Mutex::new(result_receiver)),
            result_sender,
            external_manager: None,
            external_ingestor: None,
        })
    }

    pub fn with_external_manager(
        config: WorkerPoolConfig,
        manager: Arc<RwLock<DataDistributionManager>>,
    ) -> Result<Self> {
        let (task_sender, task_receiver) = mpsc::sync_channel(config.queue_capacity);
        let (result_sender, result_receiver) = mpsc::channel();

        let stats = Arc::new(RwLock::new(PoolStats::default()));
        let running = Arc::new(AtomicBool::new(true));
        let task_counter = Arc::new(Mutex::new(0u64));

        let mut worker_senders = Vec::with_capacity(config.num_workers);
        let mut workers = Vec::with_capacity(config.num_workers);

        for worker_id in 0..config.num_workers {
            let (worker_sender, worker_receiver) = mpsc::channel();
            worker_senders.push(worker_sender);

            let result_sender_clone = result_sender.clone();
            let stats_clone = stats.clone();
            let running_clone = running.clone();
            let config_clone = config.clone();
            let manager_clone = manager.clone();

            let handle = thread::spawn(move || {
                Self::worker_loop_with_external_manager(
                    worker_id,
                    worker_receiver,
                    result_sender_clone,
                    stats_clone,
                    running_clone,
                    config_clone,
                    manager_clone,
                );
            });
            workers.push(handle);
        }

        let dispatcher_handle =
            Self::spawn_dispatcher(task_receiver, worker_senders, config.num_workers);
        workers.push(dispatcher_handle);

        Ok(Self {
            workers,
            task_sender,
            stats,
            running,
            task_counter,
            result_receiver: Arc::new(Mutex::new(result_receiver)),
            result_sender,
            external_manager: Some(manager),
            external_ingestor: None,
        })
    }

    pub fn with_external_ingestor(
        config: WorkerPoolConfig,
        ingestor: Arc<LogIngestor>,
    ) -> Result<Self> {
        let (task_sender, task_receiver) = mpsc::sync_channel(config.queue_capacity);
        let (result_sender, result_receiver) = mpsc::channel();

        let stats = Arc::new(RwLock::new(PoolStats::default()));
        let running = Arc::new(AtomicBool::new(true));
        let task_counter = Arc::new(Mutex::new(0u64));

        let mut worker_senders = Vec::with_capacity(config.num_workers);
        let mut workers = Vec::with_capacity(config.num_workers);

        for worker_id in 0..config.num_workers {
            let (worker_sender, worker_receiver) = mpsc::channel();
            worker_senders.push(worker_sender);

            let result_sender_clone = result_sender.clone();
            let stats_clone = stats.clone();
            let running_clone = running.clone();
            let config_clone = config.clone();
            let ingestor_clone = ingestor.clone();

            let handle = thread::spawn(move || {
                Self::worker_loop_with_external_ingestor(
                    worker_id,
                    worker_receiver,
                    result_sender_clone,
                    stats_clone,
                    running_clone,
                    config_clone,
                    ingestor_clone,
                );
            });
            workers.push(handle);
        }

        let dispatcher_handle =
            Self::spawn_dispatcher(task_receiver, worker_senders, config.num_workers);
        workers.push(dispatcher_handle);

        Ok(Self {
            workers,
            task_sender,
            stats,
            running,
            task_counter,
            result_receiver: Arc::new(Mutex::new(result_receiver)),
            result_sender,
            external_manager: None,
            external_ingestor: Some(ingestor),
        })
    }

    fn spawn_dispatcher(
        task_receiver: mpsc::Receiver<WorkerMessage>,
        worker_senders: Vec<mpsc::Sender<WorkerMessage>>,
        num_workers: usize,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            if num_workers == 0 {
                // No workers to dispatch to, just drain the channel to avoid blocking
                for _ in task_receiver {
                    eprintln!("Warning: No workers available to process task");
                }
                return;
            }

            let mut worker_idx = 0;
            for message in task_receiver {
                if let Some(sender) = worker_senders.get(worker_idx) {
                    if let Err(e) = sender.send(message) {
                        eprintln!("Failed to send to worker {}: {}", worker_idx, e);
                    }
                }
                worker_idx = (worker_idx + 1) % num_workers;
            }
        })
    }

    fn worker_loop_with_own_resources(
        worker_id: usize,
        worker_receiver: mpsc::Receiver<WorkerMessage>,
        result_sender: mpsc::Sender<TaskResult>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        data_dir: PathBuf,
    ) {
        let distribution_manager =
            match DataDistributionManager::new(&data_dir, config.distribution_strategy.clone()) {
                Ok(dm) => Arc::new(RwLock::new(dm)),
                Err(e) => {
                    error!("Worker {} failed: {}", worker_id, e);
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
            worker_receiver,
            result_sender,
            stats,
            running,
            config,
            &ingestor,
        );
    }

    fn worker_loop_with_external_manager(
        worker_id: usize,
        worker_receiver: mpsc::Receiver<WorkerMessage>,
        result_sender: mpsc::Sender<TaskResult>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        manager: Arc<RwLock<DataDistributionManager>>,
    ) {
        let grok_parser = GrokLogParser::new(&config.default_source);
        for (name, pattern) in &config.grok_patterns {
            let _ = grok_parser.add_pattern(name, pattern);
        }

        let ingestor = LogIngestor::new(manager, grok_parser, Default::default());
        Self::worker_loop_common(
            worker_id,
            worker_receiver,
            result_sender,
            stats,
            running,
            config,
            &ingestor,
        );
    }

    fn worker_loop_with_external_ingestor(
        worker_id: usize,
        worker_receiver: mpsc::Receiver<WorkerMessage>,
        result_sender: mpsc::Sender<TaskResult>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        ingestor: Arc<LogIngestor>,
    ) {
        Self::worker_loop_common(
            worker_id,
            worker_receiver,
            result_sender,
            stats,
            running,
            config,
            &ingestor,
        );
    }

    fn worker_loop_common(
        worker_id: usize,
        worker_receiver: mpsc::Receiver<WorkerMessage>,
        result_sender: mpsc::Sender<TaskResult>,
        stats: Arc<RwLock<PoolStats>>,
        running: Arc<AtomicBool>,
        config: WorkerPoolConfig,
        ingestor: &LogIngestor,
    ) {
        info!("Worker {} starting", worker_id);

        while running.load(Ordering::Relaxed) {
            // Use blocking receive with timeout to allow checking running flag
            let message = match worker_receiver.recv_timeout(Duration::from_millis(500)) {
                Ok(msg) => msg,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    info!("Worker {} channel disconnected", worker_id);
                    break;
                }
            };

            match message {
                WorkerMessage::Task(task, task_id) => {
                    debug!("Worker {} processing task {}", worker_id, task_id);

                    let start_time = std::time::Instant::now();
                    let (success, stats_result, error) =
                        Self::execute_task(ingestor, task, &config);
                    let duration_ms = start_time.elapsed().as_millis() as u64;

                    let result = TaskResult {
                        task_id,
                        stats: stats_result.clone(),
                        success,
                        error,
                        duration_ms,
                    };

                    // Update stats
                    {
                        let mut pool_stats = stats.write();
                        pool_stats.total_tasks_completed += 1;
                        if !success {
                            pool_stats.total_tasks_failed += 1;
                        }
                        pool_stats.total_records_ingested +=
                            stats_result.total_records_stored as u64;
                        pool_stats.total_ingestion_time_ms += duration_ms;
                    }

                    // Send result back
                    if let Err(e) = result_sender.send(result) {
                        error!("Worker {} failed to send result: {}", worker_id, e);
                    } else {
                        debug!("Worker {} completed task {}", worker_id, task_id);
                    }
                }
                WorkerMessage::Shutdown => {
                    info!("Worker {} received shutdown signal", worker_id);
                    break;
                }
            }
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
                IngestionTask::File { path, log_type } => {
                    if !path.exists() {
                        return (
                            false,
                            IngestionStats::default(),
                            Some(format!("File not found: {:?}", path)),
                        );
                    }
                    ingestor.ingest_log_file(path, log_type)
                }
                IngestionTask::Url { url, log_type } => ingestor.ingest_from_url(url, log_type),
                IngestionTask::Lines { lines, log_type } => {
                    if lines.is_empty() {
                        // Empty lines is success with zero records
                        return (true, IngestionStats::default(), None);
                    }
                    ingestor.ingest_log_lines(lines.clone(), log_type)
                }
                IngestionTask::Stop => return (true, IngestionStats::default(), None),
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

        let mut counter = self.task_counter.lock().unwrap();
        let task_id = *counter;
        *counter += 1;

        debug!("Submitting task {}: {:?}", task_id, task);

        {
            let mut stats = self.stats.write();
            stats.total_tasks_submitted += 1;
        }

        // Use regular send - it will block if the channel is full
        match self.task_sender.send(WorkerMessage::Task(task, task_id)) {
            Ok(_) => Ok(task_id),
            Err(e) => Err(format!("Failed to submit task: {}", e)),
        }
    }

    pub fn submit_file(&self, path: PathBuf, log_type: String) -> Result<u64> {
        self.submit_task(IngestionTask::File { path, log_type })
    }

    pub fn submit_url(&self, url: String, log_type: String) -> Result<u64> {
        self.submit_task(IngestionTask::Url { url, log_type })
    }

    pub fn submit_lines(&self, lines: Vec<String>, log_type: String) -> Result<u64> {
        self.submit_task(IngestionTask::Lines { lines, log_type })
    }

    pub fn wait_for_task(&self, task_id: u64, timeout_seconds: u64) -> Result<Option<TaskResult>> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_seconds);

        while start.elapsed() < timeout {
            let receiver = self.result_receiver.lock().unwrap();

            // Try to receive without blocking
            match receiver.try_recv() {
                Ok(result) => {
                    if result.task_id == task_id {
                        return Ok(Some(result));
                    }
                    // For other tasks, we continue (they'll be processed later)
                }
                Err(mpsc::TryRecvError::Empty) => {
                    // No results available, sleep a bit
                    drop(receiver);
                    thread::sleep(Duration::from_millis(100));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    break;
                }
            }
        }

        Ok(None)
    }

    pub fn get_stats(&self) -> PoolStats {
        let stats = self.stats.read();
        let mut result = stats.clone();
        result.active_workers = self.workers.len().saturating_sub(1);
        result
    }

    pub fn stop(&mut self, graceful: bool) -> Result<()> {
        info!("Stopping worker pool, graceful={}", graceful);
        self.running.store(false, Ordering::Relaxed);

        if graceful {
            // Send shutdown signal to all workers
            for _ in 0..self.workers.len() {
                let _ = self.task_sender.send(WorkerMessage::Shutdown);
            }
            // Give workers time to finish current tasks
            thread::sleep(Duration::from_millis(1000));
        }

        // Wait for all workers to finish
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
        ..Default::default()
    };
    LogWorkerPool::new(config)
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
        use_external_manager: true,
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
        use_external_ingestor: true,
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
    let mut results = Vec::new();
    let mut remaining: Vec<u64> = task_ids.to_vec();

    while !remaining.is_empty() && start.elapsed() < Duration::from_secs(timeout_seconds) {
        let receiver = pool.result_receiver.lock().unwrap();
        if let Ok(result) = receiver.try_recv() {
            if let Some(pos) = remaining.iter().position(|&id| id == result.task_id) {
                remaining.remove(pos);
                results.push(result);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    if !remaining.is_empty() {
        return Err(format!("Timeout waiting for tasks: {:?}", remaining));
    }

    Ok(results)
}
