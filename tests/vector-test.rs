use bund_blobstore::VectorStore;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_vector_embedding() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = VectorStore::open(temp_file.path())?;

        // Insert documents
        store.insert_text("doc1", "The quick brown fox jumps over the lazy dog", None)?;
        store.insert_text("doc2", "A quick brown dog jumps over the lazy fox", None)?;
        store.insert_text("doc3", "The lazy cat sleeps all day", None)?;

        // Search for similar documents
        let results = store.search_similar("fast fox", 2)?;
        assert!(!results.is_empty());
        assert!(results[0].score > 0.3);

        // Check statistics
        let stats = store.statistics();
        assert_eq!(stats.total_vectors, 3);
        assert!(stats.dimension > 0);

        Ok(())
    }

    #[test]
    fn test_batch_insertion() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = VectorStore::open(temp_file.path())?;

        let documents = vec![
            ("doc1", "First document about Rust", Some("rust")),
            ("doc2", "Second document about Python", Some("python")),
            ("doc3", "Third document about JavaScript", Some("js")),
        ];

        store.insert_batch(documents)?;

        let stats = store.statistics();
        assert_eq!(stats.total_vectors, 3);

        Ok(())
    }
}
