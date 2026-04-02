use crate::concurrent::UnifiedConcurrentStore;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ConnectionPool {
    stores: Vec<UnifiedConcurrentStore>,
    current: AtomicUsize,
}

impl ConnectionPool {
    pub fn new<P: AsRef<Path>>(
        path: P,
        pool_size: usize,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut stores = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            stores.push(UnifiedConcurrentStore::open(path.as_ref())?);
        }

        Ok(ConnectionPool {
            stores,
            current: AtomicUsize::new(0),
        })
    }

    pub fn get_connection(&self) -> UnifiedConcurrentStore {
        let idx = self.current.fetch_add(1, Ordering::Relaxed) % self.stores.len();
        self.stores[idx].clone()
    }

    pub fn size(&self) -> usize {
        self.stores.len()
    }
}
