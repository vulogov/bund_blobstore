// examples/embeddings_demo.rs
use bund_blobstore::common::embeddings::{
    EmbeddingGenerator, average_embeddings, cosine_similarity, euclidean_distance,
    normalize_vector, zero_embedding,
};
use std::time::Instant;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║           Embeddings Demo - Vector Operations                   ║");
    println!("║           Similarity, Distance, Normalization                   ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    if let Err(e) = run_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_demo() -> Result<(), String> {
    println!("📚 Step 1: Initializing Embedding Generator\n");

    // Initialize the embedding generator (downloads model on first run)
    let embedder = EmbeddingGenerator::with_download_progress(true)
        .map_err(|e| format!("Failed to create embedder: {}", e))?;

    println!(
        "✓ Embedding generator initialized with dimension: {}",
        embedder.dimension()
    );
    println!("  Model: all-MiniLM-L6-v2 (384 dimensions)");
    println!("  Cache directory: ./fastembed_cache\n");

    // Wait for download if needed
    if !embedder.is_download_complete() {
        println!("⏳ Downloading model... This may take a few minutes on first run\n");
        embedder
            .wait_for_download(300)
            .map_err(|e| format!("Download failed: {}", e))?;
        println!("✓ Model download complete\n");
    }

    println!("🔤 Step 2: Generating Embeddings for Text\n");

    // Generate embeddings for various texts
    let texts = vec![
        "The quick brown fox jumps over the lazy dog",
        "A fast brown fox leaps over a sleepy dog",
        "Machine learning algorithms process data efficiently",
        "Deep neural networks learn hierarchical representations",
        "The cat sat on the mat",
    ];

    let mut embeddings = Vec::new();

    for (i, text) in texts.iter().enumerate() {
        println!("Text {}: \"{}\"", i + 1, text);

        let start = Instant::now();
        let embedding = embedder.embed(text)?;
        let duration = start.elapsed();

        println!("  → Embedding dimension: {}", embedding.len());
        println!("  → Generation time: {:?}", duration);
        println!(
            "  → First 10 values: {:?}",
            &embedding[..10.min(embedding.len())]
        );
        println!(
            "  → Norm: {:.4}",
            embedding.iter().map(|x| x * x).sum::<f32>().sqrt()
        );
        embeddings.push(embedding);
        println!();
    }

    println!("📊 Step 3: Batch Embedding Generation\n");

    // Batch generate embeddings for efficiency
    let batch_texts: Vec<&str> = texts.iter().map(|s| *s).collect();
    let start = Instant::now();
    let batch_embeddings = embedder.embed_batch(&batch_texts)?;
    let duration = start.elapsed();

    println!(
        "Batch generated {} embeddings in {:?}",
        batch_embeddings.len(),
        duration
    );
    println!(
        "Average time per embedding: {:?}",
        duration / batch_embeddings.len() as u32
    );
    println!();

    println!("🔍 Step 4: Cosine Similarity Analysis\n");

    // Calculate cosine similarities between texts
    println!("Cosine Similarity Matrix:");
    println!("{:<30}", "");
    for i in 0..texts.len() {
        print!("| T{} ", i + 1);
    }
    println!("|");
    println!("{}", "─".repeat(35 + texts.len() * 5));

    for i in 0..texts.len() {
        print!("Text {:<24}", i + 1);
        for j in 0..texts.len() {
            let similarity = cosine_similarity(&embeddings[i], &embeddings[j]);
            print!("| {:5.3} ", similarity);
        }
        println!("|");
    }
    println!();

    // Analyze specific similarities
    println!("Similarity Analysis:");
    let sim_1_2 = cosine_similarity(&embeddings[0], &embeddings[1]);
    let sim_1_3 = cosine_similarity(&embeddings[0], &embeddings[2]);
    let sim_2_3 = cosine_similarity(&embeddings[1], &embeddings[2]);
    let sim_3_4 = cosine_similarity(&embeddings[2], &embeddings[3]);

    println!("  Text 1 vs Text 2 (similar meaning):     {:.4}", sim_1_2);
    println!("  Text 1 vs Text 3 (different meaning):   {:.4}", sim_1_3);
    println!("  Text 2 vs Text 3 (different meaning):   {:.4}", sim_2_3);
    println!("  Text 3 vs Text 4 (both ML-related):     {:.4}", sim_3_4);
    println!();

    println!("📏 Step 5: Euclidean Distance Analysis\n");

    // Calculate Euclidean distances
    println!("Euclidean Distance Matrix:");
    println!("{:<30}", "");
    for i in 0..texts.len() {
        print!("| T{} ", i + 1);
    }
    println!("|");
    println!("{}", "─".repeat(35 + texts.len() * 5));

    for i in 0..texts.len() {
        print!("Text {:<24}", i + 1);
        for j in 0..texts.len() {
            let distance = euclidean_distance(&embeddings[i], &embeddings[j]);
            print!("| {:5.3} ", distance);
        }
        println!("|");
    }
    println!();

    println!("Distance Analysis:");
    let dist_1_2 = euclidean_distance(&embeddings[0], &embeddings[1]);
    let dist_1_3 = euclidean_distance(&embeddings[0], &embeddings[2]);

    println!("  Text 1 vs Text 2 distance: {:.4}", dist_1_2);
    println!("  Text 1 vs Text 3 distance: {:.4}", dist_1_3);
    println!("  (Smaller distance = more similar)");
    println!();

    println!("⚡ Step 6: Vector Normalization\n");

    // Demonstrate vector normalization
    let raw_vector = vec![3.0, 4.0];
    let mut normalized = raw_vector.clone();
    let original_norm = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();
    normalize_vector(&mut normalized);
    let normalized_norm = normalized.iter().map(|x| x * x).sum::<f32>().sqrt();

    println!("Original vector: {:?}", raw_vector);
    println!("  Norm: {:.4}", original_norm);
    println!("Normalized vector: {:?}", normalized);
    println!("  Norm: {:.4}", normalized_norm);
    println!("  Unit vector achieved!");
    println!();

    println!("🎨 Step 7: Creating Custom Embeddings\n");

    // Create zero embedding
    let zero_emb = zero_embedding(384);
    println!("Zero embedding (all zeros)");
    println!("  Dimension: {}", zero_emb.len());
    println!("  Sum of values: {:.4}", zero_emb.iter().sum::<f32>());
    println!();

    // Create averaged embeddings
    let avg_embedding = average_embeddings(&embeddings[0..3]);
    if let Some(avg) = avg_embedding {
        println!("Averaged embedding of first 3 texts:");
        println!("  Dimension: {}", avg.len());
        println!("  First 10 values: {:?}", &avg[..10.min(avg.len())]);
        println!(
            "  Norm: {:.4}",
            avg.iter().map(|x| x * x).sum::<f32>().sqrt()
        );

        // Compare averaged embedding with individual ones
        let avg_sim_1 = cosine_similarity(&avg, &embeddings[0]);
        let avg_sim_2 = cosine_similarity(&avg, &embeddings[1]);
        let avg_sim_3 = cosine_similarity(&avg, &embeddings[2]);

        println!("\n  Similarity of averaged embedding:");
        println!("    With Text 1: {:.4}", avg_sim_1);
        println!("    With Text 2: {:.4}", avg_sim_2);
        println!("    With Text 3: {:.4}", avg_sim_3);
    }
    println!();

    println!("🔄 Step 8: Consistency Check (Batch vs Single)\n");

    // Verify batch and single embeddings are consistent
    let test_text = "Consistency test for embeddings";
    let single_emb = embedder.embed(test_text)?;
    let batch_emb = embedder.embed_batch(&[test_text])?;

    let consistency = cosine_similarity(&single_emb, &batch_emb[0]);
    println!("Single vs Batch embedding similarity: {:.6}", consistency);
    if consistency > 0.999 {
        println!("✓ Batch and single embeddings are consistent!");
    } else {
        println!("⚠️ Batch and single embeddings differ slightly (expected)");
    }
    println!();

    println!("📈 Step 9: Performance Metrics\n");

    // Performance benchmarks with simple approach
    let num_iterations = 5;
    let mut test_texts = Vec::new();
    for i in 0..num_iterations {
        test_texts.push(format!(
            "This is test document number {} for performance benchmarking",
            i
        ));
    }

    // Measure single embedding time
    let start = Instant::now();
    for text in &test_texts {
        let _ = embedder.embed(text)?;
    }
    let single_duration = start.elapsed();

    // Measure batch embedding time
    let batch_text_refs: Vec<&str> = test_texts.iter().map(|s| s.as_str()).collect();
    let start = Instant::now();
    let _ = embedder.embed_batch(&batch_text_refs)?;
    let batch_duration = start.elapsed();

    println!("Performance Comparison ({} texts):", num_iterations);
    println!("  Single embeddings total time: {:?}", single_duration);
    println!(
        "  Average per embedding: {:?}",
        single_duration / num_iterations as u32
    );
    println!("  Batch embeddings total time: {:?}", batch_duration);
    println!(
        "  Average per embedding: {:?}",
        batch_duration / num_iterations as u32
    );
    println!(
        "  Batch speedup: {:.2}x",
        single_duration.as_secs_f64() / batch_duration.as_secs_f64()
    );
    println!();

    println!("🎯 Step 10: Finding Most Similar Texts\n");

    // Find most similar text to the first one
    let query_embedding = &embeddings[0];
    let mut similarities: Vec<(usize, f32)> = embeddings
        .iter()
        .enumerate()
        .map(|(i, emb)| (i, cosine_similarity(query_embedding, emb)))
        .collect();

    similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("Most similar texts to '{}':", texts[0]);
    for (i, (idx, similarity)) in similarities.iter().enumerate().take(3) {
        if *idx == 0 {
            continue;
        }
        println!(
            "  {}. Text {}: {:.4} - \"{}\"",
            i + 1,
            idx + 1,
            similarity,
            &texts[*idx][..texts[*idx].len().min(50)]
        );
    }
    println!();

    println!("🔬 Step 11: Semantic Clustering\n");

    // Simple clustering based on similarity threshold
    let threshold = 0.5;
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut assigned = vec![false; embeddings.len()];

    for i in 0..embeddings.len() {
        if assigned[i] {
            continue;
        }

        let mut cluster = vec![i];
        assigned[i] = true;

        for j in i + 1..embeddings.len() {
            if !assigned[j] {
                let similarity = cosine_similarity(&embeddings[i], &embeddings[j]);
                if similarity > threshold {
                    cluster.push(j);
                    assigned[j] = true;
                }
            }
        }

        if cluster.len() > 1 {
            clusters.push(cluster);
        }
    }

    println!(
        "Found {} semantic clusters (threshold = {}):",
        clusters.len(),
        threshold
    );
    for (i, cluster) in clusters.iter().enumerate() {
        println!("  Cluster {}: {:?}", i + 1, cluster);
        println!("    Texts:");
        for &idx in cluster {
            println!("      - {}", texts[idx]);
        }
    }
    println!();

    println!("✅ Demo completed successfully!");
    println!("\n📊 Summary:");
    println!(
        "  - Generated {} embeddings ({} dimensions each)",
        embeddings.len(),
        embedder.dimension()
    );
    println!(
        "  - Batch generation is {:.2}x faster than single",
        single_duration.as_secs_f64() / batch_duration.as_secs_f64()
    );
    println!("  - Cosine similarity ranges from -1 to 1 (1 = identical)");
    println!("  - Euclidean distance ranges from 0 to ∞ (0 = identical)");
    println!("  - Vector normalization creates unit vectors");
    println!("  - Embeddings can be averaged for combined representations");

    Ok(())
}
