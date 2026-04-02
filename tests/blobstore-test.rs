use bund_blobstore::blobstore::{BlobStore, QueryOptions};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
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
    fn test_update_blob() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = BlobStore::open(temp_file.path())?;

        store.put("test", b"original", Some("test"))?;
        let original_meta = store.get_metadata("test")?.unwrap();

        // Sleep for 1 second to ensure timestamp changes
        std::thread::sleep(Duration::from_secs(1));

        store.update("test", b"updated", Some("test"))?;
        let updated_meta = store.get_metadata("test")?.unwrap();

        assert_eq!(store.get("test")?, Some(b"updated".to_vec()));
        assert_eq!(original_meta.created_at, updated_meta.created_at);
        assert!(
            updated_meta.modified_at > original_meta.modified_at,
            "modified_at should increase: {} <= {}",
            updated_meta.modified_at,
            original_meta.modified_at
        );

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
        assert_eq!(results.len(), 2); // Should match log_2024_01 and log_2024_02

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
    fn test_query_with_limit_offset() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = BlobStore::open(temp_file.path())?;

        // Insert keys in order
        for i in 1..=10 {
            store.put(&format!("key_{:02}", i), b"data", None)?;
        }

        // Test offset: skip first 2 keys (key_01, key_02), so first result should be key_03
        let options = QueryOptions {
            prefix: None,
            pattern: None,
            limit: Some(5),
            offset: Some(2), // Skip first 2 results
        };

        let results = store.query(options)?;
        assert_eq!(results.len(), 5);
        assert_eq!(
            results[0].0, "key_03",
            "First result at offset 2 should be key_03"
        );
        assert_eq!(results[1].0, "key_04");
        assert_eq!(results[2].0, "key_05");
        assert_eq!(results[3].0, "key_06");
        assert_eq!(results[4].0, "key_07");

        // Test with offset 0 (no skip)
        let options = QueryOptions {
            prefix: None,
            pattern: None,
            limit: Some(3),
            offset: Some(0),
        };

        let results = store.query(options)?;
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, "key_01");

        // Test without limit (should return all remaining from offset)
        let options = QueryOptions {
            prefix: None,
            pattern: None,
            limit: None,
            offset: Some(8),
        };

        let results = store.query(options)?;
        assert_eq!(results.len(), 2); // key_09 and key_10
        assert_eq!(results[0].0, "key_09");

        Ok(())
    }

    #[test]
    fn test_batch_operations() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = BlobStore::open(temp_file.path())?;

        // Test multiple operations
        for i in 0..100 {
            store.put(&format!("batch_key_{}", i), b"batch_data", Some("batch"))?;
        }

        assert_eq!(store.len()?, 100);

        // Test batch query
        let results = store.query_by_metadata_prefix("batch")?;
        assert_eq!(results.len(), 100);

        // Test batch delete
        for i in 0..100 {
            store.remove(&format!("batch_key_{}", i))?;
        }

        assert_eq!(store.len()?, 0);

        Ok(())
    }
}
