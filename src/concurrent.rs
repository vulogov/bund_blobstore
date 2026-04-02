use crate::blobstore::BlobStore;
use crossbeam::channel::{Receiver, Sender, bounded};
use std::path::Path;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Thread-safe wrapper around BlobStore
#[derive(Clone)]
pub struct ConcurrentBlobStore {
    inner: Arc<RwLock<BlobStore>>,
}

impl ConcurrentBlobStore {
    /// Create a new concurrent blob store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, redb::Error> {
        let store = BlobStore::open(path)?;
        Ok(ConcurrentBlobStore {
            inner: Arc::new(RwLock::new(store)),
        })
    }

    /// Acquire a read lock
    pub fn read(&self) -> ReadGuard<'_> {
        ReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    /// Acquire a write lock
    pub fn write(&self) -> WriteGuard<'_> {
        WriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    /// Concurrent put operation
    pub fn put(&self, key: &str, data: &[u8], prefix: Option<&str>) -> Result<(), redb::Error> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.put(key, data, prefix)
    }

    /// Concurrent get operation
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.get(key)
    }

    /// Concurrent remove operation
    pub fn remove(&self, key: &str) -> Result<bool, redb::Error> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.remove(key)
    }

    /// Get store statistics
    pub fn stats(&self) -> Result<StoreStats, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        Ok(StoreStats {
            total_blobs: read_guard.len()?,
            keys: read_guard.list_keys()?,
        })
    }
}

/// Read guard for concurrent access
pub struct ReadGuard<'a> {
    guard: RwLockReadGuard<'a, BlobStore>,
}

impl<'a> ReadGuard<'a> {
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        self.guard.get(key)
    }

    pub fn exists(&self, key: &str) -> Result<bool, redb::Error> {
        self.guard.exists(key)
    }

    pub fn list_keys(&self) -> Result<Vec<String>, redb::Error> {
        self.guard.list_keys()
    }
}

/// Write guard for concurrent access
pub struct WriteGuard<'a> {
    guard: RwLockWriteGuard<'a, BlobStore>,
}

impl<'a> WriteGuard<'a> {
    pub fn put(&mut self, key: &str, data: &[u8], prefix: Option<&str>) -> Result<(), redb::Error> {
        self.guard.put(key, data, prefix)
    }

    pub fn remove(&mut self, key: &str) -> Result<bool, redb::Error> {
        self.guard.remove(key)
    }

    pub fn clear(&mut self) -> Result<(), redb::Error> {
        self.guard.clear()
    }
}

/// Store statistics
#[derive(Debug, Clone)]
pub struct StoreStats {
    pub total_blobs: usize,
    pub keys: Vec<String>,
}

/// Batch operation worker
pub struct BatchWorker {
    sender: Sender<BatchOperation>,
    receiver: Receiver<BatchOperation>,
    store: ConcurrentBlobStore,
}

#[derive(Debug)]
pub enum BatchOperation {
    Put {
        key: String,
        data: Vec<u8>,
        prefix: Option<String>,
    },
    Delete {
        key: String,
    },
    Get {
        key: String,
        response: Sender<Option<Vec<u8>>>,
    },
    Flush {
        response: Sender<()>,
    },
}

impl BatchWorker {
    /// Create a new batch worker
    pub fn new(store: ConcurrentBlobStore, buffer_size: usize) -> Self {
        let (sender, receiver) = bounded(buffer_size);

        BatchWorker {
            sender,
            receiver,
            store,
        }
    }

    /// Start the batch worker
    pub fn start(&self) -> std::thread::JoinHandle<()> {
        let receiver = self.receiver.clone();
        let store = self.store.clone();

        std::thread::spawn(move || {
            let mut batch: Vec<BatchOperation> = Vec::new();

            for op in receiver {
                match op {
                    BatchOperation::Put { key, data, prefix } => {
                        batch.push(BatchOperation::Put { key, data, prefix });
                        if batch.len() >= 100 {
                            Self::flush_batch(&store, &mut batch);
                        }
                    }
                    BatchOperation::Delete { key } => {
                        batch.push(BatchOperation::Delete { key });
                        if batch.len() >= 100 {
                            Self::flush_batch(&store, &mut batch);
                        }
                    }
                    BatchOperation::Get { key, response } => {
                        Self::flush_batch(&store, &mut batch);
                        let result = store.get(&key).ok().flatten();
                        let _ = response.send(result);
                    }
                    BatchOperation::Flush { response } => {
                        Self::flush_batch(&store, &mut batch);
                        let _ = response.send(());
                    }
                }
            }

            // Final flush
            Self::flush_batch(&store, &mut batch);
        })
    }

    fn flush_batch(store: &ConcurrentBlobStore, batch: &mut Vec<BatchOperation>) {
        let mut write_guard = store.inner.write().unwrap();

        for op in batch.drain(..) {
            match op {
                BatchOperation::Put { key, data, prefix } => {
                    let _ = write_guard.put(&key, &data, prefix.as_deref());
                }
                BatchOperation::Delete { key } => {
                    let _ = write_guard.remove(&key);
                }
                _ => {}
            }
        }
    }

    /// Submit a put operation
    pub fn put(
        &self,
        key: String,
        data: Vec<u8>,
        prefix: Option<String>,
    ) -> Result<(), crossbeam::channel::SendError<BatchOperation>> {
        self.sender.send(BatchOperation::Put { key, data, prefix })
    }

    /// Submit a delete operation
    pub fn delete(&self, key: String) -> Result<(), crossbeam::channel::SendError<BatchOperation>> {
        self.sender.send(BatchOperation::Delete { key })
    }

    /// Submit a get operation
    pub fn get(
        &self,
        key: String,
    ) -> Result<Receiver<Option<Vec<u8>>>, crossbeam::channel::SendError<BatchOperation>> {
        let (sender, receiver) = bounded(1);
        self.sender.send(BatchOperation::Get {
            key,
            response: sender,
        })?;
        Ok(receiver)
    }

    /// Flush all pending operations
    pub fn flush(&self) -> Result<(), crossbeam::channel::SendError<BatchOperation>> {
        let (sender, receiver) = bounded(1);
        self.sender
            .send(BatchOperation::Flush { response: sender })?;
        let _ = receiver.recv();
        Ok(())
    }
}

/// Simple connection pool for concurrent access
pub struct ConnectionPool {
    stores: Vec<ConcurrentBlobStore>,
    current: std::sync::atomic::AtomicUsize,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new<P: AsRef<Path>>(path: P, pool_size: usize) -> Result<Self, redb::Error> {
        let mut stores = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            stores.push(ConcurrentBlobStore::open(path.as_ref())?);
        }

        Ok(ConnectionPool {
            stores,
            current: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// Get a connection from the pool (round-robin)
    pub fn get_connection(&self) -> ConcurrentBlobStore {
        let idx = self
            .current
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % self.stores.len();
        self.stores[idx].clone()
    }
}
