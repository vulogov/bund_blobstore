use crate::common::embeddings::{EmbeddingGenerator, cosine_similarity};
use crate::data_distribution::DataDistributionManager;
use parking_lot::{RwLock, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

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
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
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

    fn load_index(&self) -> Result<DocumentIndex> {
        let mg = self.manager.read();
        match mg.get(&self.index_key) {
            Ok(Some(ref data)) => serde_json::from_slice(data).map_err(|e| e.to_string()),
            _ => Ok(DocumentIndex::default()),
        }
    }

    fn load_index_with_lock(
        &self,
        mg: &RwLockWriteGuard<'_, DataDistributionManager>,
    ) -> Result<DocumentIndex> {
        match mg.get(&self.index_key) {
            Ok(Some(ref data)) => serde_json::from_slice(data).map_err(|e| e.to_string()),
            _ => Ok(DocumentIndex::default()),
        }
    }

    pub fn flush_index(&self) -> Result<()> {
        let index = self.load_index()?;
        let data = serde_json::to_vec(&index).map_err(|e| e.to_string())?;

        let mg = self.manager.write();
        let num_shards = mg.shard_count();

        // Saturation loop to ensure all shards have a copy of the index
        for _ in 0..(num_shards * 2) {
            mg.put(&self.index_key, &data, None)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn add_to_index(&self, id: &str) -> Result<()> {
        let mg = self.manager.write();
        let mut index = self.load_index_with_lock(&mg)?;

        if !index.ids.contains(&id.to_string()) {
            index.ids.push(id.to_string());
            index.last_updated = chrono::Utc::now().timestamp();

            let data = serde_json::to_vec(&index).map_err(|e| e.to_string())?;
            mg.put(&self.index_key, &data, None)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn get_all_ids(&self) -> Result<Vec<String>> {
        let index = self.load_index()?;
        Ok(index.ids)
    }

    pub fn generate_fingerprint(&self, value: &Value, depth: usize) -> Result<Vec<f32>> {
        let text_repr = self.json_to_text(value, depth)?;
        self.embedder.embed(&text_repr)
    }

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

    pub fn update_document(
        &self,
        id: &str,
        content: Value,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        let _ = self.delete_document(id);
        self.store_document(id, content, metadata)
    }

    pub fn store_document(
        &self,
        id: &str,
        content: Value,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        let whole_doc_fingerprint = self.generate_fingerprint(&content, 0)?;

        let mut field_fingerprints = HashMap::new();
        if let Some(obj) = content.as_object() {
            for (key, val) in obj {
                if let Ok(fp) = self.generate_fingerprint(val, 0) {
                    field_fingerprints.insert(key.clone(), fp);
                }
            }
        }

        let doc = JsonDocument {
            id: id.to_string(),
            content,
            fingerprint: whole_doc_fingerprint,
            field_fingerprints,
            metadata,
            created_at: chrono::Utc::now().timestamp(),
        };

        let data = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;

        {
            let mg = self.manager.write();
            mg.put(id, &data, None).map_err(|e| e.to_string())?;
        }

        self.add_to_index(id)?;
        Ok(())
    }

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

    pub fn find_similar_documents(
        &self,
        query_json: &Value,
        threshold: f32,
        top_k: usize,
    ) -> Result<Vec<JsonSearchResult>> {
        let query_fingerprint = self.generate_fingerprint(query_json, 0)?;
        self.search_by_fingerprint(&query_fingerprint, threshold, top_k)
    }

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

    pub fn calculate_cosine_similarity(&self, v1: &[f32], v2: &[f32]) -> f64 {
        if v1.len() != v2.len() || v1.is_empty() {
            return 0.0;
        }

        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..v1.len() {
            let a = v1[i] as f64;
            let b = v2[i] as f64;
            dot_product += a * b;
            norm_a += a * a;
            norm_b += b * b;
        }

        let magnitude = norm_a.sqrt() * norm_b.sqrt();
        if magnitude == 0.0 {
            0.0
        } else {
            dot_product / magnitude
        }
    }

    pub fn multi_field_weighted_search(
        &self,
        query_fields: HashMap<String, Value>,
        weights: HashMap<String, f64>,
        limit: usize,
    ) -> Result<Vec<(String, f64)>> {
        let index = self.load_index()?;
        let mut results: Vec<(String, f64)> = Vec::new();

        let mut query_fps = HashMap::new();
        for (field, val) in &query_fields {
            if let Ok(fp) = self.generate_fingerprint(val, 0) {
                query_fps.insert(field.clone(), fp);
            }
        }

        for id in &index.ids {
            if let Ok(Some(doc)) = self.get_document(id) {
                let mut weighted_score_sum = 0.0;
                let mut total_weight_applied = 0.0;

                for (field, query_fp) in &query_fps {
                    let weight = weights.get(field).copied().unwrap_or(1.0);
                    if let Some(stored_fp) = doc.field_fingerprints.get(field) {
                        let sim = self.calculate_cosine_similarity(query_fp, stored_fp);
                        let effective_sim = sim.max(0.0).powi(2);
                        let effective_weight = weight.powi(2);

                        weighted_score_sum += effective_sim * effective_weight;
                        total_weight_applied += effective_weight;
                    } else {
                        total_weight_applied += weight.powi(2);
                    }
                }

                if total_weight_applied > 0.0 {
                    let final_score = weighted_score_sum / total_weight_applied;
                    results.push((id.clone(), final_score));
                }
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(results.into_iter().take(limit).collect())
    }

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

        let mut query_fingerprints = HashMap::new();
        for (field, query_value) in &field_queries {
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
                            self.generate_fingerprint(current, 0).ok()
                        } else {
                            doc.field_fingerprints.get(field).cloned()
                        };

                        if let Some(fp) = field_fp {
                            let similarity = cosine_similarity(query_fp, &fp);
                            field_similarities.insert(field.clone(), similarity);
                            total_score += similarity * weight;
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
        Ok(results)
    }

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

    pub fn delete_document(&self, id: &str) -> Result<bool> {
        let mg = self.manager.write();
        let deleted = mg.delete(id).map_err(|e| e.to_string())?;

        if deleted {
            let mut index = self.load_index_with_lock(&mg)?;
            index.ids.retain(|x| x != id);
            index.last_updated = chrono::Utc::now().timestamp();

            let data = serde_json::to_vec(&index).map_err(|e| e.to_string())?;
            mg.put(&self.index_key, &data, None)
                .map_err(|e| e.to_string())?;
        }
        Ok(deleted)
    }

    pub fn get_stats(&self) -> crate::data_distribution::DistributionStats {
        self.manager.read().get_stats()
    }

    pub fn get_index_stats(&self) -> Result<HashMap<String, usize>> {
        let index = self.load_index()?;
        let mut stats = HashMap::new();
        stats.insert("total_documents".to_string(), index.ids.len());
        stats.insert("last_updated".to_string(), index.last_updated as usize);
        Ok(stats)
    }
}

pub fn json_from_str(s: &str) -> Result<Value> {
    serde_json::from_str(s).map_err(|e| format!("Invalid JSON: {}", e))
}

pub fn to_pretty_json(value: &Value) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(|e| format!("Failed to serialize JSON: {}", e))
}
