use crate::common::embeddings::{EmbeddingGenerator, cosine_similarity};
use crate::data_distribution::DataDistributionManager;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracing::{debug, info};

pub type Result<T> = std::result::Result<T, String>;

/// Configuration for JSON fingerprinting
#[derive(Debug, Clone)]
pub struct JsonFingerprintConfig {
    pub include_fields: Option<Vec<String>>,
    pub exclude_fields: Option<Vec<String>>,
    pub include_field_names: bool,
    pub normalize_values: bool,
    pub max_depth: usize,
    pub sort_keys: bool,
}

impl Default for JsonFingerprintConfig {
    fn default() -> Self {
        Self {
            include_fields: None,
            exclude_fields: None,
            include_field_names: true,
            normalize_values: true,
            max_depth: 5,
            sort_keys: true,
        }
    }
}

/// JSON document with fingerprint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonDocument {
    pub id: String,
    pub content: Value,
    pub fingerprint: Vec<f32>,
    pub field_fingerprints: HashMap<String, Vec<f32>>,
    pub metadata: HashMap<String, String>,
    pub created_at: i64,
}

/// Document index stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DocumentIndex {
    ids: Vec<String>,
    last_updated: i64,
}

/// Search result for JSON similarity
#[derive(Debug, Clone)]
pub struct JsonSearchResult {
    pub document_id: String,
    pub content: Value,
    pub similarity: f32,
    pub field_similarities: HashMap<String, f32>,
    pub metadata: HashMap<String, String>,
}

/// JSON fingerprint manager with database-stored index
pub struct JsonFingerprintManager {
    manager: Arc<RwLock<DataDistributionManager>>,
    embedder: EmbeddingGenerator,
    config: JsonFingerprintConfig,
    index_key: String,
}

impl JsonFingerprintManager {
    /// Create a new JSON fingerprint manager
    pub fn new(
        manager: Arc<RwLock<DataDistributionManager>>,
        embedder: EmbeddingGenerator,
        config: JsonFingerprintConfig,
    ) -> Self {
        Self {
            manager,
            embedder,
            config,
            index_key: "__document_index__".to_string(),
        }
    }

    /// Load document index from database
    fn load_index(&self) -> Result<DocumentIndex> {
        let manager = self.manager.read();
        match manager.get(&self.index_key) {
            Ok(Some(data)) => serde_json::from_slice(&data)
                .map_err(|e| format!("Failed to deserialize index: {}", e)),
            Ok(None) => {
                // Create new index if none exists
                Ok(DocumentIndex {
                    ids: Vec::new(),
                    last_updated: chrono::Utc::now().timestamp(),
                })
            }
            Err(e) => Err(format!("Failed to load index: {}", e)),
        }
    }

    /// Save document index to database
    fn save_index(&self, index: &DocumentIndex) -> Result<()> {
        let data =
            serde_json::to_vec(index).map_err(|e| format!("Failed to serialize index: {}", e))?;

        self.manager
            .write()
            .put(&self.index_key, &data, None)
            .map_err(|e| format!("Failed to save index: {}", e))
    }

    /// Flush index to ensure persistence
    pub fn flush_index(&self) -> Result<()> {
        let index = self.load_index()?;
        self.save_index(&index)?;

        // Verify flush worked
        let verify_index = self.load_index()?;
        if verify_index.ids.len() == index.ids.len() {
            Ok(())
        } else {
            Err("Flush verification failed".to_string())
        }
    }

    /// Add document ID to index with retry
    fn add_to_index(&self, id: &str) -> Result<()> {
        let mut retries = 3;
        let mut last_error = String::new();

        while retries > 0 {
            let mut index = self.load_index()?;
            if !index.ids.contains(&id.to_string()) {
                index.ids.push(id.to_string());
                index.last_updated = chrono::Utc::now().timestamp();

                match self.save_index(&index) {
                    Ok(_) => {
                        // Verify the save worked
                        match self.load_index() {
                            Ok(verify_index) => {
                                if verify_index.ids.contains(&id.to_string()) {
                                    info!(
                                        "Added document {} to index, total: {}",
                                        id,
                                        verify_index.ids.len()
                                    );
                                    return Ok(());
                                } else {
                                    last_error = "Verification failed".to_string();
                                    retries -= 1;
                                    if retries == 0 {
                                        return Err(format!(
                                            "Failed to verify index save for document {}: {}",
                                            id, last_error
                                        ));
                                    }
                                    std::thread::sleep(std::time::Duration::from_millis(10));
                                    continue;
                                }
                            }
                            Err(e) => {
                                last_error =
                                    format!("Failed to load index for verification: {}", e);
                                retries -= 1;
                                if retries == 0 {
                                    return Err(last_error);
                                }
                                std::thread::sleep(std::time::Duration::from_millis(10));
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        last_error = format!("Failed to save index: {}", e);
                        retries -= 1;
                        if retries > 0 {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            continue;
                        }
                        return Err(format!(
                            "Failed to save index after retries: {}",
                            last_error
                        ));
                    }
                }
            } else {
                return Ok(());
            }
        }

        Err(format!(
            "Failed to add document {} to index after retries: {}",
            id, last_error
        ))
    }

    /// Remove document ID from index with retry
    fn remove_from_index(&self, id: &str) -> Result<()> {
        let mut retries = 3;

        while retries > 0 {
            let mut index = self.load_index()?;
            if let Some(pos) = index.ids.iter().position(|x| x == id) {
                index.ids.remove(pos);
                index.last_updated = chrono::Utc::now().timestamp();

                if let Err(e) = self.save_index(&index) {
                    retries -= 1;
                    if retries == 0 {
                        return Err(format!("Failed to save index: {}", e));
                    }
                    thread::sleep(Duration::from_millis(10));
                    continue;
                }

                // Verify removal
                let verify_index = self.load_index()?;
                if !verify_index.ids.contains(&id.to_string()) {
                    info!("Removed document {} from index", id);
                    return Ok(());
                }
            }
            return Ok(());
        }

        Ok(())
    }

    /// Get all document IDs from index
    pub fn get_all_ids(&self) -> Result<Vec<String>> {
        let index = self.load_index()?;
        Ok(index.ids)
    }

    /// Generate fingerprint for a JSON value
    pub fn generate_fingerprint(&self, value: &Value, depth: usize) -> Result<Vec<f32>> {
        let text_repr = self.json_to_text(value, depth)?;
        self.embedder.embed(&text_repr)
    }

    /// Generate fingerprints for all fields of a JSON object
    pub fn generate_field_fingerprints(&self, obj: &Value) -> Result<HashMap<String, Vec<f32>>> {
        let mut field_fingerprints = HashMap::new();

        if let Value::Object(map) = obj {
            for (key, value) in map {
                if self.should_include_field(key) {
                    let fingerprint = self.generate_fingerprint(value, 0)?;
                    field_fingerprints.insert(key.clone(), fingerprint);
                }
            }
        }

        Ok(field_fingerprints)
    }

    /// Convert JSON to text representation for embedding
    fn json_to_text(&self, value: &Value, depth: usize) -> Result<String> {
        if depth > self.config.max_depth {
            return Ok("[MAX_DEPTH]".to_string());
        }

        match value {
            Value::Null => Ok("null".to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Number(n) => Ok(n.to_string()),
            Value::String(s) => {
                let mut text = s.clone();
                if self.config.normalize_values {
                    text = text.to_lowercase().trim().to_string();
                }
                Ok(text)
            }
            Value::Array(arr) => {
                let mut elements = Vec::new();
                for item in arr {
                    elements.push(self.json_to_text(item, depth + 1)?);
                }
                Ok(format!("[{}]", elements.join(", ")))
            }
            Value::Object(obj) => {
                let mut pairs = Vec::new();
                let mut entries: Vec<(&String, &Value)> = obj.iter().collect();

                if self.config.sort_keys {
                    entries.sort_by_key(|(k, _)| *k);
                }

                for (key, val) in entries {
                    if self.should_include_field(key) {
                        let val_text = self.json_to_text(val, depth + 1)?;
                        if self.config.include_field_names {
                            pairs.push(format!("{}: {}", key, val_text));
                        } else {
                            pairs.push(val_text);
                        }
                    }
                }

                Ok(format!("{{{}}}", pairs.join(", ")))
            }
        }
    }

    /// Check if a field should be included in fingerprint
    fn should_include_field(&self, field: &str) -> bool {
        if let Some(exclude) = &self.config.exclude_fields {
            if exclude.contains(&field.to_string()) {
                return false;
            }
        }

        if let Some(include) = &self.config.include_fields {
            return include.contains(&field.to_string());
        }

        true
    }

    /// Store a JSON document
    pub fn store_document(
        &self,
        id: &str,
        content: Value,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        let fingerprint = self.generate_fingerprint(&content, 0)?;
        let field_fingerprints = self.generate_field_fingerprints(&content)?;

        let doc = JsonDocument {
            id: id.to_string(),
            content,
            fingerprint,
            field_fingerprints,
            metadata,
            created_at: chrono::Utc::now().timestamp(),
        };

        let data = serde_json::to_vec(&doc).map_err(|e| format!("Failed to serialize: {}", e))?;

        self.manager
            .write()
            .put(id, &data, None)
            .map_err(|e| format!("Failed to store document: {}", e))?;

        let fingerprint_key = format!("fp_{}", id);
        let fingerprint_data: Vec<u8> = doc
            .fingerprint
            .iter()
            .flat_map(|&f| f.to_le_bytes())
            .collect();
        self.manager
            .write()
            .put(&fingerprint_key, &fingerprint_data, None)
            .map_err(|e| format!("Failed to store fingerprint: {}", e))?;

        // Add to database-stored index
        self.add_to_index(id)?;

        // Force a sync to ensure data is written
        self.flush_index()?;

        info!("Stored JSON document: {}", id);
        Ok(())
    }

    /// Retrieve a JSON document
    pub fn get_document(&self, id: &str) -> Result<Option<JsonDocument>> {
        let data = self
            .manager
            .read()
            .get(id)
            .map_err(|e| format!("Failed to retrieve document: {}", e))?;

        if let Some(data) = data {
            let doc: JsonDocument = serde_json::from_slice(&data)
                .map_err(|e| format!("Failed to deserialize: {}", e))?;
            Ok(Some(doc))
        } else {
            Ok(None)
        }
    }

    /// Find similar documents by whole JSON similarity
    pub fn find_similar_documents(
        &self,
        query_json: &Value,
        threshold: f32,
        top_k: usize,
    ) -> Result<Vec<JsonSearchResult>> {
        let query_fingerprint = self.generate_fingerprint(query_json, 0)?;
        self.search_by_fingerprint(&query_fingerprint, threshold, top_k)
    }

    /// Find similar documents by specific field similarity
    pub fn find_similar_by_field(
        &self,
        field: &str,
        query_json: &Value,
        threshold: f32,
        top_k: usize,
    ) -> Result<Vec<JsonSearchResult>> {
        let field_fingerprint = self.generate_fingerprint(query_json, 0)?;
        self.search_by_field_fingerprint(field, &field_fingerprint, threshold, top_k)
    }

    /// Search documents by fingerprint using database index
    fn search_by_fingerprint(
        &self,
        query_fingerprint: &[f32],
        threshold: f32,
        top_k: usize,
    ) -> Result<Vec<JsonSearchResult>> {
        let mut results = Vec::new();
        let all_ids = self.get_all_ids()?;
        let manager = self.manager.read();

        for doc_id in all_ids {
            if let Some(data) = manager
                .get(&doc_id)
                .map_err(|e| format!("Failed to retrieve document {}: {}", doc_id, e))?
            {
                if let Ok(doc) = serde_json::from_slice::<JsonDocument>(&data) {
                    let similarity = cosine_similarity(query_fingerprint, &doc.fingerprint);
                    if similarity >= threshold {
                        results.push(JsonSearchResult {
                            document_id: doc.id.clone(),
                            content: doc.content,
                            similarity,
                            field_similarities: HashMap::new(),
                            metadata: doc.metadata,
                        });
                    }
                }
            }
        }

        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(top_k);

        Ok(results)
    }

    /// Search documents by field fingerprint
    fn search_by_field_fingerprint(
        &self,
        field: &str,
        query_fingerprint: &[f32],
        threshold: f32,
        top_k: usize,
    ) -> Result<Vec<JsonSearchResult>> {
        let mut results = Vec::new();
        let all_ids = self.get_all_ids()?;
        let manager = self.manager.read();

        for doc_id in all_ids {
            if let Some(data) = manager
                .get(&doc_id)
                .map_err(|e| format!("Failed to retrieve document {}: {}", doc_id, e))?
            {
                if let Ok(doc) = serde_json::from_slice::<JsonDocument>(&data) {
                    if let Some(field_fp) = doc.field_fingerprints.get(field) {
                        let similarity = cosine_similarity(query_fingerprint, field_fp);
                        if similarity >= threshold {
                            results.push(JsonSearchResult {
                                document_id: doc.id.clone(),
                                content: doc.content,
                                similarity,
                                field_similarities: {
                                    let mut map = HashMap::new();
                                    map.insert(field.to_string(), similarity);
                                    map
                                },
                                metadata: doc.metadata,
                            });
                        }
                    }
                }
            }
        }

        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(top_k);

        Ok(results)
    }

    /// Multi-field similarity search
    pub fn multi_field_search(
        &self,
        field_queries: HashMap<String, Value>,
        weights: HashMap<String, f32>,
        threshold: f32,
        top_k: usize,
    ) -> Result<Vec<JsonSearchResult>> {
        let mut results = Vec::new();
        let all_ids = self.get_all_ids()?;
        let manager = self.manager.read();

        // Pre-compute query fingerprints for each field
        let mut query_fingerprints = HashMap::new();
        for (field, query_value) in &field_queries {
            // For nested fields, extract the actual value
            let value_to_embed = if field.contains('.') {
                self.extract_field_value(query_value, field)
            } else {
                query_value.clone()
            };
            let fp = self.generate_fingerprint(&value_to_embed, 0)?;
            query_fingerprints.insert(field.clone(), fp);
            info!("Query fingerprint generated for field: {}", field);
        }

        for doc_id in all_ids {
            if let Some(data) = manager
                .get(&doc_id)
                .map_err(|e| format!("Failed to retrieve document {}: {}", doc_id, e))?
            {
                if let Ok(doc) = serde_json::from_slice::<JsonDocument>(&data) {
                    let mut total_score = 0.0;
                    let mut field_similarities = HashMap::new();
                    let mut total_weight = 0.0;

                    for (field, query_fp) in &query_fingerprints {
                        let weight = weights.get(field).unwrap_or(&1.0);
                        total_weight += weight;

                        // Handle nested field paths
                        let field_fp = if field.contains('.') {
                            let parts: Vec<&str> = field.split('.').collect();
                            let mut current = &doc.content;
                            for part in parts {
                                if let Some(val) = current.get(part) {
                                    current = val;
                                } else {
                                    break;
                                }
                            }
                            // Generate fingerprint for the nested value
                            if let Ok(fp) = self.generate_fingerprint(current, 0) {
                                Some(fp)
                            } else {
                                None
                            }
                        } else {
                            doc.field_fingerprints.get(field).cloned()
                        };

                        if let Some(field_fp) = field_fp {
                            let similarity = cosine_similarity(query_fp, &field_fp);
                            field_similarities.insert(field.clone(), similarity);
                            total_score += similarity * weight;
                            debug!(
                                "Field {} similarity: {:.4}, weight: {}",
                                field, similarity, weight
                            );
                        }
                    }

                    let final_score = if total_weight > 0.0 {
                        total_score / total_weight
                    } else {
                        total_score
                    };

                    if final_score >= threshold {
                        results.push(JsonSearchResult {
                            document_id: doc.id.clone(),
                            content: doc.content,
                            similarity: final_score,
                            field_similarities,
                            metadata: doc.metadata,
                        });
                    }
                }
            }
        }

        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(top_k);

        if results.is_empty() {
            debug!(
                "No results found for multi-field search with threshold {}",
                threshold
            );
        }

        Ok(results)
    }

    /// Extract field value from JSON (supports nested paths)
    pub fn extract_field_value(&self, value: &Value, path: &str) -> Value {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            match current {
                Value::Object(obj) => {
                    if let Some(v) = obj.get(part) {
                        current = v;
                    } else {
                        return Value::Null;
                    }
                }
                _ => return Value::Null,
            }
        }

        current.clone()
    }

    /// Delete a document
    pub fn delete_document(&self, id: &str) -> Result<bool> {
        let existed = self
            .manager
            .write()
            .delete(id)
            .map_err(|e| format!("Failed to delete document: {}", e))?;

        let fingerprint_key = format!("fp_{}", id);
        let _ = self.manager.write().delete(&fingerprint_key);

        if existed {
            self.remove_from_index(id)?;
            info!("Deleted document: {}", id);
        }

        Ok(existed)
    }

    /// Update a document
    pub fn update_document(
        &self,
        id: &str,
        content: Value,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        self.delete_document(id)?;
        self.store_document(id, content, metadata)
    }

    /// Get statistics
    pub fn get_stats(&self) -> crate::data_distribution::DistributionStats {
        self.manager.read().get_stats()
    }

    /// Get index statistics
    pub fn get_index_stats(&self) -> Result<HashMap<String, usize>> {
        let index = self.load_index()?;
        let mut stats = HashMap::new();
        stats.insert("total_documents".to_string(), index.ids.len());
        stats.insert("last_updated".to_string(), index.last_updated as usize);
        Ok(stats)
    }
}

/// Helper function to create JSON from string
pub fn json_from_str(s: &str) -> Result<Value> {
    serde_json::from_str(s).map_err(|e| format!("Invalid JSON: {}", e))
}

/// Helper function to create pretty JSON string
pub fn to_pretty_json(value: &Value) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|e| format!("Failed to serialize JSON: {}", e))
}
