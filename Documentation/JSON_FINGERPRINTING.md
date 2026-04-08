```markdown
# JSON Fingerprinting Documentation

## Overview

The JSON Fingerprinting module provides semantic search capabilities for JSON documents using vector embeddings. It automatically generates fingerprints (embeddings) for entire JSON documents and individual fields, enabling similarity search at both document and field levels. The module integrates with DataDistributionManager for persistent storage and FastEmbed for high-quality embeddings.

## Features

- **Document Fingerprinting** - Automatically generates vector embeddings for JSON documents
- **Field-Level Fingerprinting** - Creates separate embeddings for individual fields
- **Whole Document Similarity** - Find similar documents based on overall structure and content
- **Field-Specific Search** - Search by specific fields (supports nested paths like "address.city")
- **Multi-Field Weighted Search** - Combine multiple fields with different weights
- **Persistent Storage** - Stores documents, fingerprints, and index in DataDistributionManager
- **CRUD Operations** - Create, Read, Update, Delete JSON documents
- **Configurable Fingerprinting** - Include/exclude fields, normalize values, sort keys
- **Index Management** - Maintains document index for efficient queries
- **Metadata Support** - Store and query additional metadata with documents

## Quick Start

```rust
use bund_blobstore::common::json_fingerprint::{
    JsonFingerprintManager, JsonFingerprintConfig, json_from_str
};
use bund_blobstore::common::embeddings::EmbeddingGenerator;
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use std::sync::Arc;
use parking_lot::RwLock;

// Initialize components
let manager = Arc::new(RwLock::new(
    DataDistributionManager::new("./data", DistributionStrategy::RoundRobin)?
));
let embedder = EmbeddingGenerator::new()?;
let config = JsonFingerprintConfig::default();
let fp_manager = JsonFingerprintManager::new(manager, embedder, config);

// Store a JSON document
let json = json_from_str(r#"{"name": "John Doe", "age": 30, "city": "New York"}"#)?;
fp_manager.store_document("user_1", json, HashMap::new())?;

// Find similar documents
let query = json_from_str(r#"{"name": "Johnathan Doe", "age": 31, "city": "NYC"}"#)?;
let results = fp_manager.find_similar_documents(&query, 0.3, 5)?;

for result in results {
    println!("Found: {} (similarity: {:.3})", result.document_id, result.similarity);
}
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["full"] }
```

## Core Components

### JsonFingerprintConfig

Configuration for fingerprint generation:

```rust
pub struct JsonFingerprintConfig {
    pub include_fields: Option<Vec<String>>,  // Fields to include (empty = all)
    pub exclude_fields: Option<Vec<String>>,  // Fields to exclude
    pub include_field_names: bool,            // Include field names in embedding
    pub normalize_values: bool,               // Normalize values (lowercase, trim)
    pub max_depth: usize,                     // Maximum nesting depth
    pub sort_keys: bool,                      // Sort object keys for consistency
}
```

### JsonDocument

Stored document structure:

```rust
pub struct JsonDocument {
    pub id: String,                           // Document identifier
    pub content: Value,                       // JSON content
    pub fingerprint: Vec<f32>,                // Document embedding
    pub field_fingerprints: HashMap<String, Vec<f32>>, // Field embeddings
    pub metadata: HashMap<String, String>,    // Additional metadata
    pub created_at: i64,                      // Creation timestamp
}
```

### JsonSearchResult

Search result structure:

```rust
pub struct JsonSearchResult {
    pub document_id: String,                  // Document identifier
    pub content: Value,                       // JSON content
    pub similarity: f32,                      // Overall similarity score
    pub field_similarities: HashMap<String, f32>, // Per-field scores
    pub metadata: HashMap<String, String>,    // Document metadata
}
```

## Usage Examples

### 1. Basic Document Storage

```rust
use bund_blobstore::common::json_fingerprint::{JsonFingerprintManager, json_from_str};

let config = JsonFingerprintConfig::default();
let fp_manager = JsonFingerprintManager::new(manager, embedder, config);

// Store a simple document
let user = json_from_str(r#"{
    "name": "Alice Johnson",
    "email": "alice@example.com",
    "age": 28,
    "active": true
}"#)?;

let mut metadata = HashMap::new();
metadata.insert("source".to_string(), "import".to_string());

fp_manager.store_document("user_001", user, metadata)?;

// Retrieve the document
if let Some(doc) = fp_manager.get_document("user_001")? {
    println!("Document: {}", serde_json::to_string_pretty(&doc.content)?);
    println!("Fingerprint dimension: {}", doc.fingerprint.len());
}
```

### 2. Whole Document Similarity Search

```rust
// Query for similar documents
let query = json_from_str(r#"{
    "name": "Bob Smith",
    "email": "bob@example.com",
    "age": 30
}"#)?;

let results = fp_manager.find_similar_documents(&query, 0.3, 5)?;

println!("Found {} similar documents:", results.len());
for result in results {
    println!("  - {} (similarity: {:.3})", result.document_id, result.similarity);
    if let Some(name) = result.content.get("name") {
        println!("    Name: {}", name);
    }
}
```

### 3. Field-Specific Similarity Search

```rust
// Search for products with similar specifications
let specs = json_from_str(r#"{
    "cpu": "Intel i9",
    "ram": "32GB",
    "gpu": "RTX 3080"
}"#)?;

let results = fp_manager.find_similar_by_field("specs", &specs, 0.3, 5)?;

for result in results {
    println!("Product: {}", result.document_id);
    if let Some(product_specs) = result.content.get("specs") {
        println!("  CPU: {}", product_specs.get("cpu").unwrap());
        println!("  RAM: {}", product_specs.get("ram").unwrap());
    }
}
```

### 4. Nested Field Search

```rust
// Search by nested field using dot notation
let address_query = json_from_str(r#"{"city": "New York", "country": "USA"}"#)?;

// Search in the "address" field
let results = fp_manager.find_similar_by_field("address", &address_query, 0.3, 5)?;

// Search deeper nested field
let zip_query = json_from_str(r#"{"zip": "10001"}"#)?;
let results = fp_manager.find_similar_by_field("address.zip", &zip_query, 0.5, 3)?;
```

### 5. Multi-Field Weighted Search

```rust
use std::collections::HashMap;

// Define field queries with different weights
let mut field_queries = HashMap::new();
field_queries.insert("name".to_string(), json_from_str(r#""Gaming Laptop""#)?);
field_queries.insert("specs.gpu".to_string(), json_from_str(r#""RTX 3080""#)?);
field_queries.insert("price".to_string(), json_from_str(r#"1500"#)?);

let mut weights = HashMap::new();
weights.insert("name".to_string(), 0.3);      // 30% weight on name similarity
weights.insert("specs.gpu".to_string(), 0.5); // 50% weight on GPU
weights.insert("price".to_string(), 0.2);     // 20% weight on price

let results = fp_manager.multi_field_search(field_queries, weights, 0.2, 5)?;

for result in results {
    println!("Document: {} (score: {:.3})", result.document_id, result.similarity);
    for (field, score) in &result.field_similarities {
        println!("  {} similarity: {:.3}", field, score);
    }
}
```

### 6. Configuration Customization

```rust
// Only include specific fields in fingerprint
let config = JsonFingerprintConfig {
    include_fields: Some(vec!["name".to_string(), "email".to_string()]),
    exclude_fields: None,
    include_field_names: true,
    normalize_values: true,
    max_depth: 5,
    sort_keys: true,
};

// Exclude sensitive fields
let config = JsonFingerprintConfig {
    include_fields: None,
    exclude_fields: Some(vec!["password".to_string(), "ssn".to_string()]),
    include_field_names: true,
    normalize_values: true,
    max_depth: 5,
    sort_keys: true,
};

// Disable field name inclusion (value-only comparison)
let config = JsonFingerprintConfig {
    include_field_names: false,
    ..Default::default()
};

let fp_manager = JsonFingerprintManager::new(manager, embedder, config);
```

### 7. Document Management

```rust
// Update a document
let updated_user = json_from_str(r#"{
    "name": "Alice Johnson Updated",
    "email": "alice.new@example.com",
    "age": 29,
    "active": true
}"#)?;

let mut metadata = HashMap::new();
metadata.insert("updated".to_string(), "true".to_string());

fp_manager.update_document("user_001", updated_user, metadata)?;

// Delete a document
let deleted = fp_manager.delete_document("user_001")?;
if deleted {
    println!("Document deleted successfully");
}

// Check if document exists
if let Some(doc) = fp_manager.get_document("user_001")? {
    println!("Document exists: {}", doc.id);
} else {
    println!("Document not found");
}
```

### 8. Working with Metadata

```rust
// Store document with rich metadata
let mut metadata = HashMap::new();
metadata.insert("category".to_string(), "user".to_string());
metadata.insert("source".to_string(), "api".to_string());
metadata.insert("version".to_string(), "1.0".to_string());
metadata.insert("environment".to_string(), "production".to_string());

fp_manager.store_document("user_002", user_json, metadata)?;

// Retrieve and access metadata
if let Some(doc) = fp_manager.get_document("user_002")? {
    println!("Category: {}", doc.metadata.get("category").unwrap());
    println!("Source: {}", doc.metadata.get("source").unwrap());
}
```

### 9. Index Management

```rust
// Get index statistics
let stats = fp_manager.get_index_stats()?;
println!("Total documents: {}", stats.get("total_documents").unwrap());
println!("Last updated: {}", stats.get("last_updated").unwrap());

// Get all document IDs
let all_ids = fp_manager.get_all_ids()?;
println!("All document IDs: {:?}", all_ids);

// Flush index to ensure persistence
fp_manager.flush_index()?;
```

### 10. Performance Optimization

```rust
// Batch document storage
let documents = vec![
    ("doc1", r#"{"id": 1, "data": "first"}"#),
    ("doc2", r#"{"id": 2, "data": "second"}"#),
    ("doc3", r#"{"id": 3, "data": "third"}"#),
];

for (id, json_str) in documents {
    let json = json_from_str(json_str)?;
    fp_manager.store_document(id, json, HashMap::new())?;
}

// Benchmark similarity search
use std::time::Instant;

let start = Instant::now();
for _ in 0..100 {
    let _ = fp_manager.find_similar_documents(&query, 0.3, 10)?;
}
let duration = start.elapsed();
println!("Average search time: {:?}", duration / 100);
```

### 11. Complex JSON Structures

```rust
// Store deeply nested JSON
let complex_json = json_from_str(r#"{
    "level1": {
        "level2": {
            "level3": {
                "value": "deeply nested",
                "array": [1, 2, 3, 4, 5],
                "mixed": ["string", 123, true, null]
            }
        }
    },
    "metadata": {
        "created_by": "system",
        "version": "2.0"
    }
}"#)?;

fp_manager.store_document("complex_1", complex_json, HashMap::new())?;

// Search within nested structures
let nested_query = json_from_str(r#"{"level2": {"level3": {"value": "nested"}}}"#)?;
let results = fp_manager.find_similar_documents(&nested_query, 0.2, 3)?;
```

### 12. Real-World Use Case: E-commerce Product Search

```rust
// Store products with detailed specifications
let products = vec![
    ("laptop_1", r#"{
        "type": "laptop",
        "brand": "Apple",
        "model": "MacBook Pro",
        "specs": {"cpu": "M1 Pro", "ram": "16GB", "storage": "512GB"},
        "price": 1999,
        "category": "premium"
    }"#),
    ("laptop_2", r#"{
        "type": "laptop",
        "brand": "Dell",
        "model": "XPS 15",
        "specs": {"cpu": "Intel i7", "ram": "32GB", "storage": "1TB"},
        "price": 1899,
        "category": "premium"
    }"#),
    ("laptop_3", r#"{
        "type": "laptop",
        "brand": "Lenovo",
        "model": "ThinkPad",
        "specs": {"cpu": "Intel i5", "ram": "8GB", "storage": "256GB"},
        "price": 899,
        "category": "budget"
    }"#),
];

for (id, json_str) in products {
    let json = json_from_str(json_str)?;
    fp_manager.store_document(id, json, HashMap::new())?;
}

// Search for premium laptops with high specs
let query = json_from_str(r#"{
    "type": "laptop",
    "category": "premium",
    "specs": {"ram": "32GB", "storage": "1TB"}
}"#)?;

let results = fp_manager.find_similar_documents(&query, 0.3, 5)?;

println!("Matching products:");
for result in results {
    println!("  - {} {} (price: ${})",
        result.content.get("brand").unwrap(),
        result.content.get("model").unwrap(),
        result.content.get("price").unwrap()
    );
}
```

## Configuration Options

| Option | Description | Default |
|--------|-------------|---------|
| `include_fields` | Fields to include in fingerprint (None = all) | `None` |
| `exclude_fields` | Fields to exclude from fingerprint | `None` |
| `include_field_names` | Include field names in embedding | `true` |
| `normalize_values` | Normalize values (lowercase, trim) | `true` |
| `max_depth` | Maximum nesting depth for processing | `5` |
| `sort_keys` | Sort object keys for consistent fingerprints | `true` |

## Performance Characteristics

| Operation | Time Complexity | Notes |
|-----------|----------------|-------|
| Store Document | O(n) + embedding | n = document size |
| Similarity Search | O(m) + embedding | m = number of documents |
| Field Search | O(m) | m = number of documents |
| Multi-Field Search | O(m * f) | f = number of fields |
| Get Document | O(1) | Direct lookup |

## Error Handling

```rust
match fp_manager.store_document("id", json, metadata) {
    Ok(_) => println!("Document stored successfully"),
    Err(e) => eprintln!("Failed to store document: {}", e),
}

match fp_manager.find_similar_documents(&query, 0.3, 5) {
    Ok(results) => println!("Found {} results", results.len()),
    Err(e) => eprintln!("Search failed: {}", e),
}
```

## Best Practices

1. **Field Selection** - Use `include_fields` or `exclude_fields` to focus on relevant content
2. **Threshold Tuning** - Start with 0.3-0.5 for broad matches, 0.7+ for exact matches
3. **Batch Operations** - Store documents in batches when possible
4. **Metadata Usage** - Use metadata for filtering rather than embedding
5. **Index Management** - Flush index periodically to ensure persistence
6. **Configuration** - Sort keys and normalize values for consistent fingerprints
7. **Nested Fields** - Use dot notation for deep nested field access

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Poor similarity results | Adjust threshold, check field configuration, normalize data |
| Slow search performance | Reduce number of documents, use field-specific search, add indexes |
| High memory usage | Process in batches, use streaming for large documents |
| Index not persisting | Call `flush_index()` after batch operations |

## See Also

- [Embeddings Module Documentation](./EMBEDDINGS.md)
- [Data Distribution Manager](./DATA_DISTRIBUTION.md)
- [JSON Fingerprinting Demo](../examples/json_fingerprint_demo.rs)

## License

This module is part of the Bund BlobStore project and is licensed under the same terms.
```
