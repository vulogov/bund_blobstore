use bund_blobstore::MultiModalStore;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_multi_modal_store() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = MultiModalStore::open(temp_file.path())?;

        // Insert text
        store.insert_text("doc1", "A beautiful sunset over mountains", None)?;

        // Search similar
        let results = store.search_similar("sunset landscape", 5)?;
        assert!(!results.is_empty());

        Ok(())
    }
}
