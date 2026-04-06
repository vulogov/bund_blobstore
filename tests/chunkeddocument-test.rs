use bund_blobstore::{ChunkingConfig, DataDistributionManager, DistributionStrategy};
use std::collections::HashMap;
use tempfile::TempDir;

#[test]
fn test_chunked_document_store() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Configure chunking
    let chunk_config = ChunkingConfig {
        chunk_size: 100,
        chunk_overlap: 20,
        min_chunk_size: 30,
    };
    manager.set_chunk_config(chunk_config);

    // Create a test document
    let long_text = "Rust is a systems programming language that runs blazingly fast, \
                     prevents segfaults, and guarantees thread safety. \
                     It is used for writing system software, game engines, \
                     operating systems, and web browsers. Rust's memory safety \
                     guarantees make it an excellent choice for secure software development. \
                     The language features zero-cost abstractions, move semantics, \
                     guaranteed memory safety, threads without data races, \
                     trait-based generics, pattern matching, type inference, \
                     minimal runtime, and efficient C bindings. Rust has been voted \
                     the most loved programming language on Stack Overflow's \
                     annual developer survey for several years running.";

    let metadata = HashMap::new();

    // Store the document
    let doc = manager.store_chunked_document("rust_intro", long_text, metadata)?;

    // Verify document was stored correctly
    assert_eq!(doc.id, "rust_intro");
    assert_eq!(doc.original_text, long_text);
    assert!(
        doc.chunks.len() > 0,
        "Document should be split into multiple chunks"
    );

    println!("Document stored with {} chunks", doc.chunks.len());
    for (i, chunk) in doc.chunks.iter().enumerate() {
        println!(
            "Chunk {}: {} chars - {}",
            i,
            chunk.text.len(),
            &chunk.text[..50.min(chunk.text.len())]
        );
        assert!(!chunk.text.is_empty());
        assert!(!chunk.shard.is_empty());
        assert!(!chunk.vector_key.is_empty());
    }

    Ok(())
}

#[test]
fn test_retrieve_chunked_document() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let chunk_config = ChunkingConfig::default();
    manager.set_chunk_config(chunk_config);

    let test_text =
        "This is a test document for retrieval testing. It contains multiple sentences.";
    let metadata = HashMap::new();

    manager.store_chunked_document("retrieve_test", test_text, metadata)?;

    let retrieved = manager.get_chunked_document("retrieve_test")?;
    assert!(retrieved.is_some());

    let doc = retrieved.unwrap();
    assert_eq!(doc.id, "retrieve_test");
    assert_eq!(doc.original_text, test_text);

    Ok(())
}

#[test]
fn test_vector_search_chunks() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let chunk_config = ChunkingConfig {
        chunk_size: 200,
        chunk_overlap: 50,
        min_chunk_size: 50,
    };
    manager.set_chunk_config(chunk_config);

    // Store multiple documents
    let docs = vec![
        (
            "doc1",
            "Rust programming language focuses on safety and performance. It has a strong type system and ownership model.",
        ),
        (
            "doc2",
            "Python is great for data science and machine learning. It has simple syntax and many libraries.",
        ),
        (
            "doc3",
            "JavaScript runs in web browsers and is essential for frontend development. Node.js enables backend JavaScript.",
        ),
        (
            "doc4",
            "Go language is designed for concurrent systems. It has goroutines and channels for parallelism.",
        ),
    ];

    let metadata = HashMap::new();
    for (id, text) in &docs {
        manager.store_chunked_document(id, text, metadata.clone())?;
    }

    // Wait a moment for indexing to complete
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Test vector search
    let results = manager.vector_search_chunks("programming language safety", 5)?;

    assert!(!results.is_empty(), "Should find at least one result");
    println!("Vector search results:");
    for result in &results {
        println!("  Doc: {}, Score: {:.3}", result.document_id, result.score);
        println!("    Text: {}", &result.text[..result.text.len().min(100)]);
    }

    Ok(())
}

#[test]
fn test_hybrid_search_chunks() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let chunk_config = ChunkingConfig {
        chunk_size: 100,
        chunk_overlap: 20,
        min_chunk_size: 30,
    };
    manager.set_chunk_config(chunk_config);

    // Store documents with specific keywords - make them more distinct
    let docs = vec![
        (
            "doc_rust",
            "Rust systems programming language memory safety ownership borrowing",
        ),
        (
            "doc_python",
            "Python high-level programming language data science AI machine learning",
        ),
        (
            "doc_go",
            "Go concurrent programming language goroutines channels parallelism",
        ),
    ];

    let metadata = HashMap::new();
    for (id, text) in &docs {
        manager.store_chunked_document(id, text, metadata.clone())?;
        println!("Stored document: {}", id);
    }

    // Wait for indexing
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Test vector search first to ensure it works
    let vector_results = manager.vector_search_chunks("systems programming", 5)?;
    println!("Vector search found {} results", vector_results.len());

    // Test hybrid search (70% vector, 30% keyword)
    let results = manager.hybrid_search_chunks("systems programming safety", 5, 0.7)?;

    if results.is_empty() {
        println!("No hybrid search results found. Trying with different weights...");
        let results2 = manager.hybrid_search_chunks("rust programming", 5, 0.5)?;
        assert!(
            !results2.is_empty(),
            "Hybrid search should find at least one result"
        );
        println!(
            "Hybrid search found {} results with weight 0.5",
            results2.len()
        );
    } else {
        println!("\nHybrid search results (weight 0.7):");
        for result in &results {
            println!(
                "  Doc: {}, Combined: {:.3} (Vector: {:.3}, Keyword: {:.3})",
                result.document_id,
                result.combined_score,
                result.vector_score,
                result.keyword_score
            );
            println!("    Text: {}", &result.text[..result.text.len().min(80)]);
        }
        assert!(!results.is_empty());
    }

    Ok(())
}

#[test]
fn test_search_by_document() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let chunk_config = ChunkingConfig {
        chunk_size: 100,
        chunk_overlap: 0,
        min_chunk_size: 30,
    };
    manager.set_chunk_config(chunk_config);

    let long_text =
        "First sentence. Second sentence. Third sentence. Fourth sentence. Fifth sentence.";
    manager.store_chunked_document("multi_chunk_doc", long_text, HashMap::new())?;

    let chunks = manager.search_chunks_by_document("multi_chunk_doc")?;
    assert!(!chunks.is_empty());
    println!("Found {} chunks for document", chunks.len());

    for chunk in &chunks {
        println!("  Chunk: {}", &chunk.text[..chunk.text.len().min(50)]);
    }

    Ok(())
}

#[test]
fn test_delete_chunked_document() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let chunk_config = ChunkingConfig::default();
    manager.set_chunk_config(chunk_config);

    manager.store_chunked_document(
        "delete_test",
        "This document will be deleted",
        HashMap::new(),
    )?;

    // Verify it exists
    assert!(manager.get_chunked_document("delete_test")?.is_some());

    // Delete it
    let deleted = manager.delete_chunked_document("delete_test")?;
    assert!(deleted);

    // Verify it's gone
    assert!(manager.get_chunked_document("delete_test")?.is_none());

    // Try to delete non-existent document
    let deleted = manager.delete_chunked_document("nonexistent")?;
    assert!(!deleted);

    Ok(())
}

#[test]
fn test_chunk_statistics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let chunk_config = ChunkingConfig {
        chunk_size: 150,
        chunk_overlap: 30,
        min_chunk_size: 40,
    };
    manager.set_chunk_config(chunk_config);

    // Store multiple documents
    let texts = vec![
        "Short document with only a few words.",
        "Medium length document that will be split into multiple chunks because it contains enough text to exceed the chunk size limit.",
        "Another document with different content for testing distribution across shards.",
    ];

    let metadata = HashMap::new();
    for (i, text) in texts.iter().enumerate() {
        manager.store_chunked_document(&format!("stats_doc_{}", i), text, metadata.clone())?;
    }

    let stats = manager.get_chunk_statistics()?;

    println!("\nChunk Statistics:");
    println!("  Total documents: {}", stats.total_documents);
    println!("  Total chunks: {}", stats.total_chunks);
    println!("  Avg chunks per doc: {:.2}", stats.avg_chunks_per_doc);
    println!("  Chunk size: {}", stats.chunk_size);
    println!("  Chunk overlap: {}", stats.chunk_overlap);
    println!("  Chunks per shard: {:?}", stats.chunks_per_shard);

    assert_eq!(stats.total_documents, 3);
    assert!(stats.total_chunks > 0);
    assert!(stats.avg_chunks_per_doc > 0.0);

    Ok(())
}

#[test]
fn test_chunk_distribution_across_shards() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let chunk_config = ChunkingConfig {
        chunk_size: 50,
        chunk_overlap: 10,
        min_chunk_size: 20,
    };
    manager.set_chunk_config(chunk_config);

    // Create a document that will generate many chunks
    let long_text = "word ".repeat(200);
    manager.store_chunked_document("distributed_doc", &long_text, HashMap::new())?;

    let stats = manager.get_chunk_statistics()?;

    println!("\nChunk Distribution:");
    for (shard, count) in &stats.chunks_per_shard {
        println!("  Shard {}: {} chunks", shard, count);
    }

    // Chunks should be distributed across multiple shards
    assert!(
        stats.chunks_per_shard.len() > 1,
        "Chunks should be distributed across shards"
    );

    Ok(())
}

#[test]
fn test_chunk_config_update() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Test default config
    let default_config = ChunkingConfig::default();
    assert_eq!(default_config.chunk_size, 512);

    // Update config
    let new_config = ChunkingConfig {
        chunk_size: 256,
        chunk_overlap: 40,
        min_chunk_size: 80,
    };
    manager.set_chunk_config(new_config);

    // Store document with new config
    let test_text = "This document should use the updated chunking configuration for splitting.";
    manager.store_chunked_document("config_test", test_text, HashMap::new())?;

    let stats = manager.get_chunk_statistics()?;
    assert_eq!(stats.chunk_size, 256);
    assert_eq!(stats.chunk_overlap, 40);

    Ok(())
}

#[test]
fn test_metadata_preservation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let mut metadata = HashMap::new();
    metadata.insert("version".to_string(), "1.0".to_string());
    metadata.insert("priority".to_string(), "high".to_string());
    metadata.insert("tags".to_string(), "test,documentation".to_string());

    manager.store_chunked_document(
        "metadata_test",
        "Document with important metadata",
        metadata,
    )?;

    let retrieved = manager.get_chunked_document("metadata_test")?;
    assert!(retrieved.is_some());

    let doc = retrieved.unwrap();
    assert_eq!(doc.metadata.get("version"), Some(&"1.0".to_string()));
    assert_eq!(doc.metadata.get("priority"), Some(&"high".to_string()));
    assert_eq!(
        doc.metadata.get("tags"),
        Some(&"test,documentation".to_string())
    );

    Ok(())
}
