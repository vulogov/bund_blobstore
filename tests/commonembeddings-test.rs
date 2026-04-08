// tests/embeddings_tests.rs
use bund_blobstore::common::embeddings::{
    EmbeddingGenerator, average_embeddings, cosine_similarity, euclidean_distance,
    normalize_vector, zero_embedding,
};
use std::thread;
use std::time::Duration;

#[test]
fn test_embedding_generator_creation() {
    let generator_result = EmbeddingGenerator::with_download_progress(true);
    assert!(
        generator_result.is_ok(),
        "Failed to create embedding generator: {:?}",
        generator_result.err()
    );

    if let Ok(generator) = generator_result {
        // Wait for download to complete (max 5 minutes)
        let wait_result = generator.wait_for_download(300);
        assert!(
            wait_result.is_ok(),
            "Download failed: {:?}",
            wait_result.err()
        );
        assert_eq!(generator.dimension(), 384);
    }
}

#[test]
fn test_embedding_generator_default() {
    let generator = EmbeddingGenerator::default();
    // Wait for download to complete
    let _ = generator.wait_for_download(300);
    assert_eq!(generator.dimension(), 384);
}

#[test]
fn test_embed_single_text() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download to complete
    let wait_result = generator.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed: {:?}",
        wait_result.err()
    );

    let text = "This is a test document for embedding generation";

    let embedding_result = generator.embed(text);
    assert!(embedding_result.is_ok());

    let embedding = embedding_result.unwrap();
    assert_eq!(embedding.len(), 384);
    assert!(embedding.iter().any(|&x| x > 0.0));
}

#[test]
fn test_embed_batch_texts() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download to complete
    let wait_result = generator.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed: {:?}",
        wait_result.err()
    );

    let texts = vec![
        "First test document",
        "Second test document with different content",
        "Third document about vector databases",
    ];

    let embeddings_result = generator.embed_batch(&texts);
    assert!(embeddings_result.is_ok());

    let embeddings = embeddings_result.unwrap();
    assert_eq!(embeddings.len(), 3);
    assert_eq!(embeddings[0].len(), 384);
    assert_eq!(embeddings[1].len(), 384);
    assert_eq!(embeddings[2].len(), 384);
}

#[test]
fn test_embed_empty_text() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download to complete
    let wait_result = generator.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed: {:?}",
        wait_result.err()
    );

    let text = "";

    let embedding_result = generator.embed(text);
    assert!(embedding_result.is_ok());

    let embedding = embedding_result.unwrap();
    assert_eq!(embedding.len(), 384);
    // FastEmbed returns a valid embedding even for empty strings
    assert!(embedding.iter().all(|&x| !x.is_nan() && !x.is_infinite()));
    let has_non_zero = embedding.iter().any(|&x| x.abs() > 0.0001);
    assert!(has_non_zero, "Embedding should have non-zero values");
}

#[test]
fn test_similar_texts_produce_similar_embeddings() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download to complete
    let wait_result = generator.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed: {:?}",
        wait_result.err()
    );

    let text1 = "The quick brown fox jumps over the lazy dog";
    let text2 = "A fast brown fox leaps over a sleepy dog";
    let text3 = "Machine learning algorithms process data";

    let emb1 = generator.embed(text1).expect("Failed to embed text1");
    let emb2 = generator.embed(text2).expect("Failed to embed text2");
    let emb3 = generator.embed(text3).expect("Failed to embed text3");

    let sim_similar = cosine_similarity(&emb1, &emb2);
    let sim_different = cosine_similarity(&emb1, &emb3);

    // Similar texts should have higher cosine similarity
    assert!(
        sim_similar > sim_different,
        "Similar texts should have higher similarity. Similar: {}, Different: {}",
        sim_similar,
        sim_different
    );
}

#[test]
fn test_batch_embedding_consistency() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download to complete
    let wait_result = generator.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed: {:?}",
        wait_result.err()
    );

    let texts = vec!["First document", "Second document", "Third document"];

    // Generate embeddings one by one
    let single_embeddings: Result<Vec<Vec<f32>>, _> =
        texts.iter().map(|t| generator.embed(t)).collect();

    // Generate embeddings in batch
    let batch_embeddings = generator.embed_batch(&texts);

    assert!(single_embeddings.is_ok());
    assert!(batch_embeddings.is_ok());

    let single = single_embeddings.unwrap();
    let batch = batch_embeddings.unwrap();

    assert_eq!(single.len(), batch.len());

    // Check that results are consistent
    for i in 0..single.len() {
        let similarity = cosine_similarity(&single[i], &batch[i]);
        assert!(
            similarity > 0.99,
            "Embeddings should be nearly identical. Similarity: {}",
            similarity
        );
    }
}

#[test]
fn test_embedding_dimension_consistency() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download to complete
    let wait_result = generator.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed: {:?}",
        wait_result.err()
    );

    let dim = generator.dimension();

    let text1 = "Short text";
    let text2 = "A much longer text with many more words to ensure embedding consistency";

    let emb1 = generator.embed(text1).unwrap();
    let emb2 = generator.embed(text2).unwrap();

    assert_eq!(emb1.len(), dim);
    assert_eq!(emb2.len(), dim);
}

#[test]
fn test_download_status() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Initially might be false, wait for completion with timeout
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(300);

    while !generator.is_download_complete() && start.elapsed() < timeout {
        thread::sleep(Duration::from_millis(100));
    }

    assert!(
        generator.is_download_complete(),
        "Download did not complete within timeout"
    );
}

#[test]
fn test_wait_for_download_timeout() {
    let generator =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download with 5 minute timeout (should succeed if network is available)
    let wait_result = generator.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed or timed out: {:?}",
        wait_result.err()
    );
    assert!(generator.is_download_complete());
}

// These tests don't require the embedding generator
#[test]
fn test_cosine_similarity_identical_vectors() {
    let a = vec![1.0, 2.0, 3.0];
    let b = vec![1.0, 2.0, 3.0];
    let similarity = cosine_similarity(&a, &b);
    assert!((similarity - 1.0).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_orthogonal_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0];
    let similarity = cosine_similarity(&a, &b);
    assert!((similarity - 0.0).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_opposite_vectors() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![-1.0, 0.0, 0.0];
    let similarity = cosine_similarity(&a, &b);
    assert!((similarity + 1.0).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_partial_similarity() {
    let a = vec![1.0, 1.0, 0.0];
    let b = vec![1.0, 0.0, 0.0];
    let similarity = cosine_similarity(&a, &b);
    let expected = 1.0 / 2.0_f32.sqrt();
    assert!((similarity - expected).abs() < 0.001);
}

#[test]
fn test_cosine_similarity_zero_vector() {
    let a = vec![0.0, 0.0, 0.0];
    let b = vec![1.0, 2.0, 3.0];
    let similarity = cosine_similarity(&a, &b);
    assert_eq!(similarity, 0.0);
}

#[test]
fn test_euclidean_distance_identical_points() {
    let a = vec![1.0, 2.0, 3.0];
    let b = vec![1.0, 2.0, 3.0];
    let distance = euclidean_distance(&a, &b);
    assert!((distance - 0.0).abs() < 0.001);
}

#[test]
fn test_euclidean_distance_different_points() {
    let a = vec![0.0, 0.0, 0.0];
    let b = vec![3.0, 4.0, 0.0];
    let distance = euclidean_distance(&a, &b);
    assert!((distance - 5.0).abs() < 0.001);
}

#[test]
fn test_normalize_vector_unit_length() {
    let mut v = vec![3.0, 4.0];
    normalize_vector(&mut v);
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 0.001);
    assert!((v[0] - 0.6).abs() < 0.001);
    assert!((v[1] - 0.8).abs() < 0.001);
}

#[test]
fn test_normalize_vector_already_unit() {
    let mut v = vec![1.0, 0.0, 0.0];
    normalize_vector(&mut v);
    assert!((v[0] - 1.0).abs() < 0.001);
    assert!((v[1] - 0.0).abs() < 0.001);
}

#[test]
fn test_normalize_vector_zero() {
    let mut v = vec![0.0, 0.0, 0.0];
    normalize_vector(&mut v);
    assert_eq!(v, vec![0.0, 0.0, 0.0]);
}

#[test]
fn test_zero_embedding() {
    let dim = 128;
    let embedding = zero_embedding(dim);
    assert_eq!(embedding.len(), dim);
    assert!(embedding.iter().all(|&x| x == 0.0));
}

#[test]
fn test_average_embeddings() {
    let emb1 = vec![1.0, 2.0, 3.0];
    let emb2 = vec![3.0, 4.0, 5.0];
    let emb3 = vec![5.0, 6.0, 7.0];

    let avg_result = average_embeddings(&[emb1, emb2, emb3]);
    assert!(avg_result.is_some());

    let avg = avg_result.unwrap();
    assert_eq!(avg.len(), 3);
    assert!((avg[0] - 3.0).abs() < 0.001);
    assert!((avg[1] - 4.0).abs() < 0.001);
    assert!((avg[2] - 5.0).abs() < 0.001);
}

#[test]
fn test_average_embeddings_single() {
    let emb1 = vec![1.0, 2.0, 3.0];
    let avg_result = average_embeddings(&[emb1]);
    assert!(avg_result.is_some());

    let avg = avg_result.unwrap();
    assert_eq!(avg, vec![1.0, 2.0, 3.0]);
}

#[test]
fn test_average_embeddings_empty() {
    let embeddings: Vec<Vec<f32>> = vec![];
    let avg = average_embeddings(&embeddings);
    assert!(avg.is_none());
}

#[test]
fn test_normalize_then_cosine() {
    let mut v1 = vec![3.0, 4.0];
    let mut v2 = vec![6.0, 8.0];

    normalize_vector(&mut v1);
    normalize_vector(&mut v2);

    let similarity = cosine_similarity(&v1, &v2);
    assert!((similarity - 1.0).abs() < 0.001);
}

#[test]
fn test_euclidean_vs_cosine() {
    let a = vec![1.0, 0.0];
    let b = vec![0.0, 1.0];
    let c = vec![0.5, 0.5];

    let cos_ab = cosine_similarity(&a, &b);
    let cos_ac = cosine_similarity(&a, &c);
    let dist_ab = euclidean_distance(&a, &b);
    let dist_ac = euclidean_distance(&a, &c);

    assert!(cos_ac > cos_ab);
    assert!(dist_ac < dist_ab);
}

#[test]
fn test_embedding_generator_clone() {
    let generator1 =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");

    // Wait for download to complete
    let wait_result = generator1.wait_for_download(300);
    assert!(
        wait_result.is_ok(),
        "Download failed: {:?}",
        wait_result.err()
    );

    let text = "Test text";
    let embedding1 = generator1.embed(text).unwrap();

    // Create a new generator (should produce same results for same text)
    let generator2 =
        EmbeddingGenerator::with_download_progress(false).expect("Failed to create generator");
    let wait_result2 = generator2.wait_for_download(300);
    assert!(
        wait_result2.is_ok(),
        "Download failed: {:?}",
        wait_result2.err()
    );

    let embedding2 = generator2.embed(text).unwrap();

    let similarity = cosine_similarity(&embedding1, &embedding2);
    assert!(
        similarity > 0.95,
        "Embeddings should be very similar. Similarity: {}",
        similarity
    );
}
