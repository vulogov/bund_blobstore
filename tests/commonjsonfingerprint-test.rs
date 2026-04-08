// tests/json_fingerprint_tests.rs
use bund_blobstore::common::embeddings::EmbeddingGenerator;
use bund_blobstore::common::json_fingerprint::{
    JsonFingerprintConfig, JsonFingerprintManager, json_from_str,
};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;

// Helper function to setup test environment
fn setup_test_env() -> (
    tempfile::TempDir,
    Arc<RwLock<DataDistributionManager>>,
    JsonFingerprintManager,
) {
    let temp_dir = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));

    let embedder = EmbeddingGenerator::with_download_progress(false).unwrap();
    // Wait for download if needed
    let _ = embedder.wait_for_download(300);

    let config = JsonFingerprintConfig::default();
    let fp_manager = JsonFingerprintManager::new(manager.clone(), embedder, config);

    (temp_dir, manager, fp_manager)
}

#[test]
fn test_store_and_retrieve_document() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json = json_from_str(
        r#"{
        "name": "Test User",
        "email": "test@example.com",
        "age": 25
    }"#,
    )
    .unwrap();

    let mut metadata = HashMap::new();
    metadata.insert("category".to_string(), "test".to_string());
    metadata.insert("version".to_string(), "1.0".to_string());

    let result = fp_manager.store_document("test_doc_1", json.clone(), metadata);
    assert!(result.is_ok());

    let retrieved = fp_manager.get_document("test_doc_1").unwrap();
    assert!(retrieved.is_some());

    let doc = retrieved.unwrap();
    assert_eq!(doc.id, "test_doc_1");
    assert_eq!(
        doc.content.get("name").unwrap().as_str().unwrap(),
        "Test User"
    );
    assert_eq!(doc.metadata.get("category").unwrap(), "test");
    assert_eq!(doc.fingerprint.len(), 384);
}

#[test]
fn test_update_document() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let initial_json = json_from_str(r#"{"name": "Original Name", "value": 100}"#).unwrap();
    let updated_json = json_from_str(r#"{"name": "Updated Name", "value": 200}"#).unwrap();

    let metadata = HashMap::new();

    fp_manager
        .store_document("test_doc", initial_json, metadata.clone())
        .unwrap();
    fp_manager
        .update_document("test_doc", updated_json, metadata)
        .unwrap();

    let retrieved = fp_manager.get_document("test_doc").unwrap().unwrap();
    assert_eq!(
        retrieved.content.get("name").unwrap().as_str().unwrap(),
        "Updated Name"
    );
    assert_eq!(
        retrieved.content.get("value").unwrap().as_i64().unwrap(),
        200
    );
}

#[test]
fn test_delete_document() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json = json_from_str(r#"{"id": 1, "data": "test"}"#).unwrap();
    let metadata = HashMap::new();

    fp_manager
        .store_document("delete_test", json, metadata)
        .unwrap();
    assert!(fp_manager.get_document("delete_test").unwrap().is_some());

    let deleted = fp_manager.delete_document("delete_test").unwrap();
    assert!(deleted);
    assert!(fp_manager.get_document("delete_test").unwrap().is_none());
}

#[test]
fn test_fingerprint_generation() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json1 = json_from_str(r#"{"key": "value1", "number": 123}"#).unwrap();
    let json2 = json_from_str(r#"{"key": "value2", "number": 456}"#).unwrap();
    let json3 = json_from_str(r#"{"different": "structure", "completely": "different"}"#).unwrap();

    let fp1 = fp_manager.generate_fingerprint(&json1, 0).unwrap();
    let fp2 = fp_manager.generate_fingerprint(&json2, 0).unwrap();
    let fp3 = fp_manager.generate_fingerprint(&json3, 0).unwrap();

    assert_eq!(fp1.len(), 384);
    assert_eq!(fp2.len(), 384);
    assert_eq!(fp3.len(), 384);
    assert!(fp1 != fp2);
    assert!(fp1 != fp3);
}

#[test]
fn test_field_fingerprint_generation() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json = json_from_str(
        r#"{
        "user": {"name": "John", "age": 30},
        "product": {"name": "Laptop", "price": 999}
    }"#,
    )
    .unwrap();

    let field_fps = fp_manager.generate_field_fingerprints(&json).unwrap();

    assert!(field_fps.contains_key("user"));
    assert!(field_fps.contains_key("product"));
    assert_eq!(field_fps["user"].len(), 384);
    assert_eq!(field_fps["product"].len(), 384);
}

#[test]
fn test_whole_json_similarity_search() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    // Store test documents with meaningful content
    let docs = vec![
        (
            "doc1",
            r#"{"type": "user", "name": "John Doe", "email": "john@example.com", "age": 30, "city": "New York"}"#,
        ),
        (
            "doc2",
            r#"{"type": "user", "name": "Jane Smith", "email": "jane@example.com", "age": 28, "city": "Boston"}"#,
        ),
        (
            "doc3",
            r#"{"type": "product", "name": "MacBook Pro", "price": 1999, "category": "electronics"}"#,
        ),
        (
            "doc4",
            r#"{"type": "user", "name": "Bob Johnson", "email": "bob@example.com", "age": 35, "city": "Chicago"}"#,
        ),
        (
            "doc5",
            r#"{"type": "user", "name": "Johnathan Doe", "email": "john.doe@email.com", "age": 31, "city": "New York City"}"#,
        ),
    ];

    for (id, json_str) in docs {
        let json = json_from_str(json_str).unwrap();
        fp_manager.store_document(id, json, HashMap::new()).unwrap();
    }

    // Query similar to John Doe
    let query = json_from_str(
        r#"{"type": "user", "name": "Johnathan Doe", "age": 31, "city": "New York"}"#,
    )
    .unwrap();

    // Use a very low threshold to ensure we get results
    let results = fp_manager.find_similar_documents(&query, 0.1, 3).unwrap();

    // Should find similar user documents
    assert!(
        !results.is_empty(),
        "No results found for similar user query"
    );

    // Print similarities for debugging
    println!("Similarity results:");
    for result in &results {
        println!(
            "  {}: similarity = {:.4}",
            result.document_id, result.similarity
        );
    }

    // The most similar should be doc1 or doc5 (similar names and cities)
    let top_ids: Vec<&str> = results.iter().map(|r| r.document_id.as_str()).collect();
    assert!(
        top_ids.contains(&"doc1") || top_ids.contains(&"doc5"),
        "Expected doc1 or doc5 in results, got {:?}",
        top_ids
    );
}

#[test]
fn test_field_specific_similarity_search() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    // Store documents with address information
    let docs = vec![
        (
            "doc1",
            r#"{"name": "User1", "address": {"city": "New York", "country": "USA", "zip": "10001"}}"#,
        ),
        (
            "doc2",
            r#"{"name": "User2", "address": {"city": "San Francisco", "country": "USA", "zip": "94105"}}"#,
        ),
        (
            "doc3",
            r#"{"name": "User3", "address": {"city": "London", "country": "UK", "zip": "SW1A"}}"#,
        ),
        (
            "doc4",
            r#"{"name": "User4", "address": {"city": "New York City", "country": "USA", "zip": "10002"}}"#,
        ),
        (
            "doc5",
            r#"{"name": "User5", "address": {"city": "Brooklyn", "country": "USA", "zip": "11201"}}"#,
        ),
    ];

    for (id, json_str) in docs {
        let json = json_from_str(json_str).unwrap();
        fp_manager.store_document(id, json, HashMap::new()).unwrap();
    }

    // Query for similar city - use more specific query
    let query_city = json_from_str(r#"{"city": "New York City", "country": "USA"}"#).unwrap();
    let results = fp_manager
        .find_similar_by_field("address", &query_city, 0.1, 5)
        .unwrap();

    // Should find documents with New York addresses
    assert!(
        !results.is_empty(),
        "No results found for city similarity search"
    );

    println!("Field similarity results:");
    for result in &results {
        println!(
            "  {}: similarity = {:.4}",
            result.document_id, result.similarity
        );
        if let Some(city) = result
            .content
            .get("address")
            .and_then(|a| a.get("city"))
            .and_then(|c| c.as_str())
        {
            println!("    city: {}", city);
        }
    }

    // Check that New York related documents are in results
    let found_new_york = results.iter().any(|r| {
        r.content
            .get("address")
            .and_then(|a| a.get("city"))
            .and_then(|c| c.as_str())
            .map(|c| c.contains("New York"))
            .unwrap_or(false)
    });

    assert!(
        found_new_york,
        "Expected New York related cities in results"
    );
}

#[test]
fn test_multi_field_weighted_search() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    // Store product documents
    let docs = vec![
        (
            "product1",
            r#"{"name": "Gaming Laptop", "cpu": "Intel i7", "gpu": "RTX 3060", "ram": "16GB"}"#,
        ),
        (
            "product2",
            r#"{"name": "Ultra Laptop", "cpu": "Intel i9", "gpu": "RTX 3080", "ram": "32GB"}"#,
        ),
        (
            "product3",
            r#"{"name": "Budget Laptop", "cpu": "Intel i5", "gpu": "Integrated", "ram": "8GB"}"#,
        ),
        (
            "product4",
            r#"{"name": "Gaming Desktop", "cpu": "AMD Ryzen 7", "gpu": "RTX 3070", "ram": "32GB"}"#,
        ),
        (
            "product5",
            r#"{"name": "Workstation", "cpu": "Intel i9", "gpu": "RTX 3090", "ram": "64GB"}"#,
        ),
    ];

    for (id, json_str) in docs {
        let json = json_from_str(json_str).unwrap();
        fp_manager.store_document(id, json, HashMap::new()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Multi-field query targeting product2
    let mut field_queries = HashMap::new();
    field_queries.insert("cpu".to_string(), json_from_str(r#""Intel i9""#).unwrap());
    field_queries.insert("gpu".to_string(), json_from_str(r#""RTX 3080""#).unwrap());
    field_queries.insert("ram".to_string(), json_from_str(r#""32GB""#).unwrap());

    let mut weights = HashMap::new();
    weights.insert("cpu".to_string(), 0.4);
    weights.insert("gpu".to_string(), 0.4);
    weights.insert("ram".to_string(), 0.2);

    let results = fp_manager
        .multi_field_search(field_queries, weights, 0.01, 5)
        .unwrap();

    assert!(
        !results.is_empty(),
        "No results found for multi-field search"
    );

    // Check that product2 is in results
    let product2_found = results.iter().any(|r| r.document_id == "product2");
    assert!(product2_found, "Product2 should be in results");
}

#[test]
fn test_configuration_include_fields() {
    let temp_dir = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let embedder = EmbeddingGenerator::with_download_progress(false).unwrap();
    let _ = embedder.wait_for_download(300);

    let config = JsonFingerprintConfig {
        include_fields: Some(vec!["name".to_string(), "email".to_string()]),
        exclude_fields: None,
        include_field_names: true,
        normalize_values: true,
        max_depth: 5,
        sort_keys: true,
    };

    let fp_manager = JsonFingerprintManager::new(manager.clone(), embedder, config);

    let json = json_from_str(
        r#"{
        "name": "Test User",
        "email": "test@example.com",
        "age": 30,
        "address": "123 Main St"
    }"#,
    )
    .unwrap();

    let fingerprint = fp_manager.generate_fingerprint(&json, 0).unwrap();
    assert_eq!(fingerprint.len(), 384);
}

#[test]
fn test_configuration_exclude_fields() {
    let temp_dir = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));
    let embedder = EmbeddingGenerator::with_download_progress(false).unwrap();
    let _ = embedder.wait_for_download(300);

    let config = JsonFingerprintConfig {
        include_fields: None,
        exclude_fields: Some(vec!["password".to_string(), "ssn".to_string()]),
        include_field_names: true,
        normalize_values: true,
        max_depth: 5,
        sort_keys: true,
    };

    let fp_manager = JsonFingerprintManager::new(manager.clone(), embedder, config);

    let json = json_from_str(
        r#"{
        "name": "Test User",
        "email": "test@example.com",
        "password": "secret123",
        "ssn": "123-45-6789"
    }"#,
    )
    .unwrap();

    let fingerprint = fp_manager.generate_fingerprint(&json, 0).unwrap();
    assert_eq!(fingerprint.len(), 384);
}

#[test]
fn test_document_not_found() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let result = fp_manager.get_document("nonexistent");
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

#[test]
fn test_delete_nonexistent_document() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let deleted = fp_manager.delete_document("nonexistent").unwrap();
    assert!(!deleted);
}

#[test]
fn test_multiple_documents_storage() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    for i in 0..10 {
        let json = json_from_str(&format!(r#"{{"id": {}, "data": "document_{}"}}"#, i, i)).unwrap();
        fp_manager
            .store_document(&format!("doc_{}", i), json, HashMap::new())
            .unwrap();
    }

    for i in 0..10 {
        let doc = fp_manager.get_document(&format!("doc_{}", i)).unwrap();
        assert!(doc.is_some());
        assert_eq!(doc.unwrap().content.get("id").unwrap().as_i64().unwrap(), i);
    }
}

#[test]
fn test_fingerprint_consistency() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json = json_from_str(r#"{"test": "value", "number": 42}"#).unwrap();

    let fp1 = fp_manager.generate_fingerprint(&json, 0).unwrap();
    let fp2 = fp_manager.generate_fingerprint(&json, 0).unwrap();

    assert_eq!(fp1, fp2);
}

#[test]
fn test_store_document_with_metadata() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json = json_from_str(r#"{"data": "important"}"#).unwrap();

    let mut metadata = HashMap::new();
    metadata.insert("source".to_string(), "test".to_string());
    metadata.insert("priority".to_string(), "high".to_string());
    metadata.insert("timestamp".to_string(), "2024-01-01".to_string());

    fp_manager
        .store_document("meta_test", json, metadata.clone())
        .unwrap();

    let retrieved = fp_manager.get_document("meta_test").unwrap().unwrap();
    assert_eq!(retrieved.metadata.get("source").unwrap(), "test");
    assert_eq!(retrieved.metadata.get("priority").unwrap(), "high");
    assert_eq!(retrieved.metadata.get("timestamp").unwrap(), "2024-01-01");
}

#[test]
fn test_complex_nested_json() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json = json_from_str(
        r#"{
        "level1": {
            "level2": {
                "level3": {
                    "level4": {
                        "value": "deeply nested"
                    }
                }
            }
        },
        "array": [1, 2, 3, 4, 5],
        "mixed": ["string", 123, true, null]
    }"#,
    )
    .unwrap();

    let fingerprint = fp_manager.generate_fingerprint(&json, 0);
    assert!(fingerprint.is_ok());
    assert_eq!(fingerprint.unwrap().len(), 384);
}

#[test]
fn test_empty_json() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json = json_from_str("{}").unwrap();
    let fingerprint = fp_manager.generate_fingerprint(&json, 0).unwrap();
    assert_eq!(fingerprint.len(), 384);
}

#[test]
fn test_json_array() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let json =
        json_from_str(r#"[{"name": "item1"}, {"name": "item2"}, {"name": "item3"}]"#).unwrap();
    let fingerprint = fp_manager.generate_fingerprint(&json, 0).unwrap();
    assert_eq!(fingerprint.len(), 384);
}

#[test]
fn test_batch_store_and_retrieve() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    let batch_size = 20;
    let mut ids = Vec::new();

    for i in 0..batch_size {
        let json = json_from_str(&format!(
            r#"{{"index": {}, "batch": true, "value": "data_{}"}}"#,
            i, i
        ))
        .unwrap();
        let id = format!("batch_{}", i);
        ids.push(id.clone());
        fp_manager
            .store_document(&id, json, HashMap::new())
            .unwrap();
    }

    for id in ids {
        let doc = fp_manager.get_document(&id).unwrap();
        assert!(doc.is_some());
    }
}

#[test]
fn test_get_stats() {
    let (_temp_dir, _manager, fp_manager) = setup_test_env();

    for i in 0..5 {
        let json = json_from_str(&format!(r#"{{"id": {}}}"#, i)).unwrap();
        fp_manager
            .store_document(&format!("stats_{}", i), json, HashMap::new())
            .unwrap();
    }

    let stats = fp_manager.get_stats();
    assert!(stats.total_records >= 5);
}

// tests/json_fingerprint_tests.rs - Fix the index_persistence test

#[test]
fn test_index_persistence() {
    let temp_dir = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));

    // Create first manager instance
    let embedder1 = EmbeddingGenerator::with_download_progress(false).unwrap();
    let _ = embedder1.wait_for_download(300);
    let config1 = JsonFingerprintConfig::default();
    let fp_manager = JsonFingerprintManager::new(manager.clone(), embedder1, config1);

    // Store documents with verification
    let test_docs = vec![
        ("persist_0", r#"{"id": 0, "data": "test_0"}"#),
        ("persist_1", r#"{"id": 1, "data": "test_1"}"#),
        ("persist_2", r#"{"id": 2, "data": "test_2"}"#),
    ];

    for (id, json_str) in &test_docs {
        let json = json_from_str(json_str).unwrap();
        fp_manager.store_document(id, json, HashMap::new()).unwrap();
        // Small delay to ensure index is written
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Flush index to ensure it's written
    fp_manager.flush_index().unwrap();

    // Get index stats from first manager
    let stats = fp_manager.get_index_stats().unwrap();
    let total = *stats.get("total_documents").unwrap_or(&0);
    assert_eq!(total, 3, "Expected 3 documents in index, got {}", total);

    // Create second manager instance (simulating restart)
    let embedder2 = EmbeddingGenerator::with_download_progress(false).unwrap();
    let _ = embedder2.wait_for_download(300);
    let config2 = JsonFingerprintConfig::default();
    let fp_manager2 = JsonFingerprintManager::new(manager.clone(), embedder2, config2);

    // Give time for index to load
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Verify index stats are still there
    let stats2 = fp_manager2.get_index_stats().unwrap();
    let total2 = *stats2.get("total_documents").unwrap_or(&0);
    assert_eq!(
        total2, 3,
        "Index should have 3 documents after restart, got {}",
        total2
    );

    // Verify documents are still accessible
    for (id, _) in &test_docs {
        let doc = fp_manager2.get_document(id).unwrap();
        assert!(doc.is_some(), "Document {} should exist after restart", id);
    }
}
