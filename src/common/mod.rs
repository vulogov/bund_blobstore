// src/common/mod.rs
pub mod grok_integration;
pub mod log_ingestor;
pub mod log_worker_pool;

pub use grok_integration::GrokLogParser;
pub use log_ingestor::{IngestionStats, LogIngestionConfig, LogIngestor, SimilarityConfig};
pub use log_worker_pool::{
    IngestionTask, LogWorkerPool, PoolStats, TaskResult, WorkerPoolConfig, start_worker_pool,
    start_worker_pool_with_ingestor, start_worker_pool_with_manager, stop_worker_pool,
    submit_batch, wait_for_tasks,
};
