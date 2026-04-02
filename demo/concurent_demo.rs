use bund_blobstore::{
    BatchWorker, ConcurrentBlobStore, ConcurrentFacetedIndex, ConcurrentGraphStore,
    ConcurrentMultiModalStore, ConcurrentSearchStore, ConcurrentVectorStore, ConnectionPool,
    UnifiedConcurrentStore,
};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Unified store - access all features from one instance
    let unified = UnifiedConcurrentStore::open("unified.redb")?;

    // Thread-safe operations on any storage type
    unified.blob().put("key", b"value", None)?;
    unified.search().put_text("doc", "content", None)?;
    unified.vector().insert_text("vec", "text", None)?;

    // Or use individual concurrent stores
    let blob_store = ConcurrentBlobStore::open("blob.redb")?;
    let search_store = ConcurrentSearchStore::open("search.redb")?;
    let vector_store = ConcurrentVectorStore::open("vector.redb")?;
    let graph_store = ConcurrentGraphStore::open("graph.redb")?;
    let faceted_store = ConcurrentFacetedIndex::open("faceted.redb")?;
    let multi_store = ConcurrentMultiModalStore::open("multimodal.redb")?;

    // Use read/write guards for complex operations
    let read_guard = blob_store.read();
    let keys = read_guard.list_keys()?;
    println!("Found {} keys", keys.len());

    let mut write_guard = blob_store.write();
    write_guard.put("new_key", b"data", None)?;
    drop(write_guard); // Release lock

    // Batch operations for high throughput
    let worker = BatchWorker::new(blob_store, 100);
    let handle = worker.start();

    for i in 0..1000 {
        worker.put(
            format!("key_{}", i),
            format!("value_{}", i).into_bytes(),
            None,
        )?;
    }
    worker.flush()?;
    handle.join().unwrap();

    // Connection pool for load balancing
    let pool = ConnectionPool::new("pooled.redb", 5)?;
    let conn = pool.get_connection();
    conn.blob().put("load_balanced", b"data", None)?;

    Ok(())
}
