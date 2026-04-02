use bund_blobstore::{BatchWorker, ConcurrentBlobStore};

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tempfile::NamedTempFile;

    #[test]
    fn test_concurrent_reads() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let store = ConcurrentBlobStore::open(temp_file.path())?;

        // Write some data
        store.put("test_key", b"test_value", None)?;

        // Spawn multiple readers
        let mut handles = vec![];
        for _ in 0..10 {
            let store_clone = store.clone();
            handles.push(thread::spawn(move || {
                let data = store_clone.get("test_key").unwrap();
                assert_eq!(data, Some(b"test_value".to_vec()));
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        Ok(())
    }

    #[test]
    fn test_batch_worker() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let store = ConcurrentBlobStore::open(temp_file.path())?;
        let worker = BatchWorker::new(store, 10);

        let handle = worker.start();

        // Submit batch operations
        for i in 0..100 {
            worker.put(
                format!("key_{}", i),
                format!("value_{}", i).into_bytes(),
                None,
            )?;
        }

        worker.flush()?;

        // Verify results
        for i in 0..100 {
            let receiver = worker.get(format!("key_{}", i))?;
            let value = receiver.recv()?;
            assert_eq!(value, Some(format!("value_{}", i).into_bytes()));
        }

        drop(worker);
        handle.join().unwrap();

        Ok(())
    }
}
