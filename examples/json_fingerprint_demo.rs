// examples/json_fingerprint_demo.rs
use bund_blobstore::common::embeddings::EmbeddingGenerator;
use bund_blobstore::common::json_fingerprint::{
    JsonFingerprintConfig, JsonFingerprintManager, json_from_str, to_pretty_json,
};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║           JSON Fingerprinting Demo                              ║");
    println!("║           Semantic Search for JSON Documents                    ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    if let Err(e) = run_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_demo() -> Result<(), String> {
    // Setup data directory
    let data_dir = PathBuf::from("./json_fingerprint_demo");
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir)
            .map_err(|e| format!("Failed to remove old dir: {}", e))?;
    }
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    println!("📚 Step 1: Initializing Components\n");

    // Initialize DataDistributionManager
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?,
    ));
    println!("✓ DataDistributionManager initialized");

    // Initialize embedding generator
    let embedder = EmbeddingGenerator::with_download_progress(true)
        .map_err(|e| format!("Failed to create embedder: {}", e))?;

    if !embedder.is_download_complete() {
        println!("⏳ Downloading embedding model...");
        embedder
            .wait_for_download(300)
            .map_err(|e| format!("Download failed: {}", e))?;
    }
    println!(
        "✓ Embedding generator ready (dimension: {})\n",
        embedder.dimension()
    );

    // Configure JSON fingerprinting
    let config = JsonFingerprintConfig {
        include_fields: None,
        exclude_fields: None,
        include_field_names: true,
        normalize_values: true,
        max_depth: 5,
        sort_keys: true,
    };

    let fp_manager = JsonFingerprintManager::new(manager.clone(), embedder, config);
    println!("✓ JSON Fingerprint Manager configured\n");

    println!("📝 Step 2: Storing JSON Documents\n");

    // Store various JSON documents - use a vec of tuples
    let documents = vec![
        (
            "user_1",
            r#"{
                "type": "user",
                "name": "John Doe",
                "email": "john@example.com",
                "age": 30,
                "address": {
                    "city": "New York",
                    "country": "USA",
                    "zip": "10001"
                },
                "interests": ["programming", "reading", "gaming"],
                "active": true
            }"#,
            vec![("category".to_string(), "user".to_string())],
        ),
        (
            "user_2",
            r#"{
                "type": "user",
                "name": "Jane Smith",
                "email": "jane@example.com",
                "age": 28,
                "address": {
                    "city": "San Francisco",
                    "country": "USA",
                    "zip": "94105"
                },
                "interests": ["hiking", "photography", "travel"],
                "active": true
            }"#,
            vec![("category".to_string(), "user".to_string())],
        ),
        (
            "user_3",
            r#"{
                "type": "user",
                "name": "Bob Johnson",
                "email": "bob@example.com",
                "age": 35,
                "address": {
                    "city": "Chicago",
                    "country": "USA",
                    "zip": "60601"
                },
                "interests": ["sports", "music", "movies"],
                "active": false
            }"#,
            vec![("category".to_string(), "user".to_string())],
        ),
        (
            "product_1",
            r#"{
                "type": "product",
                "name": "MacBook Pro",
                "price": 1999.99,
                "category": "electronics",
                "specs": {
                    "cpu": "Intel i7",
                    "ram": "16GB",
                    "storage": "512GB SSD",
                    "gpu": "Integrated"
                },
                "in_stock": true
            }"#,
            vec![("category".to_string(), "product".to_string())],
        ),
        (
            "product_2",
            r#"{
                "type": "product",
                "name": "Gaming Laptop",
                "price": 1499.99,
                "category": "electronics",
                "specs": {
                    "cpu": "Intel i9",
                    "ram": "32GB",
                    "storage": "1TB SSD",
                    "gpu": "RTX 3080"
                },
                "in_stock": true
            }"#,
            vec![("category".to_string(), "product".to_string())],
        ),
        (
            "product_3",
            r#"{
                "type": "product",
                "name": "Budget Laptop",
                "price": 599.99,
                "category": "electronics",
                "specs": {
                    "cpu": "Intel i3",
                    "ram": "8GB",
                    "storage": "256GB SSD",
                    "gpu": "Integrated"
                },
                "in_stock": false
            }"#,
            vec![("category".to_string(), "product".to_string())],
        ),
        (
            "support_ticket_1",
            r#"{
                "type": "ticket",
                "title": "Login Issue",
                "description": "User cannot log into the application",
                "priority": "high",
                "status": "open",
                "user_id": "user_1"
            }"#,
            vec![("category".to_string(), "ticket".to_string())],
        ),
        (
            "support_ticket_2",
            r#"{
                "type": "ticket",
                "title": "Payment Failed",
                "description": "Credit card payment not processing",
                "priority": "critical",
                "status": "in_progress",
                "user_id": "user_2"
            }"#,
            vec![("category".to_string(), "ticket".to_string())],
        ),
    ];

    let total_docs = documents.len();
    let start = Instant::now();

    for (id, json_str, metadata) in &documents {
        let json = json_from_str(json_str).unwrap();
        let metadata_map: HashMap<String, String> = metadata.iter().cloned().collect();
        fp_manager.store_document(id, json, metadata_map)?;
        println!("  ✓ Stored document: {}", id);
    }
    let duration = start.elapsed();

    println!("\n✓ Stored {} documents in {:.2?}", total_docs, duration);

    // Display storage statistics
    let stats = fp_manager.get_stats();
    println!("\n📊 Storage Statistics:");
    println!("  - Total records: {}", stats.total_records);
    println!("  - Shard distribution: {:?}", stats.shard_distribution);
    println!("  - Load balance: {:.3}\n", stats.load_balance_score);

    println!("🔍 Step 3: Whole Document Similarity Search\n");

    // Query: Find similar users
    let query_user = json_from_str(
        r#"{
        "type": "user",
        "name": "Jonathan Doe",
        "email": "jonathan.doe@email.com",
        "age": 32,
        "city": "New York City"
    }"#,
    )
    .unwrap();

    println!("Query: Find users similar to:");
    println!("{}", to_pretty_json(&query_user)?);

    let results = fp_manager.find_similar_documents(&query_user, 0.3, 3)?;
    println!("\nTop 3 Similar Users:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. {} (similarity: {:.3})",
            i + 1,
            result.document_id,
            result.similarity
        );
        if let Some(name) = result.content.get("name") {
            println!("     Name: {}", name);
        }
        if let Some(city) = result.content.get("address").and_then(|a| a.get("city")) {
            println!("     City: {}", city);
        }
    }

    println!("\n🔍 Step 4: Field-Specific Similarity Search\n");

    // Query: Find products with similar specs
    let query_specs = json_from_str(
        r#"{
        "cpu": "Intel i9",
        "ram": "32GB",
        "gpu": "RTX 3080"
    }"#,
    )
    .unwrap();

    println!("Query: Find products with similar specs:");
    println!("{}", to_pretty_json(&query_specs)?);

    let results = fp_manager.find_similar_by_field("specs", &query_specs, 0.3, 3)?;
    println!("\nTop 3 Similar Products:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. {} (similarity: {:.3})",
            i + 1,
            result.document_id,
            result.similarity
        );
        if let Some(name) = result.content.get("name") {
            println!("     Name: {}", name);
        }
        if let Some(specs) = result.content.get("specs") {
            if let Some(cpu) = specs.get("cpu") {
                println!("     CPU: {}", cpu);
            }
            if let Some(gpu) = specs.get("gpu") {
                println!("     GPU: {}", gpu);
            }
        }
    }

    println!("\n🔍 Step 5: Multi-Field Weighted Search\n");

    // Multi-field query: Find products with specific criteria
    let mut field_queries = HashMap::new();
    field_queries.insert("name".to_string(), json_from_str(r#""Gaming""#).unwrap());
    field_queries.insert("specs.gpu".to_string(), json_from_str(r#""RTX""#).unwrap());
    field_queries.insert("price".to_string(), json_from_str(r#"1500"#).unwrap());

    let mut weights = HashMap::new();
    weights.insert("name".to_string(), 0.3);
    weights.insert("specs.gpu".to_string(), 0.5);
    weights.insert("price".to_string(), 0.2);

    println!("Multi-field query with weights:");
    println!("  - Name similarity (weight 0.3): 'Gaming'");
    println!("  - GPU similarity (weight 0.5): 'RTX'");
    println!("  - Price proximity (weight 0.2): ~1500");

    let results = fp_manager.multi_field_search(field_queries, weights, 0.2, 3)?;
    println!("\nTop 3 Results:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. {} (score: {:.3})",
            i + 1,
            result.document_id,
            result.similarity
        );
        if let Some(name) = result.content.get("name") {
            println!("     Name: {}", name);
        }
        if let Some(price) = result.content.get("price") {
            println!("     Price: ${}", price);
        }
        if let Some(specs) = result.content.get("specs") {
            if let Some(gpu) = specs.get("gpu") {
                println!("     GPU: {}", gpu);
            }
        }
    }

    println!("\n🔍 Step 6: Support Ticket Search\n");

    // Query: Find similar support tickets
    let query_ticket = json_from_str(
        r#"{
        "title": "Login Problem",
        "description": "Cannot access account",
        "priority": "high"
    }"#,
    )
    .unwrap();

    println!("Query: Find similar support tickets:");
    println!("{}", to_pretty_json(&query_ticket)?);

    let results = fp_manager.find_similar_documents(&query_ticket, 0.2, 3)?;
    println!("\nTop 3 Similar Tickets:");
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. {} (similarity: {:.3})",
            i + 1,
            result.document_id,
            result.similarity
        );
        if let Some(title) = result.content.get("title") {
            println!("     Title: {}", title);
        }
        if let Some(priority) = result.content.get("priority") {
            println!("     Priority: {}", priority);
        }
        if let Some(status) = result.content.get("status") {
            println!("     Status: {}", status);
        }
    }

    println!("\n🔍 Step 7: Document Retrieval and Verification\n");

    // Retrieve a specific document
    if let Some(doc) = fp_manager.get_document("user_1")? {
        println!("Retrieved document 'user_1':");
        println!("{}", to_pretty_json(&doc.content)?);
        println!("Fingerprint dimension: {}", doc.fingerprint.len());
        println!("Field fingerprints: {}", doc.field_fingerprints.len());
        println!("Metadata: {:?}", doc.metadata);
        println!("Created at: {}", doc.created_at);
    }

    println!("\n🔍 Step 8: Index Statistics\n");

    let index_stats = fp_manager.get_index_stats()?;
    println!("Index Statistics:");
    println!(
        "  - Total documents: {}",
        index_stats.get("total_documents").unwrap_or(&0)
    );
    println!(
        "  - Last updated: {}",
        index_stats.get("last_updated").unwrap_or(&0)
    );

    println!("\n🔍 Step 9: Performance Metrics\n");

    // Benchmark similarity search
    let iterations = 10;
    let query = json_from_str(r#"{"type": "user", "city": "New York"}"#).unwrap();

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = fp_manager.find_similar_documents(&query, 0.3, 5)?;
    }
    let duration = start.elapsed();

    println!("Search Performance ({} iterations):", iterations);
    println!("  - Total time: {:.2?}", duration);
    println!("  - Average time: {:.2?}", duration / iterations as u32);

    println!("\n🔍 Step 10: Document Update Demo\n");

    // Update a document
    let updated_user = json_from_str(
        r#"{
        "type": "user",
        "name": "John Doe Updated",
        "email": "john.updated@example.com",
        "age": 31,
        "address": {
            "city": "New York",
            "country": "USA",
            "zip": "10002"
        },
        "interests": ["programming", "reading", "gaming", "AI"],
        "active": true
    }"#,
    )
    .unwrap();

    let mut metadata = HashMap::new();
    metadata.insert("category".to_string(), "user".to_string());
    metadata.insert("updated".to_string(), "true".to_string());

    println!("Updating document 'user_1'...");
    fp_manager.update_document("user_1", updated_user, metadata)?;
    println!("✓ Document updated successfully");

    // Verify update
    if let Some(doc) = fp_manager.get_document("user_1")? {
        println!("\nUpdated document:");
        println!("  Name: {}", doc.content.get("name").unwrap());
        println!("  Email: {}", doc.content.get("email").unwrap());
        println!("  Age: {}", doc.content.get("age").unwrap());
        println!("  Interests: {:?}", doc.content.get("interests").unwrap());
        println!("  Metadata: {:?}", doc.metadata);
    }

    println!("\n🔍 Step 11: Delete Document Demo\n");

    println!("Deleting document 'product_3'...");
    let deleted = fp_manager.delete_document("product_3")?;
    if deleted {
        println!("✓ Document deleted successfully");
    }

    // Verify deletion
    if fp_manager.get_document("product_3")?.is_none() {
        println!("✓ Document no longer exists in database");
    }

    println!("\n📊 Step 12: Final Statistics\n");

    let final_stats = fp_manager.get_stats();
    let final_index_stats = fp_manager.get_index_stats()?;

    println!("Final Database Statistics:");
    println!("  - Total records: {}", final_stats.total_records);
    println!(
        "  - Shard distribution: {:?}",
        final_stats.shard_distribution
    );
    println!("  - Load balance: {:.3}", final_stats.load_balance_score);
    println!(
        "  - Documents in index: {}",
        final_index_stats.get("total_documents").unwrap_or(&0)
    );

    println!("\n✅ Demo completed successfully!");
    println!("\n📊 Summary of JSON Fingerprinting Features:");
    println!("  ✓ Document storage with automatic fingerprinting");
    println!("  ✓ Whole document similarity search");
    println!("  ✓ Field-specific similarity search");
    println!("  ✓ Multi-field weighted search");
    println!("  ✓ Document update and delete operations");
    println!("  ✓ Persistent index storage");
    println!("  ✓ Configurable field inclusion/exclusion");
    println!("  ✓ Support for nested JSON structures");
    println!("  ✓ Fast vector-based similarity search");

    // Cleanup
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    Ok(())
}
