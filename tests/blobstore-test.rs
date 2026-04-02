#[cfg(test)]
mod tests {
    use tempfile::NamedTempFile;
    // Import from the correct module path
    use bund_blobstore::blobstore::BlobStore;

    #[test]
    fn test_blob_store_basic_operations() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = BlobStore::open(temp_file.path().to_str().unwrap())?;

        // Test put and get
        store.put("test_key", b"test_value")?;
        let retrieved = store.get("test_key")?;
        assert_eq!(retrieved, Some(b"test_value".to_vec()));

        // Test exists
        assert!(store.exists("test_key")?);
        assert!(!store.exists("nonexistent")?);

        // Test delete
        store.delete("test_key")?;
        assert!(!store.exists("test_key")?);

        Ok(())
    }

    #[test]
    fn test_multiple_values() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = BlobStore::open(temp_file.path().to_str().unwrap())?;

        // Store multiple values
        store.put("key1", b"value1")?;
        store.put("key2", b"value2")?;
        store.put("key3", b"value3")?;

        // Check length
        assert_eq!(store.len()?, 3);

        // List keys
        let mut keys = store.list_keys()?;
        keys.sort();
        assert_eq!(keys, vec!["key1", "key2", "key3"]);

        // Get all
        let all = store.get_all()?;
        assert_eq!(all.len(), 3);

        Ok(())
    }
}
