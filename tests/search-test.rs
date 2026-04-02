use bund_blobstore::{SearchableBlobStore, TokenizerOptions};

#[test]
fn test_full_text_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new()?;
    let mut store = SearchableBlobStore::open(temp_file.path())?;

    // Index some documents
    store.put_text(
        "doc1",
        "The quick brown fox jumps over the lazy dog",
        Some("docs"),
    )?;
    store.put_text(
        "doc2",
        "A quick brown dog jumps over the lazy fox",
        Some("docs"),
    )?;
    store.put_text("doc3", "The lazy cat sleeps all day", Some("docs"))?;

    // Search
    let results = store.search("quick brown", 10)?;
    assert_eq!(results.len(), 2);
    assert!(results[0].score >= results[1].score);

    // Search with highlighting
    let highlighted = store.search_with_highlight("fox", 10)?;
    assert!(!highlighted.is_empty());

    // Test index statistics
    let stats = store.index_stats();
    assert!(stats.total_terms > 0);

    Ok(())
}

#[test]
fn test_remove_from_index() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new()?;
    let mut store = SearchableBlobStore::open(temp_file.path())?;

    store.put_text("test_doc", "important content", None)?;

    let results = store.search("important", 10)?;
    assert_eq!(results.len(), 1);

    store.remove("test_doc")?;

    let results = store.search("important", 10)?;
    assert_eq!(results.len(), 0);

    Ok(())
}

#[test]
fn test_tokenizer_options() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tempfile::NamedTempFile;

    let mut options = TokenizerOptions::default();
    options.min_token_length = 5;
    options.case_sensitive = true;

    let temp_file = NamedTempFile::new()?;
    let store = SearchableBlobStore::open_with_options(temp_file.path(), options)?;

    // We can't directly access the index field, so we'll test through search functionality
    // Store a document with known content
    let mut store_mut = store; // Make mutable for put_text

    // Test with short words (should be filtered out due to min_token_length=5)
    store_mut.put_text("test_short", "cat dog bird fish", None)?;
    let results_short = store_mut.search("cat", 10)?;
    // "cat" has length 3, should be filtered out, so no results
    assert_eq!(results_short.len(), 0, "Short words should be filtered out");

    // Test with long enough words
    store_mut.put_text("test_long", "elephant butterfly dolphin", None)?;
    let results_long = store_mut.search("elephant", 10)?;
    // "elephant" has length 8, should be indexed
    assert_eq!(results_long.len(), 1, "Long enough words should be indexed");

    // Test case sensitivity
    store_mut.put_text("test_case", "Hello World", None)?;
    let results_case_insensitive = store_mut.search("hello", 10)?;
    // With default case_sensitive=false, should find "Hello"
    assert_eq!(
        results_case_insensitive.len(),
        1,
        "Case insensitive search should work"
    );

    Ok(())
}

#[test]
fn test_persistence() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path().to_str().unwrap();

    // First session: index data
    {
        let mut store = SearchableBlobStore::open(path)?;
        store.put_text("persist_test", "This is a persistent search test", None)?;
        store.save_index()?;
    }

    // Second session: load index and search
    {
        let store = SearchableBlobStore::open(path)?;
        let results = store.search("persistent", 10)?;
        assert!(!results.is_empty(), "Search should find persistent content");

        // Verify the content
        if let Some(data) = store.get("persist_test")? {
            let text = String::from_utf8(data)?;
            assert!(text.contains("persistent"));
        }
    }

    Ok(())
}

#[test]
fn test_auto_indexing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new()?;
    let mut store = SearchableBlobStore::open(temp_file.path())?;

    // Test with auto-indexing enabled (default)
    store.put_text("auto_doc", "This document will be auto-indexed", None)?;
    let results = store.search("auto-indexed", 10)?;
    assert_eq!(results.len(), 1);

    // Disable auto-indexing
    store.set_auto_index(false);
    store.put_text("manual_doc", "This document will not be auto-indexed", None)?;

    // Search should not find the manually indexed document
    let results = store.search("auto-indexed", 10)?;
    assert_eq!(
        results.len(),
        1,
        "Should still only find the first document"
    );

    // Manually reindex
    store.reindex()?;

    // Now search should find both
    let results = store.search("document", 10)?;
    assert_eq!(
        results.len(),
        2,
        "After reindex, both documents should be found"
    );

    Ok(())
}

#[test]
fn test_stop_words() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new()?;
    let mut store = SearchableBlobStore::open(temp_file.path())?;

    // Document with stop words
    store.put_text(
        "stop_word_test",
        "The quick brown fox jumps over the lazy dog",
        None,
    )?;

    // Search for stop word only (should not match)
    let results = store.search("the", 10)?;
    assert_eq!(results.len(), 0, "Stop words should be ignored");

    // Search for content words (should match)
    let results = store.search("quick fox", 10)?;
    assert_eq!(results.len(), 1, "Content words should match");

    Ok(())
}

#[test]
fn test_search_scoring() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tempfile::NamedTempFile;

    let temp_file = NamedTempFile::new()?;
    let mut store = SearchableBlobStore::open(temp_file.path())?;

    // Index documents one by one with flush
    store.put_text(
        "doc_high",
        "rust programming language rust systems rust",
        None,
    )?;
    store.save_index()?;

    store.put_text("doc_medium", "rust programming language", None)?;
    store.save_index()?;

    store.put_text("doc_low", "programming language", None)?;
    store.save_index()?;

    let results = store.search("rust", 10)?;

    // Verify results
    assert_eq!(results.len(), 2, "Expected 2 documents to match 'rust'");

    // Check ordering
    if results.len() >= 2 {
        // The document with "rust" appearing 3 times should be first
        assert!(results[0].key == "doc_high" || results[1].key == "doc_medium");

        // Both should have scores
        assert!(results[0].score > 0.0);
        assert!(results[1].score > 0.0);
    }

    Ok(())
}
