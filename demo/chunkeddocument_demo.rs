use bund_blobstore::{ChunkConfig, DataDistributionManager, DistributionStrategy};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = DataDistributionManager::new("chunked_data", DistributionStrategy::RoundRobin)?;

    // Configure chunking
    let chunk_config = ChunkConfig {
        chunk_size: 512,
        chunk_overlap: 50,
        min_chunk_size: 100,
    };
    manager.set_chunk_config(chunk_config);

    // Store a large document with automatic chunking
    let long_text = "Rust is a systems programming language...".repeat(100);
    let mut metadata = HashMap::new();
    metadata.insert("author".to_string(), "John Doe".to_string());
    metadata.insert("category".to_string(), "programming".to_string());

    let doc = manager.store_chunked_document("rust_guide", &long_text, metadata)?;
    println!("Stored document with {} chunks", doc.chunks.len());

    // Vector search across chunks
    let results = manager.vector_search_chunks("systems programming language", 5)?;
    for result in results {
        println!(
            "Document: {}, Score: {:.3}",
            result.document_id, result.score
        );
        println!("  Chunk text: {}", &result.text[..100]);
    }

    // Hybrid search (vector + keyword)
    let hybrid_results = manager.hybrid_search_chunks("rust fast systems", 5, 0.7)?;
    for result in hybrid_results {
        println!(
            "Document: {}, Combined score: {:.3}",
            result.document_id, result.combined_score
        );
        println!(
            "  Vector: {:.3}, Keyword: {:.3}",
            result.vector_score, result.keyword_score
        );
    }

    // Get statistics
    let stats = manager.get_chunk_statistics()?;
    println!("Total documents: {}", stats.total_documents);
    println!("Total chunks: {}", stats.total_chunks);
    println!("Avg chunks per doc: {:.2}", stats.avg_chunks_per_doc);

    Ok(())
}
