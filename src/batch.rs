use crate::concurrent::ConcurrentBlobStore;
use crossbeam::channel::{Receiver, Sender, bounded};

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

pub struct BatchWorker {
    sender: Sender<BatchOperation>,
    receiver: Receiver<BatchOperation>,
    store: ConcurrentBlobStore,
}

impl BatchWorker {
    pub fn new(store: ConcurrentBlobStore, buffer_size: usize) -> Self {
        let (sender, receiver) = bounded(buffer_size);

        BatchWorker {
            sender,
            receiver,
            store,
        }
    }

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

    pub fn put(
        &self,
        key: String,
        data: Vec<u8>,
        prefix: Option<String>,
    ) -> Result<(), crossbeam::channel::SendError<BatchOperation>> {
        self.sender.send(BatchOperation::Put { key, data, prefix })
    }

    pub fn delete(&self, key: String) -> Result<(), crossbeam::channel::SendError<BatchOperation>> {
        self.sender.send(BatchOperation::Delete { key })
    }

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

    pub fn flush(&self) -> Result<(), crossbeam::channel::SendError<BatchOperation>> {
        let (sender, receiver) = bounded(1);
        self.sender
            .send(BatchOperation::Flush { response: sender })?;
        let _ = receiver.recv();
        Ok(())
    }
}
