use bund_blobstore::{
    AdvancedChunkingConfig, DataDistributionManager, DistributionStrategy, StemmingLanguage,
};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = DataDistributionManager::new("rag_data", DistributionStrategy::RoundRobin)?;

    // Configure advanced chunking for RAG
    let chunk_config = AdvancedChunkingConfig {
        chunk_size: 512,
        chunk_overlap: 50,
        min_chunk_size: 100,
        break_on_sentences: true,
        break_on_paragraphs: true,
        preserve_metadata: true,
        context_before_chars: 200,
        context_after_chars: 200,
        enable_stemming: true,
        language: StemmingLanguage::English,
    };

    // Store document with advanced chunking
    let long_text = "Your long document text here...";
    let metadata = HashMap::new();

    let doc = manager.store_advanced_chunked_document(
        "technical_guide",
        long_text,
        metadata,
        &chunk_config,
    )?;

    println!("Document stored with {} chunks", doc.chunks.len());
    println!(
        "Word count: {}, Sentences: {}, Paragraphs: {}",
        doc.word_count, doc.sentence_count, doc.paragraph_count
    );

    // Search with RAG-friendly context
    let results = manager.search_advanced_chunks(
        "database optimization techniques",
        5,
        0.7,  // 70% vector, 30% keyword
        true, // Include context
    )?;

    for result in results {
        println!(
            "Document: {}, Score: {:.3}",
            result.document_id, result.combined_score
        );
        println!("Context for RAG:\n{}\n", result.relevance_context);
    }

    // Get specific chunks for RAG
    let chunk_ids = doc
        .chunks
        .iter()
        .take(3)
        .map(|c| c.chunk_id.clone())
        .collect();
    let rag_chunks = manager.get_chunks_for_rag(
        "technical_guide",
        chunk_ids,
        500, // Context window characters
    )?;

    Ok(())
}
