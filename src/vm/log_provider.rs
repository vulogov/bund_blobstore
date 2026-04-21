use crate::DB;
use crate::common::log_worker_pool::{LogWorkerPool, WorkerPoolConfig};
use parking_lot::RwLock;
use std::sync::{Arc, OnceLock};

/// Kept private to enforce use of LogProvider methods
static LOG_POOL: OnceLock<LogWorkerPool> = OnceLock::new();

pub struct LogProvider;

impl LogProvider {
    pub fn init(num_workers: usize) -> Result<(), String> {
        let manager_instance = DB
            .get()
            .ok_or_else(|| "LogProvider: Global DB not initialized".to_string())?;

        // We clone the manager (which is likely an Arc internally or cheap to clone)
        // and wrap it in the Arc/RwLock expected by the worker pool
        let shared_manager = Arc::new(RwLock::new(manager_instance.clone()));

        let config = WorkerPoolConfig {
            num_workers,
            ..Default::default()
        };

        let pool = LogWorkerPool::with_external_manager(config, shared_manager)?;

        LOG_POOL
            .set(pool)
            .map_err(|_| "LogProvider: Already initialized".to_string())
    }

    #[inline]
    pub fn get() -> &'static LogWorkerPool {
        LOG_POOL.get().expect("LogProvider: Not initialized")
    }
}
