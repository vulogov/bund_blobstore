use bund_blobstore::BlobStore;
use tempfile::NamedTempFile;

#[test]
fn test_put_and_get_with_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let mut store = BlobStore::open(temp_file.path())?;

    store.put("test_key", b"Hello, World!", Some("test"))?;

    let data = store.get("test_key")?;
    assert_eq!(data, Some(b"Hello, World!".to_vec()));

    let metadata = store.get_metadata("test_key")?;
    assert!(metadata.is_some());
    let meta = metadata.unwrap();
    assert_eq!(meta.key, "test_key");
    assert_eq!(meta.size, 13);
    assert_eq!(meta.prefix, Some("test".to_string()));

    Ok(())
}

#[test]
fn test_remove_blob() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let mut store = BlobStore::open(temp_file.path())?;

    store.put("test_key", b"data", None)?;
    assert!(store.exists("test_key")?);

    let removed = store.remove("test_key")?;
    assert!(removed);
    assert!(!store.exists("test_key")?);

    let removed_again = store.remove("test_key")?;
    assert!(!removed_again);

    Ok(())
}

#[test]
fn test_integrity_check() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let mut store = BlobStore::open(temp_file.path())?;

    store.put("test", b"important data", None)?;
    assert!(store.verify_integrity("test")?);

    Ok(())
}

#[test]
fn test_query_by_prefix() -> Result<(), Box<dyn std::error::Error>> {
    let temp_file = NamedTempFile::new()?;
    let mut store = BlobStore::open(temp_file.path())?;

    store.put("user:100:name", b"Alice", Some("user"))?;
    store.put("user:100:email", b"alice@example.com", Some("user"))?;
    store.put("user:200:name", b"Bob", Some("user"))?;
    store.put("config:app", b"settings", Some("config"))?;

    let results = store.query_by_prefix("user:")?;
    assert_eq!(results.len(), 3);

    let metadata_results = store.query_by_metadata_prefix("user")?;
    assert_eq!(metadata_results.len(), 3);

    Ok(())
}

#[test]
fn test_query_with_pattern() -> Result<(), Box<dyn std::error::Error>> {
    use bund_blobstore::QueryOptions;

    let temp_file = NamedTempFile::new()?;
    let mut store = BlobStore::open(temp_file.path())?;

    store.put("log_2024_01", b"data1", None)?;
    store.put("log_2024_02", b"data2", None)?;
    store.put("log_2023_12", b"data3", None)?;
    store.put("metric_2024_01", b"data4", None)?;

    let options = QueryOptions {
        prefix: Some("log".to_string()),
        pattern: Some("*2024*".to_string()),
        limit: None,
        offset: None,
    };

    let results = store.query(options)?;
    assert_eq!(results.len(), 2);

    Ok(())
}
