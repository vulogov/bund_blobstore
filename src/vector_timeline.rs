use crate::blobstore::BlobStore;
use crate::timeline::{
    TelemetryQuery, TelemetryRecord, TelemetryStore, TelemetryValue, TimeInterval,
};
use crate::vector::VectorStore;
use chrono::{Duration, Utc};
use ndarray::Array1;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Vector-enabled telemetry record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorTelemetryRecord {
    pub telemetry: TelemetryRecord,
    pub embedding: Option<Vec<f32>>,
    pub vector_searchable: bool,
}

/// Query combining time range and vector similarity
#[derive(Debug, Clone)]
pub struct VectorTimeQuery {
    pub time_interval: Option<TimeInterval>,
    pub vector_query: Option<String>,
    pub vector_weight: f32,
    pub time_weight: f32,
    pub keys: Option<Vec<String>>,
    pub sources: Option<Vec<String>>,
    pub limit: usize,
    pub min_similarity: f32,
}

impl Default for VectorTimeQuery {
    fn default() -> Self {
        VectorTimeQuery {
            time_interval: None,
            vector_query: None,
            vector_weight: 0.7,
            time_weight: 0.3,
            keys: None,
            sources: None,
            limit: 100,
            min_similarity: 0.3,
        }
    }
}

/// Result with combined time and vector relevance scores
#[derive(Debug, Clone)]
pub struct VectorTimeResult {
    pub record: TelemetryRecord,
    pub time_score: f64,
    pub vector_score: f64,
    pub combined_score: f64,
    pub time_distance_seconds: i64,
    pub similarity: f32,
}

/// Vector-enabled telemetry store with time-vector search
pub struct VectorTelemetryStore {
    #[allow(dead_code)]
    blob_store: BlobStore,
    telemetry_store: TelemetryStore,
    vector_store: VectorStore,
    embedding_cache: Arc<RwLock<HashMap<String, Array1<f32>>>>,
    time_embeddings: Arc<RwLock<HashMap<i64, Vec<String>>>>, // timestamp bucket -> record ids
}

impl VectorTelemetryStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create a single blob store instance
        let blob_store = BlobStore::open(path.as_ref())?;

        // Create telemetry store that uses the same blob store
        let telemetry_store = TelemetryStore::open_with_store(blob_store.clone())?;

        // Create vector store that uses the same blob store
        let vector_store = VectorStore::open_with_store(blob_store.clone())?;

        let mut store = VectorTelemetryStore {
            blob_store,
            telemetry_store,
            vector_store,
            embedding_cache: Arc::new(RwLock::new(HashMap::new())),
            time_embeddings: Arc::new(RwLock::new(HashMap::new())),
        };

        store.build_time_index()?;
        Ok(store)
    }

    /// Store telemetry with vector embedding
    pub fn store_with_vector(
        &mut self,
        record: TelemetryRecord,
        generate_embedding: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store the telemetry record
        self.telemetry_store.store(record.clone())?;

        if generate_embedding {
            // Generate vector embedding from the value
            let text_for_embedding = match &record.value {
                TelemetryValue::String(s) => s.clone(),
                TelemetryValue::Float(f) => format!("{}", f),
                TelemetryValue::Int(i) => format!("{}", i),
                TelemetryValue::Json(j) => j.to_string(),
                _ => format!("{:?}", record.value),
            };

            if !text_for_embedding.is_empty() {
                // Store in vector store for similarity search
                let vector_key = format!("vector:{}", record.id);
                self.vector_store.insert_text(
                    &vector_key,
                    &text_for_embedding,
                    Some("vector_telemetry"),
                )?;

                // Cache the embedding
                if let Ok(embedding) = self.vector_store.embed(&text_for_embedding) {
                    self.embedding_cache
                        .write()
                        .insert(record.id.clone(), embedding);
                }

                // Index by time bucket
                let timestamp_seconds = record.timestamp().timestamp();
                let bucket_key = timestamp_seconds / 3600; // Hourly buckets
                self.time_embeddings
                    .write()
                    .entry(bucket_key)
                    .or_insert_with(Vec::new)
                    .push(record.id.clone());
            }
        }

        Ok(())
    }

    /// Search combining time range and vector similarity
    pub fn search_vector_time(
        &self,
        query: &VectorTimeQuery,
    ) -> Result<Vec<VectorTimeResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut candidates = Vec::new();

        // Step 1: Get time-based candidates
        if let Some(time_interval) = &query.time_interval {
            let telemetry_query = TelemetryQuery {
                time_interval: Some(time_interval.clone()),
                keys: query.keys.clone(),
                sources: query.sources.clone(),
                limit: query.limit * 2,
                primary_only: false,
                secondary_only: false,
                primary_id: None,
                value_type: None,
                offset: 0,
                bucket_by_minute: false,
            };
            let time_results = self.telemetry_store.query(&telemetry_query)?;
            candidates.extend(time_results);
        } else {
            // No time filter, need to consider all records or use vector search
            if let Some(vector_query) = &query.vector_query {
                // Use vector search to find candidates
                let vector_results = self
                    .vector_store
                    .search_similar(vector_query, query.limit * 2)?;
                for result in vector_results {
                    if let Some(record) = self
                        .telemetry_store
                        .get_record(&result.key.replace("vector:", ""))?
                    {
                        candidates.push(record);
                    }
                }
            }
        }

        if candidates.is_empty() && query.vector_query.is_some() {
            // Fallback to pure vector search
            let vector_results = self
                .vector_store
                .search_similar(query.vector_query.as_ref().unwrap(), query.limit)?;
            for result in vector_results {
                if let Some(record) = self
                    .telemetry_store
                    .get_record(&result.key.replace("vector:", ""))?
                {
                    candidates.push(record);
                }
            }
        }

        // Step 2: Calculate combined scores
        let now = Utc::now();
        let mut results = Vec::new();

        for record in candidates {
            // Calculate time score (recency)
            let time_distance = (now - record.timestamp()).num_seconds().abs();
            let time_score = if let Some(time_interval) = &query.time_interval {
                let total_range = (time_interval.end - time_interval.start).num_seconds();
                if total_range > 0 {
                    1.0 - (time_distance as f64 / total_range as f64)
                } else {
                    0.0
                }
            } else {
                // Exponential decay based on age
                (1.0 / (1.0 + (time_distance as f64 / 3600.0))).max(0.0)
            };

            // Calculate vector similarity score
            let vector_score = if let Some(vector_query) = &query.vector_query {
                let text_for_embedding = match &record.value {
                    TelemetryValue::String(s) => s.clone(),
                    TelemetryValue::Float(f) => format!("{}", f),
                    TelemetryValue::Int(i) => format!("{}", i),
                    TelemetryValue::Json(j) => j.to_string(),
                    _ => format!("{:?}", record.value),
                };

                if let Ok(similarity) = self.calculate_similarity(&text_for_embedding, vector_query)
                {
                    similarity as f64
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let combined_score = (vector_score * query.vector_weight as f64)
                + (time_score * query.time_weight as f64);

            if combined_score >= query.min_similarity as f64 {
                results.push(VectorTimeResult {
                    record,
                    time_score,
                    vector_score,
                    combined_score,
                    time_distance_seconds: time_distance,
                    similarity: vector_score as f32,
                });
            }
        }

        // Sort by combined score
        results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        results.truncate(query.limit);

        Ok(results)
    }

    /// Find temporally close and semantically similar events
    pub fn find_similar_events(
        &self,
        event_id: &str,
        time_window_hours: i64,
        limit: usize,
    ) -> Result<Vec<VectorTimeResult>, Box<dyn std::error::Error + Send + Sync>> {
        let source_event = match self.telemetry_store.get_record(event_id)? {
            Some(record) => record,
            None => return Ok(Vec::new()),
        };

        let time_interval = TimeInterval::new(
            source_event.timestamp() - Duration::hours(time_window_hours),
            source_event.timestamp() + Duration::hours(time_window_hours),
        );

        // Get text representation of source event
        let source_text = match &source_event.value {
            TelemetryValue::String(s) => s.clone(),
            TelemetryValue::Float(f) => format!("{}", f),
            TelemetryValue::Int(i) => format!("{}", i),
            TelemetryValue::Json(j) => j.to_string(),
            _ => format!("{:?}", source_event.value),
        };

        let query = VectorTimeQuery {
            time_interval: Some(time_interval),
            vector_query: Some(source_text),
            vector_weight: 0.8,
            time_weight: 0.2,
            keys: Some(vec![source_event.key.clone()]),
            sources: Some(vec![source_event.source.clone()]),
            limit,
            min_similarity: 0.4,
        };

        let mut results = self.search_vector_time(&query)?;
        // Remove the source event itself
        results.retain(|r| r.record.id != event_id);

        Ok(results)
    }

    /// Get temporal patterns for similar vectors
    pub fn get_temporal_patterns(
        &self,
        vector_query: &str,
        hours: i64,
    ) -> Result<Vec<TemporalPattern>, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let time_interval = TimeInterval::new(now - Duration::hours(hours), now);

        let query = VectorTimeQuery {
            time_interval: Some(time_interval),
            vector_query: Some(vector_query.to_string()),
            vector_weight: 0.9,
            time_weight: 0.1,
            keys: None,
            sources: None,
            limit: 1000,
            min_similarity: 0.3,
        };

        let results = self.search_vector_time(&query)?;

        // Group by hour to find patterns
        let mut hourly_patterns: HashMap<i64, Vec<f64>> = HashMap::new();

        for result in results {
            let hour_bucket = result.record.timestamp().timestamp() / 3600;
            hourly_patterns
                .entry(hour_bucket)
                .or_insert_with(Vec::new)
                .push(result.similarity as f64);
        }

        let mut patterns = Vec::new();
        for (hour, similarities) in hourly_patterns {
            let avg_similarity = similarities.iter().sum::<f64>() / similarities.len() as f64;
            patterns.push(TemporalPattern {
                hour_timestamp: hour * 3600,
                count: similarities.len(),
                avg_similarity,
                max_similarity: similarities.iter().fold(0.0, |a, &b| a.max(b)),
                min_similarity: similarities.iter().fold(1.0, |a, &b| a.min(b)),
            });
        }

        patterns.sort_by(|a, b| a.hour_timestamp.cmp(&b.hour_timestamp));
        Ok(patterns)
    }

    /// Calculate similarity between two texts
    fn calculate_similarity(
        &self,
        text1: &str,
        text2: &str,
    ) -> Result<f32, Box<dyn std::error::Error + Send + Sync>> {
        let emb1 = self.vector_store.embed(text1)?;
        let emb2 = self.vector_store.embed(text2)?;
        Ok(self.cosine_similarity(&emb1, &emb2))
    }

    fn cosine_similarity(&self, a: &Array1<f32>, b: &Array1<f32>) -> f32 {
        let dot = a.dot(b);
        let norm_a = a.dot(a).sqrt();
        let norm_b = b.dot(b).sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            (dot / (norm_a * norm_b)).max(0.0).min(1.0)
        }
    }

    fn build_time_index(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Rebuild time-embedding index from existing data
        let telemetry_query = TelemetryQuery {
            time_interval: None,
            keys: None,
            sources: None,
            primary_only: false,
            secondary_only: false,
            primary_id: None,
            value_type: None,
            limit: 10000,
            offset: 0,
            bucket_by_minute: false,
        };
        let all_records = self.telemetry_store.query(&telemetry_query)?;
        for record in all_records {
            let timestamp_seconds = record.timestamp().timestamp();
            let bucket_key = timestamp_seconds / 3600;
            self.time_embeddings
                .write()
                .entry(bucket_key)
                .or_insert_with(Vec::new)
                .push(record.id.clone());
        }
        Ok(())
    }
}

/// Temporal pattern result
#[derive(Debug, Clone)]
pub struct TemporalPattern {
    pub hour_timestamp: i64,
    pub count: usize,
    pub avg_similarity: f64,
    pub max_similarity: f64,
    pub min_similarity: f64,
}
