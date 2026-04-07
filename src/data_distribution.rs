use chrono::{DateTime, Datelike, Timelike, Utc};
use parking_lot::RwLock;
use regex::Regex;
use rust_stemmers::{Algorithm, Stemmer};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

use crate::blobstore::{BlobMetadata, BlobStore};
use crate::search::{FuzzySearchResult, SearchResult, SearchableBlobStore};
use crate::timeline::{TelemetryQuery, TelemetryRecord, TelemetryValue, TimeInterval};
use crate::vector::{VectorSearchResult, VectorStore};
use crate::vector_timeline::{VectorTimeQuery, VectorTimeResult};

#[derive(Debug, Clone)]
pub enum CacheType {
    TimeBucket,
    KeyCluster,
    All,
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub time_bucket_cache_size: usize,
    pub key_cluster_cache_size: usize,
    pub total_cache_size: usize,
}

#[derive(Debug, Clone)]
pub struct ShardHealth {
    pub shard_name: String,
    pub is_healthy: bool,
    pub key_count: usize,
    pub last_sync: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SystemStats {
    pub shard_health: Vec<ShardHealth>,
    pub cache_stats: CacheStats,
    pub total_records: usize,
    pub shard_count: usize,
    pub distribution_entropy: f64,
    pub load_balance_score: f64,
}

/// Distribution strategy types
#[derive(Debug, Clone)]
pub enum DistributionStrategy {
    RoundRobin,
    TimeBucket(TimeBucketConfig),
    KeySimilarity(SimilarityConfig),
    Adaptive(AdaptiveConfig),
}

/// Time bucket configuration
#[derive(Debug, Clone)]
pub struct TimeBucketConfig {
    pub bucket_size: TimeBucketSize,
    pub timezone_offset: i32,
    pub align_to_bucket: bool,
}

impl Default for TimeBucketConfig {
    fn default() -> Self {
        TimeBucketConfig {
            bucket_size: TimeBucketSize::Hours(1),
            timezone_offset: 0,
            align_to_bucket: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeBucketSize {
    Minutes(u32),
    Hours(u32),
    Days(u32),
    Weeks(u32),
    Months(u32),
}

/// Key similarity configuration
#[derive(Debug, Clone)]
pub struct SimilarityConfig {
    pub use_prefix: bool,
    pub use_suffix: bool,
    pub ngram_size: usize,
    pub min_similarity: f64,
    pub max_cluster_size: usize,
}

impl Default for SimilarityConfig {
    fn default() -> Self {
        SimilarityConfig {
            use_prefix: true,
            use_suffix: true,
            ngram_size: 3,
            min_similarity: 0.6,
            max_cluster_size: 100,
        }
    }
}

/// Adaptive distribution configuration
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    pub load_balancing_interval: Duration,
    pub rebalance_threshold: f64,
    pub min_shard_load: f64,
    pub max_shard_load: f64,
    pub history_size: usize,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        AdaptiveConfig {
            load_balancing_interval: Duration::from_secs(300),
            rebalance_threshold: 0.2,
            min_shard_load: 0.3,
            max_shard_load: 0.7,
            history_size: 1000,
        }
    }
}

/// Distribution statistics
#[derive(Debug, Clone)]
pub struct DistributionStats {
    pub total_records: usize,
    pub shard_distribution: HashMap<String, usize>,
    pub distribution_entropy: f64,
    pub load_balance_score: f64,
    pub time_bucket_distribution: HashMap<String, usize>,
    pub similarity_clusters: Vec<SimilarityCluster>,
}

/// Similarity cluster information
#[derive(Debug, Clone)]
pub struct SimilarityCluster {
    pub cluster_id: String,
    pub keys: Vec<String>,
    pub shard: String,
    pub size: usize,
    pub similarity_score: f64,
}

/// Shard information
#[derive(Debug, Clone)]
pub struct ShardInfo {
    pub name: String,
    pub path: PathBuf,
    pub key_count: usize,
}

/// Bucket statistics for minute-grade aggregation
#[derive(Debug, Clone)]
pub struct BucketStats {
    pub bucket: String,
    pub count: usize,
    pub avg_value: f64,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub sum_value: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct ChunkingConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub min_chunk_size: usize,
}

impl Default for ChunkingConfig {
    fn default() -> Self {
        ChunkingConfig {
            chunk_size: 512,
            chunk_overlap: 50,
            min_chunk_size: 100,
        }
    }
}

// Add ChunkedDocument struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkedDocument {
    pub id: String,
    pub original_text: String,
    pub chunks: Vec<TextChunk>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextChunk {
    pub chunk_id: String,
    pub text: String,
    pub shard: String,
    pub start_pos: usize,
    pub end_pos: usize,
    pub vector_key: String,
}

// Search result with chunk info
#[derive(Debug, Clone)]
pub struct ChunkSearchResult {
    pub document_id: String,
    pub chunk_id: String,
    pub text: String,
    pub score: f32,
    pub vector_score: f32,
    pub keyword_score: f32,
    pub combined_score: f32,
    pub metadata: HashMap<String, String>,
}

/// Data distribution manager
pub struct DataDistributionManager {
    shards: Arc<RwLock<Vec<ShardInfo>>>,
    stores: Arc<RwLock<HashMap<String, BlobStore>>>,
    vector_stores: Arc<RwLock<HashMap<String, VectorStore>>>,
    search_stores: Arc<RwLock<HashMap<String, SearchableBlobStore>>>,
    strategy: Arc<RwLock<DistributionStrategy>>,
    round_robin_counter: Arc<AtomicUsize>,
    time_bucket_cache: Arc<RwLock<HashMap<String, String>>>,
    key_clusters: Arc<RwLock<HashMap<String, SimilarityCluster>>>,
    load_history: Arc<RwLock<VecDeque<HashMap<String, usize>>>>,
    adaptive_config: AdaptiveConfig,
    chunk_config: Arc<RwLock<ChunkingConfig>>,
}

impl Clone for DataDistributionManager {
    fn clone(&self) -> Self {
        DataDistributionManager {
            shards: self.shards.clone(),
            stores: self.stores.clone(),
            vector_stores: self.vector_stores.clone(),
            search_stores: self.search_stores.clone(), // Add this line
            strategy: self.strategy.clone(),
            round_robin_counter: self.round_robin_counter.clone(),
            time_bucket_cache: self.time_bucket_cache.clone(),
            key_clusters: self.key_clusters.clone(),
            load_history: self.load_history.clone(),
            adaptive_config: self.adaptive_config.clone(),
            chunk_config: self.chunk_config.clone(), // Add this line
        }
    }
}

// Advanced chunking configuration
#[derive(Debug, Clone)]
pub struct AdvancedChunkingConfig {
    pub chunk_size: usize,           // Target chunk size in characters
    pub chunk_overlap: usize,        // Overlap between chunks
    pub min_chunk_size: usize,       // Minimum chunk size
    pub break_on_sentences: bool,    // Prefer breaking at sentence boundaries
    pub break_on_paragraphs: bool,   // Prefer breaking at paragraph boundaries
    pub preserve_metadata: bool,     // Preserve document metadata in chunks
    pub context_before_chars: usize, // Characters to include before chunk
    pub context_after_chars: usize,  // Characters to include after chunk
    pub enable_stemming: bool,       // Enable snowball stemming
    pub language: StemmingLanguage,  // Language for stemming
}

impl Default for AdvancedChunkingConfig {
    fn default() -> Self {
        AdvancedChunkingConfig {
            chunk_size: 512,
            chunk_overlap: 50,
            min_chunk_size: 100,
            break_on_sentences: true,
            break_on_paragraphs: true,
            preserve_metadata: true,
            context_before_chars: 100,
            context_after_chars: 100,
            enable_stemming: false,
            language: StemmingLanguage::English,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StemmingLanguage {
    English,
    Russian,
    German,
    French,
    Spanish,
    Italian,
    Dutch,
    Portuguese,
}

impl StemmingLanguage {
    #[allow(dead_code)]
    pub fn get_name(&self) -> &'static str {
        match self {
            StemmingLanguage::English => "english",
            StemmingLanguage::Russian => "russian",
            StemmingLanguage::German => "german",
            StemmingLanguage::French => "french",
            StemmingLanguage::Spanish => "spanish",
            StemmingLanguage::Italian => "italian",
            StemmingLanguage::Dutch => "dutch",
            StemmingLanguage::Portuguese => "portuguese",
        }
    }
}

// Enhanced chunk with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedTextChunk {
    pub chunk_id: String,
    pub text: String,
    pub context_before: String,       // Text before the chunk
    pub context_after: String,        // Text after the chunk
    pub stemmed_text: Option<String>, // Stemmed version for search
    pub shard: String,
    pub start_pos: usize,
    pub end_pos: usize,
    pub start_sentence: usize,
    pub end_sentence: usize,
    pub paragraph_index: usize,
    pub vector_key: String,
    pub metadata: HashMap<String, String>,
}

// Enhanced chunked document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnhancedChunkedDocument {
    pub id: String,
    pub original_text: String,
    pub chunks: Vec<EnhancedTextChunk>,
    pub metadata: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub word_count: usize,
    pub sentence_count: usize,
    pub paragraph_count: usize,
}

// Search result with enhanced context
#[derive(Debug, Clone)]
pub struct EnhancedChunkSearchResult {
    pub document_id: String,
    pub chunk_id: String,
    pub text: String,
    pub context_before: String,
    pub context_after: String,
    pub score: f32,
    pub vector_score: f32,
    pub keyword_score: f32,
    pub combined_score: f32,
    pub metadata: HashMap<String, String>,
    pub relevance_context: String, // Full context for RAG
}

impl DataDistributionManager {
    /// Create a new data distribution manager
    pub fn new<P: AsRef<Path>>(
        base_path: P,
        strategy: DistributionStrategy,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let base_path = base_path.as_ref();
        std::fs::create_dir_all(base_path)?;

        let mut shards = Vec::new();
        let mut stores = HashMap::new();
        let mut vector_stores = HashMap::new();
        let mut search_stores = HashMap::new();

        for i in 0..4 {
            let shard_name = format!("shard_{}", i);
            let shard_path = base_path.join(&shard_name);
            std::fs::create_dir_all(&shard_path)?;
            let store = BlobStore::open(shard_path.join("data.redb"))?;
            let vector_store = VectorStore::open(shard_path.join("vectors.redb"))?;
            let search_store = SearchableBlobStore::open(shard_path.join("search.redb"))?;

            shards.push(ShardInfo {
                name: shard_name.clone(),
                path: shard_path,
                key_count: 0,
            });
            stores.insert(shard_name.clone(), store);
            vector_stores.insert(shard_name.clone(), vector_store);
            search_stores.insert(shard_name.clone(), search_store);
        }

        Ok(DataDistributionManager {
            shards: Arc::new(RwLock::new(shards)),
            stores: Arc::new(RwLock::new(stores)),
            vector_stores: Arc::new(RwLock::new(vector_stores)),
            search_stores: Arc::new(RwLock::new(search_stores)),
            strategy: Arc::new(RwLock::new(strategy)),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
            time_bucket_cache: Arc::new(RwLock::new(HashMap::new())),
            key_clusters: Arc::new(RwLock::new(HashMap::new())),
            load_history: Arc::new(RwLock::new(VecDeque::new())),
            adaptive_config: AdaptiveConfig::default(),
            chunk_config: Arc::new(RwLock::new(ChunkingConfig::default())),
        })
    }

    // Update with_shards method
    pub fn with_shards<P: AsRef<Path>>(
        base_path: P,
        strategy: DistributionStrategy,
        num_shards: usize,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let base_path = base_path.as_ref();
        std::fs::create_dir_all(base_path)?;

        let mut shards = Vec::new();
        let mut stores = HashMap::new();
        let mut vector_stores = HashMap::new();
        let mut search_stores = HashMap::new();

        for i in 0..num_shards {
            let shard_name = format!("shard_{}", i);
            let shard_path = base_path.join(&shard_name);
            std::fs::create_dir_all(&shard_path)?;
            let store = BlobStore::open(shard_path.join("data.redb"))?;
            let vector_store = VectorStore::open(shard_path.join("vectors.redb"))?;
            let search_store = SearchableBlobStore::open(shard_path.join("search.redb"))?;

            shards.push(ShardInfo {
                name: shard_name.clone(),
                path: shard_path,
                key_count: 0,
            });
            stores.insert(shard_name.clone(), store);
            vector_stores.insert(shard_name.clone(), vector_store);
            search_stores.insert(shard_name.clone(), search_store);
        }

        Ok(DataDistributionManager {
            shards: Arc::new(RwLock::new(shards)),
            stores: Arc::new(RwLock::new(stores)),
            vector_stores: Arc::new(RwLock::new(vector_stores)),
            search_stores: Arc::new(RwLock::new(search_stores)),
            strategy: Arc::new(RwLock::new(strategy)),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
            time_bucket_cache: Arc::new(RwLock::new(HashMap::new())),
            key_clusters: Arc::new(RwLock::new(HashMap::new())),
            load_history: Arc::new(RwLock::new(VecDeque::new())),
            adaptive_config: AdaptiveConfig::default(),
            chunk_config: Arc::new(RwLock::new(ChunkingConfig::default())),
        })
    }

    /// Store data with automatic distribution
    pub fn put(
        &self,
        key: &str,
        data: &[u8],
        _timestamp: Option<DateTime<Utc>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get the shard name first (this will increment the counter)
        let shard_name = self.get_target_shard(key, _timestamp)?;

        log::debug!("Putting key '{}' into shard '{}'", key, shard_name); // Debug output

        // Get a clone of the store
        let mut store = {
            let stores = self.stores.read();
            stores.get(&shard_name).ok_or("Shard not found")?.clone()
        };

        store.put(key, data, None)?;

        // Update key count
        {
            let mut shards = self.shards.write();
            if let Some(shard) = shards.iter_mut().find(|s| s.name == shard_name) {
                shard.key_count += 1;
            }
        }

        self.update_load_history();
        Ok(())
    }

    /// Store telemetry record
    pub fn put_telemetry(
        &self,
        record: TelemetryRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timestamp = Some(record.timestamp());
        let shard_name = self.get_target_shard(&record.key, timestamp)?;
        let mut stores = self.stores.write();
        let store = stores.get_mut(&shard_name).ok_or("Shard not found")?;

        let telemetry_key = format!("telemetry:{}:{}", record.timestamp().timestamp(), record.id);
        store.put(
            &telemetry_key,
            &serde_json::to_vec(&record)?,
            Some("telemetry"),
        )?;

        self.update_load_history();
        Ok(())
    }

    /// Store telemetry with primary-secondary relationship support
    pub fn put_telemetry_with_relation(
        &self,
        record: TelemetryRecord,
        primary_id: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timestamp = Some(record.timestamp());
        let shard_name = self.get_target_shard(&record.key, timestamp)?;
        let mut stores = self.stores.write();
        let store = stores.get_mut(&shard_name).ok_or("Shard not found")?;

        let telemetry_key = if let Some(primary) = primary_id {
            format!("telemetry:secondary:{}:{}", primary, record.id)
        } else {
            format!("telemetry:primary:{}", record.id)
        };

        store.put(
            &telemetry_key,
            &serde_json::to_vec(&record)?,
            Some("telemetry"),
        )?;

        if let Some(primary) = primary_id {
            let relation_key = format!("telemetry:relation:{}:{}", primary, record.id);
            store.put(&relation_key, b"linked", Some("telemetry"))?;
        }

        self.update_load_history();
        Ok(())
    }

    /// Get secondary records for a primary telemetry record
    pub fn get_secondaries(
        &self,
        primary_id: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let mut secondaries = Vec::new();

        for store in stores.values() {
            let all_keys = store.list_keys()?;
            let prefix = format!("telemetry:secondary:{}:", primary_id);
            for key in all_keys {
                if key.starts_with(&prefix) {
                    if let Some(data) = store.get(&key)? {
                        if let Ok(record) = serde_json::from_slice::<TelemetryRecord>(&data) {
                            secondaries.push(record);
                        }
                    }
                }
            }
        }

        Ok(secondaries)
    }

    /// Get primary record for a secondary telemetry record
    pub fn get_primary(
        &self,
        secondary_id: &str,
    ) -> Result<Option<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();

        for store in stores.values() {
            let all_keys = store.list_keys()?;
            for key in all_keys {
                if key.contains(secondary_id) && key.starts_with("telemetry:primary:") {
                    if let Some(data) = store.get(&key)? {
                        if let Ok(record) = serde_json::from_slice::<TelemetryRecord>(&data) {
                            return Ok(Some(record));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Retrieve data
    pub fn get(
        &self,
        key: &str,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        for store in stores.values() {
            if let Some(data) = store.get(key)? {
                return Ok(Some(data));
            }
        }
        Ok(None) // Return None, not ()
    }

    /// Get with metadata
    pub fn get_with_metadata(
        &self,
        key: &str,
    ) -> Result<Option<(Vec<u8>, BlobMetadata)>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        for store in stores.values() {
            if let Some(data) = store.get(key)? {
                if let Ok(Some(metadata)) = store.get_metadata(key) {
                    return Ok(Some((data, metadata)));
                }
            }
        }
        Ok(None)
    }

    /// Delete data
    pub fn delete(&self, key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut stores = self.stores.write();
        for store in stores.values_mut() {
            if store.exists(key)? {
                return Ok(store.remove(key)?);
            }
        }
        Ok(false)
    }

    /// Check if key exists
    pub fn exists(&self, key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        for store in stores.values() {
            if store.exists(key)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// List keys matching pattern
    pub fn list_keys(
        &self,
        pattern: Option<&str>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let mut all_keys = Vec::new();

        for store in stores.values() {
            let keys = store.list_keys()?;
            if let Some(pattern) = pattern {
                let matched: Vec<String> = keys
                    .into_iter()
                    .filter(|k| {
                        k.contains(pattern) && !k.starts_with("__") && !k.starts_with("telemetry:")
                    })
                    .collect();
                all_keys.extend(matched);
            } else {
                let filtered: Vec<String> = keys
                    .into_iter()
                    .filter(|k| !k.starts_with("__") && !k.starts_with("telemetry:"))
                    .collect();
                all_keys.extend(filtered);
            }
        }

        all_keys.sort();
        all_keys.dedup();
        Ok(all_keys)
    }

    /// Query telemetry across shards
    pub fn query_telemetry(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let mut all_records = Vec::new();

        for store in stores.values() {
            let all_keys = store.list_keys()?;
            for key in all_keys {
                if key.starts_with("telemetry:") && !key.contains(":relation:") {
                    if let Some(data) = store.get(&key)? {
                        if let Ok(record) = serde_json::from_slice::<TelemetryRecord>(&data) {
                            // Apply time filter
                            if let Some(time_interval) = &query.time_interval {
                                let ts = record.timestamp();
                                if ts < time_interval.start || ts > time_interval.end {
                                    continue;
                                }
                            }

                            // Apply keys filter
                            if let Some(ref key_filters) = query.keys {
                                if !key_filters.iter().any(|k| record.key.contains(k)) {
                                    continue;
                                }
                            }

                            // Apply sources filter
                            if let Some(ref source_filters) = query.sources {
                                if !source_filters.iter().any(|s| record.source.contains(s)) {
                                    continue;
                                }
                            }

                            all_records.push(record);
                        }
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        all_records.sort_by(|a, b| b.timestamp().cmp(&a.timestamp()));

        // Apply limit and offset
        let start = query.offset.min(all_records.len());
        let end = (start + query.limit).min(all_records.len());

        Ok(all_records[start..end].to_vec())
    }

    /// Query telemetry with advanced filters
    pub fn query_telemetry_advanced(
        &self,
        time_interval: Option<TimeInterval>,
        keys: Option<Vec<String>>,
        sources: Option<Vec<String>>,
        value_type: Option<String>,
        limit: usize,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let query = TelemetryQuery {
            time_interval,
            keys,
            sources,
            primary_only: false,
            secondary_only: false,
            primary_id: None,
            value_type,
            limit,
            offset: 0,
            bucket_by_minute: false,
        };
        self.query_telemetry(&query)
    }

    /// Search across shards
    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let mut results = Vec::new();

        for store in stores.values() {
            let all_keys = store.list_keys()?;
            for key in all_keys {
                if !key.starts_with("__") && !key.starts_with("telemetry:") {
                    if let Some(data) = store.get(&key)? {
                        if let Ok(text) = String::from_utf8(data) {
                            if text.to_lowercase().contains(&query.to_lowercase()) {
                                results.push(SearchResult {
                                    key,
                                    score: 1.0,
                                    matches: vec![query.to_string()],
                                    metadata: None,
                                });
                            }
                        }
                    }
                }
            }
        }

        results.truncate(limit);
        Ok(results)
    }
    /// Fuzzy search across shards
    pub fn fuzzy_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FuzzySearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        for store in stores.values() {
            let all_keys = store.list_keys()?;
            for key in all_keys {
                if !key.starts_with("__") && !key.starts_with("telemetry:") {
                    if let Some(data) = store.get(&key)? {
                        if let Ok(text) = String::from_utf8(data) {
                            let text_lower = text.to_lowercase();
                            // Simple fuzzy matching - check if query appears in text
                            if text_lower.contains(&query_lower) {
                                results.push(FuzzySearchResult {
                                    key,
                                    term: query.to_string(),
                                    distance: 0,
                                    score: 1.0,
                                    metadata: None,
                                });
                            } else {
                                // Try to find similar words
                                for word in text_lower.split_whitespace() {
                                    let distance = levenshtein_distance(&query_lower, word);
                                    if distance <= 2 {
                                        results.push(FuzzySearchResult {
                                            key: key.clone(),
                                            term: word.to_string(),
                                            distance,
                                            score: 1.0 - (distance as f64 / 10.0),
                                            metadata: None,
                                        });
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Deduplicate by key, keeping the highest score
        let mut unique_results: std::collections::HashMap<String, FuzzySearchResult> =
            std::collections::HashMap::new();
        for result in results {
            let entry = unique_results
                .entry(result.key.clone())
                .or_insert(result.clone());
            if result.score > entry.score {
                *entry = result;
            }
        }

        let mut final_results: Vec<FuzzySearchResult> = unique_results.into_values().collect();
        final_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        final_results.truncate(limit);

        Ok(final_results)
    }

    /// Search by key pattern
    pub fn search_by_key(
        &self,
        pattern: &str,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let mut matches = Vec::new();

        for (_name, store) in stores.iter() {
            let keys = store.list_keys()?;
            for key in keys {
                if key.contains(pattern) && !key.starts_with("__") && !key.starts_with("telemetry:")
                {
                    matches.push(key);
                }
            }
        }

        matches.sort();
        matches.dedup();
        Ok(matches)
    }

    /// Search by source
    pub fn search_by_source(
        &self,
        source: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.query_telemetry_advanced(None, None, Some(vec![source.to_string()]), None, 1000)
    }

    /// Get minute-grade bucketed telemetry data
    pub fn get_minute_bucketed(
        &self,
        time_interval: TimeInterval,
        key_filter: Option<&str>,
    ) -> Result<HashMap<String, Vec<TelemetryRecord>>, Box<dyn std::error::Error + Send + Sync>>
    {
        let records = self.query_telemetry_advanced(
            Some(time_interval),
            key_filter.map(|k| vec![k.to_string()]),
            None,
            None,
            10000,
        )?;

        let mut buckets: HashMap<String, Vec<TelemetryRecord>> = HashMap::new();

        for record in records {
            let ts = record.timestamp();
            let bucket_key = format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                ts.year(),
                ts.month(),
                ts.day(),
                ts.hour(),
                ts.minute()
            );
            buckets
                .entry(bucket_key)
                .or_insert_with(Vec::new)
                .push(record);
        }

        Ok(buckets)
    }

    /// Get aggregated statistics for bucketed data
    pub fn get_bucket_stats(
        &self,
        time_interval: TimeInterval,
        key_filter: Option<&str>,
    ) -> Result<Vec<BucketStats>, Box<dyn std::error::Error + Send + Sync>> {
        let buckets = self.get_minute_bucketed(time_interval, key_filter)?;
        let mut stats = Vec::new();

        for (bucket, records) in buckets {
            let numeric_values: Vec<f64> =
                records.iter().filter_map(|r| r.value.as_float()).collect();

            let avg = if !numeric_values.is_empty() {
                numeric_values.iter().sum::<f64>() / numeric_values.len() as f64
            } else {
                0.0
            };

            let min = numeric_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
            let max = numeric_values
                .iter()
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let sum: f64 = numeric_values.iter().sum();

            stats.push(BucketStats {
                bucket,
                count: records.len(),
                avg_value: avg,
                min_value: if min.is_finite() { Some(min) } else { None },
                max_value: if max.is_finite() { Some(max) } else { None },
                sum_value: if sum > 0.0 { Some(sum) } else { None },
            });
        }

        stats.sort_by(|a, b| a.bucket.cmp(&b.bucket));
        Ok(stats)
    }

    /// Get target shard for a key
    fn get_target_shard(
        &self,
        key: &str,
        timestamp: Option<DateTime<Utc>>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let strategy = {
            let strategy_guard = self.strategy.read();
            strategy_guard.clone()
        };

        let shards = {
            let shards_guard = self.shards.read();
            shards_guard.clone()
        };

        if shards.is_empty() {
            return Err("No shards available".into());
        }

        let idx = match strategy {
            DistributionStrategy::RoundRobin => {
                // Increment counter and get previous value
                let counter = self.round_robin_counter.fetch_add(1, Ordering::SeqCst);
                let idx = counter % shards.len();
                log::debug!(
                    "Round-robin: counter={}, idx={}, shards={}",
                    counter,
                    idx,
                    shards.len()
                );
                idx
            }
            DistributionStrategy::TimeBucket(config) => {
                self.get_time_bucket_index(key, timestamp, &config, &shards)
            }
            DistributionStrategy::KeySimilarity(config) => {
                self.get_key_similarity_index(key, &config, &shards)
            }
            DistributionStrategy::Adaptive(config) => {
                self.get_adaptive_index(key, timestamp, &config, &shards)
            }
        };

        Ok(shards[idx].name.clone())
    }

    fn get_time_bucket_index(
        &self,
        _key: &str,
        timestamp: Option<DateTime<Utc>>,
        config: &TimeBucketConfig,
        shards: &[ShardInfo],
    ) -> usize {
        let ts = timestamp.unwrap_or_else(Utc::now);
        let bucket_key = self.get_time_bucket_key(ts, config);

        // Check cache
        if let Some(shard_name) = self.time_bucket_cache.read().get(&bucket_key) {
            if let Some(idx) = shards.iter().position(|s| s.name == *shard_name) {
                return idx;
            }
        }

        let hash = self.hash_string(&bucket_key);
        let idx = hash % shards.len();
        let shard_name = shards[idx].name.clone();
        self.time_bucket_cache
            .write()
            .insert(bucket_key, shard_name);

        idx
    }

    fn get_key_similarity_index(
        &self,
        key: &str,
        config: &SimilarityConfig,
        shards: &[ShardInfo],
    ) -> usize {
        // Check existing clusters
        let clusters = self.key_clusters.read();
        for (existing_key, cluster) in clusters.iter() {
            let similarity = self.calculate_key_similarity(key, existing_key, config);
            if similarity >= config.min_similarity && cluster.size < config.max_cluster_size {
                if let Some(idx) = shards.iter().position(|s| s.name == cluster.shard) {
                    return idx;
                }
            }
        }

        // Create new cluster
        let hash = self.hash_string(key);
        let idx = hash % shards.len();
        let shard_name = shards[idx].name.clone();

        let cluster = SimilarityCluster {
            cluster_id: format!("cluster_{}", hash),
            keys: vec![key.to_string()],
            shard: shard_name,
            size: 1,
            similarity_score: 1.0,
        };

        drop(clusters);
        self.key_clusters.write().insert(key.to_string(), cluster);

        idx
    }

    fn get_adaptive_index(
        &self,
        _key: &str,
        _timestamp: Option<DateTime<Utc>>,
        _config: &AdaptiveConfig,
        shards: &[ShardInfo],
    ) -> usize {
        // Find least loaded shard
        let mut min_count = usize::MAX;
        let mut min_idx = 0;
        for (i, shard) in shards.iter().enumerate() {
            if shard.key_count < min_count {
                min_count = shard.key_count;
                min_idx = i;
            }
        }
        min_idx
    }

    fn calculate_key_similarity(&self, key1: &str, key2: &str, config: &SimilarityConfig) -> f64 {
        let mut similarity = 0.0;
        let mut components = 0;

        if config.use_prefix {
            let prefix_sim = self.prefix_similarity(key1, key2);
            similarity += prefix_sim;
            components += 1;
        }

        if config.use_suffix {
            let suffix_sim = self.suffix_similarity(key1, key2);
            similarity += suffix_sim;
            components += 1;
        }

        let ngram_sim = self.ngram_similarity(key1, key2, config.ngram_size);
        similarity += ngram_sim;
        components += 1;

        if components > 0 {
            similarity / components as f64
        } else {
            0.0
        }
    }

    fn prefix_similarity(&self, key1: &str, key2: &str) -> f64 {
        let min_len = key1.len().min(key2.len());
        let common_prefix = key1
            .chars()
            .zip(key2.chars())
            .take_while(|(a, b)| a == b)
            .count();

        if min_len > 0 {
            common_prefix as f64 / min_len as f64
        } else {
            0.0
        }
    }

    fn suffix_similarity(&self, key1: &str, key2: &str) -> f64 {
        let rev1: String = key1.chars().rev().collect();
        let rev2: String = key2.chars().rev().collect();
        self.prefix_similarity(&rev1, &rev2)
    }

    fn ngram_similarity(&self, key1: &str, key2: &str, n: usize) -> f64 {
        let ngrams1: HashSet<String> = (0..=key1.len().saturating_sub(n))
            .map(|i| key1[i..i + n].to_string())
            .collect();

        let ngrams2: HashSet<String> = (0..=key2.len().saturating_sub(n))
            .map(|i| key2[i..i + n].to_string())
            .collect();

        let intersection: HashSet<_> = ngrams1.intersection(&ngrams2).collect();

        if ngrams1.is_empty() && ngrams2.is_empty() {
            1.0
        } else if ngrams1.is_empty() || ngrams2.is_empty() {
            0.0
        } else {
            2.0 * intersection.len() as f64 / (ngrams1.len() + ngrams2.len()) as f64
        }
    }

    fn get_time_bucket_key(&self, timestamp: DateTime<Utc>, config: &TimeBucketConfig) -> String {
        let adjusted = timestamp + chrono::Duration::hours(config.timezone_offset as i64);

        match config.bucket_size {
            TimeBucketSize::Minutes(minutes) => {
                let minute_bucket = adjusted.minute() / minutes;
                format!(
                    "{}-{}-{}_{}:{}",
                    adjusted.year(),
                    adjusted.month(),
                    adjusted.day(),
                    adjusted.hour(),
                    minute_bucket * minutes
                )
            }
            TimeBucketSize::Hours(hours) => {
                let hour_bucket = adjusted.hour() / hours;
                format!(
                    "{}-{}-{}_{}",
                    adjusted.year(),
                    adjusted.month(),
                    adjusted.day(),
                    hour_bucket * hours
                )
            }
            TimeBucketSize::Days(days) => {
                let day_bucket = adjusted.day() / days;
                format!(
                    "{}-{}-{}",
                    adjusted.year(),
                    adjusted.month(),
                    day_bucket * days
                )
            }
            TimeBucketSize::Weeks(weeks) => {
                let week_bucket = adjusted.iso_week().week() / weeks;
                format!("{}-W{}", adjusted.year(), week_bucket)
            }
            TimeBucketSize::Months(months) => {
                let month_bucket = (adjusted.month() - 1) / months;
                format!("{}-M{}", adjusted.year(), month_bucket + 1)
            }
        }
    }

    fn update_load_history(&self) {
        let shards = self.shards.read();
        let loads: HashMap<String, usize> = shards
            .iter()
            .map(|s| (s.name.clone(), s.key_count))
            .collect();

        let mut history = self.load_history.write();
        history.push_back(loads);

        while history.len() > self.adaptive_config.history_size {
            history.pop_front();
        }
    }

    fn hash_string(&self, s: &str) -> usize {
        s.bytes().fold(0usize, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as usize)
        })
    }

    // ========== Shard Management APIs ==========

    /// Add a new shard
    pub fn add_shard(
        &self,
        name: &str,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let shard_path = PathBuf::from(path);
        std::fs::create_dir_all(&shard_path)?;
        let store = BlobStore::open(shard_path.join("data.redb"))?;

        let mut shards = self.shards.write();
        let mut stores = self.stores.write();

        // Check if shard already exists
        if shards.iter().any(|s| s.name == name) {
            return Err(format!("Shard '{}' already exists", name).into());
        }

        shards.push(ShardInfo {
            name: name.to_string(),
            path: shard_path,
            key_count: 0,
        });
        stores.insert(name.to_string(), store);

        self.clear_caches();

        Ok(())
    }

    /// Add a key-range based shard
    pub fn add_key_range_shard(
        &self,
        name: &str,
        path: &str,
        _start_key: &str,
        _end_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.add_shard(name, path)
    }

    /// Add a time-range based shard
    pub fn add_time_range_shard(
        &self,
        name: &str,
        path: &str,
        _start_time: DateTime<Utc>,
        _end_time: DateTime<Utc>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.add_shard(name, path)
    }

    /// Remove a shard
    pub fn remove_shard(
        &self,
        name: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut shards = self.shards.write();
        let mut stores = self.stores.write();

        if let Some(index) = shards.iter().position(|s| s.name == name) {
            shards.remove(index);
            stores.remove(name);
            self.clear_caches();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get number of shards
    pub fn shard_count(&self) -> usize {
        self.shards.read().len()
    }

    /// Get all shard names
    pub fn get_all_shard_names(&self) -> Vec<String> {
        self.shards.read().iter().map(|s| s.name.clone()).collect()
    }

    /// Get shard details
    pub fn get_shard_details(&self) -> Vec<ShardInfo> {
        self.shards.read().clone()
    }

    /// Check if a shard exists
    pub fn shard_exists(&self, shard_name: &str) -> bool {
        self.shards.read().iter().any(|s| s.name == shard_name)
    }

    /// Get shard loads for adaptive distribution
    pub fn get_shard_loads(&self) -> HashMap<String, f64> {
        let shards = self.shards.read();
        let total: usize = shards.iter().map(|s| s.key_count).sum();

        shards
            .iter()
            .map(|s| {
                (
                    s.name.clone(),
                    if total > 0 {
                        s.key_count as f64 / total as f64
                    } else {
                        0.0
                    },
                )
            })
            .collect()
    }

    /// Get shard for a specific key
    pub fn get_shard_for_key(
        &self,
        key: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.get_target_shard(key, None)
    }

    /// Trigger rebalancing of data across shards
    pub fn trigger_rebalance(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let shards = self.shards.read();
        let total_keys: usize = shards.iter().map(|s| s.key_count).sum();
        if total_keys == 0 || shards.len() <= 1 {
            return Ok(());
        }

        let target_per_shard = total_keys / shards.len();

        // Find overloaded shards
        let overloaded: Vec<ShardInfo> = shards
            .iter()
            .filter(|s| s.key_count > target_per_shard * 2)
            .cloned()
            .collect();

        let underloaded: Vec<ShardInfo> = shards
            .iter()
            .filter(|s| s.key_count < target_per_shard / 2)
            .cloned()
            .collect();

        if overloaded.is_empty() || underloaded.is_empty() {
            return Ok(());
        }

        // Clear caches to force redistribution on next writes
        self.clear_caches();

        Ok(())
    }

    /// Rebalance data (public API)
    pub fn rebalance(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.trigger_rebalance()
    }

    /// Get distribution statistics
    pub fn get_distribution_stats(&self) -> DistributionStats {
        let shards = self.shards.read();
        let total_records: usize = shards.iter().map(|s| s.key_count).sum();

        let shard_distribution: HashMap<String, usize> = shards
            .iter()
            .map(|s| (s.name.clone(), s.key_count))
            .collect();

        // Calculate entropy
        let entropy = if total_records > 0 && shards.len() > 0 {
            let num_shards = shards.len() as f64;
            let ideal = total_records as f64 / num_shards;
            let variance: f64 = shard_distribution
                .values()
                .map(|&count| (count as f64 - ideal).powi(2))
                .sum::<f64>()
                / num_shards;
            1.0 - (variance.sqrt() / ideal).min(1.0)
        } else {
            1.0
        };

        // Calculate load balance score
        let load_balance_score = if total_records > 0 && shards.len() > 0 {
            let max_load = *shard_distribution.values().max().unwrap_or(&0) as f64;
            let avg_load = total_records as f64 / shards.len() as f64;
            if max_load > 0.0 {
                avg_load / max_load
            } else {
                1.0
            }
        } else {
            1.0
        };

        DistributionStats {
            total_records,
            shard_distribution,
            distribution_entropy: entropy,
            load_balance_score,
            time_bucket_distribution: HashMap::new(),
            similarity_clusters: self.key_clusters.read().values().cloned().collect(),
        }
    }

    /// Get distribution stats (alias)
    pub fn get_stats(&self) -> DistributionStats {
        self.get_distribution_stats()
    }

    /// Set distribution strategy
    pub fn set_strategy(&self, strategy: DistributionStrategy) {
        *self.strategy.write() = strategy;
        self.clear_caches();
    }

    /// Get current strategy
    pub fn get_strategy(&self) -> DistributionStrategy {
        self.strategy.read().clone()
    }

    /// Get similarity clusters
    pub fn get_similarity_clusters(&self) -> Vec<SimilarityCluster> {
        self.key_clusters.read().values().cloned().collect()
    }

    /// Get time range of stored telemetry data
    pub fn get_telemetry_time_range(
        &self,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, Box<dyn std::error::Error + Send + Sync>>
    {
        let stores = self.stores.read();
        let mut min_time: Option<DateTime<Utc>> = None;
        let mut max_time: Option<DateTime<Utc>> = None;

        for (_name, store) in stores.iter() {
            let all_keys = store.list_keys()?;
            for key in all_keys {
                if key.starts_with("telemetry:") && !key.contains(":relation:") {
                    if let Some(data) = store.get(&key)? {
                        if let Ok(record) = serde_json::from_slice::<TelemetryRecord>(&data) {
                            let ts = record.timestamp();
                            if min_time.is_none() || ts < min_time.unwrap() {
                                min_time = Some(ts);
                            }
                            if max_time.is_none() || ts > max_time.unwrap() {
                                max_time = Some(ts);
                            }
                        }
                    }
                }
            }
        }

        match (min_time, max_time) {
            (Some(min), Some(max)) => Ok(Some((min, max))),
            _ => Ok(None),
        }
    }
    /// Store text for vector search
    pub fn put_vector_text(
        &self,
        key: &str,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let shard_name = self.get_target_shard(key, None)?;
        let mut vector_stores = self.vector_stores.write();
        let vector_store = vector_stores
            .get_mut(&shard_name)
            .ok_or("Vector store not found")?;
        vector_store.insert_text(key, text, None)?;
        Ok(())
    }

    /// Vector search across all shards
    pub fn vector_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let vector_stores = self.vector_stores.read();
        let mut all_results = Vec::new();

        // Collect stores first, then search
        let stores_to_search: Vec<_> = vector_stores.values().collect();

        for vector_store in stores_to_search {
            let results = vector_store.search_similar(query, limit)?;
            all_results.extend(results);
        }

        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_results.truncate(limit);

        Ok(all_results)
    }

    /// Store telemetry with vector embedding for time-vector search
    pub fn put_telemetry_with_vector(
        &self,
        record: TelemetryRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let shard_name = self.get_target_shard(&record.key, Some(record.timestamp()))?;
        let mut stores = self.stores.write();
        let store = stores.get_mut(&shard_name).ok_or("Shard not found")?;
        let mut vector_stores = self.vector_stores.write();
        let vector_store = vector_stores
            .get_mut(&shard_name)
            .ok_or("Vector store not found")?;

        // Store telemetry record
        let telemetry_key = format!("telemetry:{}:{}", record.timestamp().timestamp(), record.id);
        store.put(
            &telemetry_key,
            &serde_json::to_vec(&record)?,
            Some("telemetry"),
        )?;

        // Generate text for embedding from the telemetry value
        let text_for_embedding = match &record.value {
            TelemetryValue::String(s) => s.clone(),
            TelemetryValue::Float(f) => format!("{}", f),
            TelemetryValue::Int(i) => format!("{}", i),
            TelemetryValue::Json(j) => j.to_string(),
            _ => format!("{:?}", record.value),
        };

        // Store vector embedding
        let vector_key = format!("vector:telemetry:{}", record.id);
        vector_store.insert_text(&vector_key, &text_for_embedding, Some("telemetry"))?;

        self.update_load_history();
        Ok(())
    }

    /// Time-vector search combining temporal and semantic similarity
    pub fn search_vector_time(
        &self,
        query: &VectorTimeQuery,
    ) -> Result<Vec<VectorTimeResult>, Box<dyn std::error::Error + Send + Sync>> {
        let vector_stores = self.vector_stores.read();
        let stores = self.stores.read();
        let mut results = Vec::new();

        // First, get vector search results
        if let Some(vector_query) = &query.vector_query {
            for (shard_name, vector_store) in vector_stores.iter() {
                let vector_results = vector_store.search_similar(vector_query, query.limit * 2)?;

                for vector_result in vector_results {
                    // Extract telemetry ID from vector key
                    let telemetry_id = vector_result.key.replace("vector:telemetry:", "");

                    // Get the actual telemetry record
                    let _telemetry_key = format!("telemetry:*:{}", telemetry_id);
                    if let Some(store) = stores.get(shard_name) {
                        let all_keys = store.list_keys()?;
                        for key in all_keys {
                            if key.contains(&telemetry_id) && key.starts_with("telemetry:") {
                                if let Some(data) = store.get(&key)? {
                                    if let Ok(record) =
                                        serde_json::from_slice::<TelemetryRecord>(&data)
                                    {
                                        // Calculate time score
                                        let time_score = if let Some(time_interval) =
                                            &query.time_interval
                                        {
                                            let ts = record.timestamp();
                                            if ts >= time_interval.start && ts <= time_interval.end
                                            {
                                                1.0
                                            } else {
                                                0.0
                                            }
                                        } else {
                                            1.0
                                        };

                                        let combined_score = (vector_result.score as f64
                                            * query.vector_weight as f64)
                                            + (time_score * query.time_weight as f64);

                                        if combined_score >= query.min_similarity as f64 {
                                            results.push(VectorTimeResult {
                                                record,
                                                time_score,
                                                vector_score: vector_result.score as f64,
                                                combined_score,
                                                time_distance_seconds: 0,
                                                similarity: vector_result.score,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by combined score
        results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        results.truncate(query.limit);

        Ok(results)
    }
    /// Get underlying shard manager (for compatibility)
    pub fn shard_manager(&self) -> Arc<Self> {
        Arc::new(self.clone())
    }

    /// Configure chunking parameters
    pub fn set_chunk_config(&self, config: ChunkingConfig) {
        *self.chunk_config.write() = config;
    }

    /// Split text into chunks
    fn split_into_chunks(&self, text: &str) -> Vec<String> {
        let config = self.chunk_config.read();
        let mut chunks = Vec::new();
        let mut start = 0;
        let text_len = text.len();

        while start < text_len {
            let end = (start + config.chunk_size).min(text_len);
            let mut chunk = text[start..end].to_string();

            // Try to find a good breaking point (space, punctuation)
            if end < text_len
                && !chunk.ends_with(' ')
                && !chunk.ends_with('.')
                && !chunk.ends_with('!')
                && !chunk.ends_with('?')
            {
                if let Some(last_space) = chunk.rfind(' ') {
                    chunk = chunk[..last_space].to_string();
                }
            }

            if chunk.len() >= config.min_chunk_size {
                chunks.push(chunk);
            }

            start += config.chunk_size - config.chunk_overlap;
        }

        chunks
    }

    /// Store a document with chunking and distributed vector storage
    pub fn store_chunked_document(
        &self,
        doc_id: &str,
        text: &str,
        metadata: HashMap<String, String>,
    ) -> Result<ChunkedDocument, Box<dyn std::error::Error + Send + Sync>> {
        let chunks = self.split_into_chunks(text);
        let mut chunk_objects = Vec::new();

        // Get round-robin counter start point
        let start_counter = self
            .round_robin_counter
            .fetch_add(chunks.len(), Ordering::SeqCst);

        for (idx, chunk_text) in chunks.into_iter().enumerate() {
            // Distribute chunks across shards using round-robin
            let shard_idx = (start_counter + idx) % self.shard_count();
            let shard_name = self.get_all_shard_names()[shard_idx].clone();

            let chunk_id = format!("{}_chunk_{}", doc_id, idx);
            let vector_key = format!("vector:{}", chunk_id);

            // Store vector embedding
            {
                let mut vector_stores = self.vector_stores.write();
                if let Some(vector_store) = vector_stores.get_mut(&shard_name) {
                    vector_store.insert_text(&vector_key, &chunk_text, Some("chunked_docs"))?;
                }
            }

            // Store chunk text for keyword search
            {
                let mut search_stores = self.search_stores.write();
                if let Some(search_store) = search_stores.get_mut(&shard_name) {
                    search_store.put_text(&chunk_id, &chunk_text, Some("chunked_docs"))?;
                }
            }

            chunk_objects.push(TextChunk {
                chunk_id,
                text: chunk_text,
                shard: shard_name,
                start_pos: idx * self.chunk_config.read().chunk_size,
                end_pos: (idx + 1) * self.chunk_config.read().chunk_size,
                vector_key,
            });
        }

        let doc = ChunkedDocument {
            id: doc_id.to_string(),
            original_text: text.to_string(),
            chunks: chunk_objects,
            metadata,
            created_at: Utc::now(),
        };

        // Store document metadata
        let doc_key = format!("doc:{}", doc_id);
        let serialized = serde_json::to_vec(&doc)?;

        // Store in first shard (round-robin)
        let shard_name = self.get_all_shard_names()[start_counter % self.shard_count()].clone();
        let mut stores = self.stores.write();
        if let Some(store) = stores.get_mut(&shard_name) {
            store.put(&doc_key, &serialized, Some("chunked_docs"))?;
        }

        Ok(doc)
    }
    /// Retrieve a chunked document
    pub fn get_chunked_document(
        &self,
        doc_id: &str,
    ) -> Result<Option<ChunkedDocument>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let doc_key = format!("doc:{}", doc_id);

        for store in stores.values() {
            if let Some(data) = store.get(&doc_key)? {
                let doc: ChunkedDocument = serde_json::from_slice(&data)?;
                return Ok(Some(doc));
            }
        }

        Ok(None)
    }
    /// Vector search across chunked documents
    pub fn vector_search_chunks(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ChunkSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let vector_stores = self.vector_stores.read();
        let search_stores = self.search_stores.read();
        let mut all_results = Vec::new();

        // Search in each shard
        for (shard_name, vector_store) in vector_stores.iter() {
            let results = vector_store.search_similar(query, limit)?;

            for result in results {
                // Extract chunk ID from vector key
                if result.key.starts_with("vector:") {
                    let chunk_id = result.key.replace("vector:", "");

                    // Get the chunk text from search store
                    let chunk_text = if let Some(search_store) = search_stores.get(shard_name) {
                        if let Some(data) = search_store.get(&chunk_id)? {
                            String::from_utf8_lossy(&data).to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    // Extract document ID from chunk ID
                    let doc_id = chunk_id
                        .split("_chunk_")
                        .next()
                        .unwrap_or(&chunk_id)
                        .to_string();

                    all_results.push(ChunkSearchResult {
                        document_id: doc_id,
                        chunk_id,
                        text: chunk_text,
                        score: result.score,
                        vector_score: result.score,
                        keyword_score: 0.0,
                        combined_score: result.score,
                        metadata: HashMap::new(),
                    });
                }
            }
        }

        // Sort by score and deduplicate by document (keep best chunk)
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_results.truncate(limit);

        Ok(all_results)
    }

    /// Hybrid search across chunked documents (vector + keyword)
    pub fn hybrid_search_chunks(
        &self,
        query: &str,
        limit: usize,
        vector_weight: f32,
    ) -> Result<Vec<ChunkSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let vector_stores = self.vector_stores.read();
        let search_stores = self.search_stores.read();
        let mut results_map: HashMap<String, ChunkSearchResult> = HashMap::new();

        // Vector search
        for (shard_name, vector_store) in vector_stores.iter() {
            let vector_results = vector_store.search_similar(query, limit * 2)?;

            for result in vector_results {
                if result.key.starts_with("vector:") {
                    let chunk_id = result.key.replace("vector:", "");
                    let doc_id = chunk_id
                        .split("_chunk_")
                        .next()
                        .unwrap_or(&chunk_id)
                        .to_string();

                    let chunk_text = if let Some(search_store) = search_stores.get(shard_name) {
                        if let Some(data) = search_store.get(&chunk_id)? {
                            String::from_utf8_lossy(&data).to_string()
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };

                    results_map.insert(
                        chunk_id.clone(),
                        ChunkSearchResult {
                            document_id: doc_id,
                            chunk_id,
                            text: chunk_text,
                            score: result.score,
                            vector_score: result.score,
                            keyword_score: 0.0,
                            combined_score: result.score * vector_weight,
                            metadata: HashMap::new(),
                        },
                    );
                }
            }
        }

        // Keyword search
        for (_shard_name, search_store) in search_stores.iter() {
            let keyword_results = search_store.search(query, limit * 2)?;

            for result in keyword_results {
                let chunk_id = result.key.clone();
                let doc_id = chunk_id
                    .split("_chunk_")
                    .next()
                    .unwrap_or(&chunk_id)
                    .to_string();

                let entry = results_map
                    .entry(chunk_id.clone())
                    .or_insert(ChunkSearchResult {
                        document_id: doc_id,
                        chunk_id,
                        text: String::new(),
                        score: 0.0,
                        vector_score: 0.0,
                        keyword_score: 0.0,
                        combined_score: 0.0,
                        metadata: HashMap::new(),
                    });

                let keyword_score = (result.score as f32 / 10.0).min(1.0);
                entry.keyword_score = keyword_score;
                entry.combined_score =
                    (entry.vector_score * vector_weight) + (keyword_score * (1.0 - vector_weight));
                entry.score = entry.combined_score;

                // Get chunk text if not already set
                if entry.text.is_empty() {
                    if let Some(data) = search_store.get(&entry.chunk_id)? {
                        entry.text = String::from_utf8_lossy(&data).to_string();
                    }
                }
            }
        }

        // Convert to vector and sort
        let mut results: Vec<ChunkSearchResult> = results_map.into_values().collect();
        results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        results.truncate(limit);

        Ok(results)
    }

    /// Search chunks by document ID
    pub fn search_chunks_by_document(
        &self,
        doc_id: &str,
    ) -> Result<Vec<TextChunk>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(doc) = self.get_chunked_document(doc_id)? {
            Ok(doc.chunks)
        } else {
            Ok(Vec::new())
        }
    }
    /// Delete a chunked document and all its chunks
    pub fn delete_chunked_document(
        &self,
        doc_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(doc) = self.get_chunked_document(doc_id)? {
            // Delete all chunks
            for chunk in &doc.chunks {
                // Delete vector embedding
                if let Some(_shard_name) = chunk.vector_key.split("vector:").nth(1) {
                    // This would require implementing delete in VectorStore
                }

                // Delete chunk from search store
                let mut search_stores = self.search_stores.write();
                if let Some(search_store) = search_stores.get_mut(&chunk.shard) {
                    let _ = search_store.remove(&chunk.chunk_id);
                }
            }

            // Delete document metadata
            let doc_key = format!("doc:{}", doc_id);
            let mut stores = self.stores.write();
            for store in stores.values_mut() {
                if store.exists(&doc_key)? {
                    return Ok(store.remove(&doc_key)?);
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }
    /// Get chunk statistics
    pub fn get_chunk_statistics(
        &self,
    ) -> Result<ChunkStatistics, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.chunk_config.read();
        let mut total_documents = 0;
        let mut total_chunks = 0;
        let mut chunks_per_shard: HashMap<String, usize> = HashMap::new();

        let stores = self.stores.read();
        for store in stores.values() {
            let keys = store.list_keys()?;
            for key in keys {
                if key.starts_with("doc:") {
                    total_documents += 1;
                    if let Some(data) = store.get(&key)? {
                        if let Ok(doc) = serde_json::from_slice::<ChunkedDocument>(&data) {
                            total_chunks += doc.chunks.len();
                            for chunk in doc.chunks {
                                *chunks_per_shard.entry(chunk.shard).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(ChunkStatistics {
            total_documents,
            total_chunks,
            avg_chunks_per_doc: if total_documents > 0 {
                total_chunks as f64 / total_documents as f64
            } else {
                0.0
            },
            chunks_per_shard,
            chunk_size: config.chunk_size,
            chunk_overlap: config.chunk_overlap,
        })
    }

    /// Advanced chunking with sentence and paragraph boundaries
    pub fn advanced_chunking(
        &self,
        text: &str,
        config: &AdvancedChunkingConfig,
    ) -> Vec<EnhancedTextChunk> {
        let mut chunks = Vec::new();
        let mut paragraphs: Vec<&str> = Vec::new();
        let text_len = text.len();

        // Split into paragraphs
        if config.break_on_paragraphs {
            paragraphs = text.split("\n\n").collect();
        } else {
            paragraphs.push(text);
        }

        let mut global_pos = 0;
        let mut sentence_index = 0;
        let sentence_regex = Regex::new(r"[.!?]+[\s\n]+").unwrap();

        for (para_idx, paragraph) in paragraphs.iter().enumerate() {
            let para_start = global_pos;

            // Split paragraph into sentences
            let sentences: Vec<&str> = if config.break_on_sentences {
                sentence_regex.split(paragraph).collect()
            } else {
                vec![paragraph]
            };

            let mut current_chunk = String::new();
            let mut chunk_start_pos = para_start;
            let mut chunk_start_sentence = sentence_index;

            for sentence in sentences {
                let sentence_len = sentence.len();

                if current_chunk.len() + sentence_len > config.chunk_size
                    && !current_chunk.is_empty()
                {
                    // Create chunk with context
                    let chunk_end_pos = chunk_start_pos + current_chunk.len();

                    // Calculate context windows safely
                    let context_start = if chunk_start_pos > config.context_before_chars {
                        chunk_start_pos - config.context_before_chars
                    } else {
                        0
                    };
                    let context_end = if chunk_end_pos + config.context_after_chars < text_len {
                        chunk_end_pos + config.context_after_chars
                    } else {
                        text_len
                    };

                    let context_before = if context_start < chunk_start_pos {
                        text[context_start..chunk_start_pos].to_string()
                    } else {
                        String::new()
                    };
                    let context_after = if chunk_end_pos < context_end {
                        text[chunk_end_pos..context_end].to_string()
                    } else {
                        String::new()
                    };

                    let stemmed_text = if config.enable_stemming {
                        Some(self.stem_text(&current_chunk, config.language))
                    } else {
                        None
                    };

                    chunks.push(EnhancedTextChunk {
                        chunk_id: format!("chunk_{}_{}", para_idx, chunks.len()),
                        text: current_chunk.clone(),
                        context_before,
                        context_after,
                        stemmed_text,
                        shard: String::new(),
                        start_pos: chunk_start_pos,
                        end_pos: chunk_end_pos,
                        start_sentence: chunk_start_sentence,
                        end_sentence: sentence_index,
                        paragraph_index: para_idx,
                        vector_key: String::new(),
                        metadata: HashMap::new(),
                    });

                    // Start new chunk with overlap
                    let overlap_start = if current_chunk.len() > config.chunk_overlap {
                        current_chunk.len() - config.chunk_overlap
                    } else {
                        0
                    };
                    current_chunk = current_chunk[overlap_start..].to_string();
                    chunk_start_pos = chunk_end_pos - (current_chunk.len());
                    chunk_start_sentence = sentence_index;
                }

                current_chunk.push_str(sentence);
                if config.break_on_sentences {
                    current_chunk.push_str(". ");
                }
                global_pos += sentence_len + 2;
                sentence_index += 1;
            }

            // Add last chunk
            if !current_chunk.is_empty() && current_chunk.len() >= config.min_chunk_size {
                let chunk_end_pos = chunk_start_pos + current_chunk.len();

                let context_start = if chunk_start_pos > config.context_before_chars {
                    chunk_start_pos - config.context_before_chars
                } else {
                    0
                };
                let context_end = if chunk_end_pos + config.context_after_chars < text_len {
                    chunk_end_pos + config.context_after_chars
                } else {
                    text_len
                };

                let context_before = if context_start < chunk_start_pos {
                    text[context_start..chunk_start_pos].to_string()
                } else {
                    String::new()
                };
                let context_after = if chunk_end_pos < context_end {
                    text[chunk_end_pos..context_end].to_string()
                } else {
                    String::new()
                };

                let stemmed_text = if config.enable_stemming {
                    Some(self.stem_text(&current_chunk, config.language))
                } else {
                    None
                };

                chunks.push(EnhancedTextChunk {
                    chunk_id: format!("chunk_{}_{}", para_idx, chunks.len()),
                    text: current_chunk,
                    context_before,
                    context_after,
                    stemmed_text,
                    shard: String::new(),
                    start_pos: chunk_start_pos,
                    end_pos: chunk_end_pos,
                    start_sentence: chunk_start_sentence,
                    end_sentence: sentence_index,
                    paragraph_index: para_idx,
                    vector_key: String::new(),
                    metadata: HashMap::new(),
                });
            }

            global_pos += 2; // For paragraph separation
        }

        chunks
    }

    /// Stem text using snowball
    fn stem_text(&self, text: &str, language: StemmingLanguage) -> String {
        let algorithm = match language {
            StemmingLanguage::English => Algorithm::English,
            StemmingLanguage::Russian => Algorithm::Russian,
            StemmingLanguage::German => Algorithm::German,
            StemmingLanguage::French => Algorithm::French,
            StemmingLanguage::Spanish => Algorithm::Spanish,
            StemmingLanguage::Italian => Algorithm::Italian,
            StemmingLanguage::Dutch => Algorithm::Dutch,
            StemmingLanguage::Portuguese => Algorithm::Portuguese,
        };

        let stemmer = Stemmer::create(algorithm);

        // Stem each word individually - stem returns Cow<str>
        text.split_whitespace()
            .map(|word| {
                let stemmed = stemmer.stem(word);
                stemmed.to_string() // Convert Cow<str> to String directly
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Store document with advanced chunking
    pub fn store_advanced_chunked_document(
        &self,
        doc_id: &str,
        text: &str,
        metadata: HashMap<String, String>,
        config: &AdvancedChunkingConfig,
    ) -> Result<EnhancedChunkedDocument, Box<dyn std::error::Error + Send + Sync>> {
        // Create advanced chunks
        let mut chunks = self.advanced_chunking(text, config);

        // Distribute chunks across shards using round-robin
        let start_counter = self
            .round_robin_counter
            .fetch_add(chunks.len(), Ordering::SeqCst);

        for (idx, chunk) in chunks.iter_mut().enumerate() {
            let shard_idx = (start_counter + idx) % self.shard_count();
            let shard_name = self.get_all_shard_names()[shard_idx].clone();
            chunk.shard = shard_name.clone();

            let chunk_id = format!("{}_chunk_{}", doc_id, idx);
            chunk.chunk_id = chunk_id.clone();

            // Store vector embedding
            let text_for_embedding = if config.enable_stemming {
                chunk.stemmed_text.as_ref().unwrap_or(&chunk.text)
            } else {
                &chunk.text
            };

            let vector_key = format!("vector:{}", chunk_id);
            chunk.vector_key = vector_key.clone();

            {
                let mut vector_stores = self.vector_stores.write();
                if let Some(vector_store) = vector_stores.get_mut(&shard_name) {
                    vector_store.insert_text(
                        &vector_key,
                        text_for_embedding,
                        Some("advanced_chunks"),
                    )?;
                }
            }

            // Store chunk text for keyword search
            {
                let mut search_stores = self.search_stores.write();
                if let Some(search_store) = search_stores.get_mut(&shard_name) {
                    let search_text = if config.enable_stemming {
                        format!(
                            "{}\nContext before: {}\nContext after: {}\n{}",
                            chunk.text,
                            chunk.context_before,
                            chunk.context_after,
                            chunk.stemmed_text.as_ref().unwrap_or(&String::new())
                        )
                    } else {
                        format!(
                            "{}\nContext before: {}\nContext after: {}",
                            chunk.text, chunk.context_before, chunk.context_after
                        )
                    };
                    search_store.put_text(&chunk_id, &search_text, Some("advanced_chunks"))?;
                }
            }

            // Preserve metadata
            if config.preserve_metadata {
                chunk.metadata = metadata.clone();
            }
        }

        // Calculate statistics
        let word_count = text.unicode_words().count();
        let sentence_regex = Regex::new(r"[.!?]+[\s\n]+").unwrap();
        let sentence_count = sentence_regex.split(text).count();
        let paragraph_count = text.split("\n\n").count();

        let doc = EnhancedChunkedDocument {
            id: doc_id.to_string(),
            original_text: text.to_string(),
            chunks,
            metadata,
            created_at: Utc::now(),
            word_count,
            sentence_count,
            paragraph_count,
        };

        // Store document metadata
        let doc_key = format!("advanced_doc:{}", doc_id);
        let serialized = serde_json::to_vec(&doc)?;

        let shard_name = self.get_all_shard_names()[start_counter % self.shard_count()].clone();
        let mut stores = self.stores.write();
        if let Some(store) = stores.get_mut(&shard_name) {
            store.put(&doc_key, &serialized, Some("advanced_docs"))?;
        }

        Ok(doc)
    }

    /// Search advanced chunks with RAG-friendly context
    pub fn search_advanced_chunks(
        &self,
        query: &str,
        limit: usize,
        vector_weight: f32,
        include_context: bool,
    ) -> Result<Vec<EnhancedChunkSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let vector_stores = self.vector_stores.read();
        let search_stores = self.search_stores.read();
        let mut results_map: HashMap<String, EnhancedChunkSearchResult> = HashMap::new();

        // Vector search
        for (_shard_name, vector_store) in vector_stores.iter() {
            let vector_results = vector_store.search_similar(query, limit * 2)?;

            for result in vector_results {
                if result.key.starts_with("vector:") {
                    let chunk_id = result.key.replace("vector:", "");
                    let doc_id = chunk_id
                        .split("_chunk_")
                        .next()
                        .unwrap_or(&chunk_id)
                        .to_string();

                    // Get enhanced chunk data
                    if let Some(doc) = self.get_advanced_chunked_document(&doc_id)? {
                        if let Some(chunk) = doc.chunks.iter().find(|c| c.chunk_id == chunk_id) {
                            let relevance_context = if include_context {
                                format!(
                                    "[Context Before]\n{}\n\n[Main Content]\n{}\n\n[Context After]\n{}",
                                    chunk.context_before, chunk.text, chunk.context_after
                                )
                            } else {
                                chunk.text.clone()
                            };

                            results_map.insert(
                                chunk_id.clone(),
                                EnhancedChunkSearchResult {
                                    document_id: doc_id.clone(),
                                    chunk_id: chunk_id.clone(),
                                    text: chunk.text.clone(),
                                    context_before: chunk.context_before.clone(),
                                    context_after: chunk.context_after.clone(),
                                    score: result.score,
                                    vector_score: result.score,
                                    keyword_score: 0.0,
                                    combined_score: result.score * vector_weight,
                                    metadata: chunk.metadata.clone(),
                                    relevance_context,
                                },
                            );
                        }
                    }
                }
            }
        }

        // Keyword search
        for (_shard_name, search_store) in search_stores.iter() {
            let keyword_results = search_store.search(query, limit * 2)?;

            for result in keyword_results {
                let chunk_id = result.key.clone();
                let doc_id = chunk_id
                    .split("_chunk_")
                    .next()
                    .unwrap_or(&chunk_id)
                    .to_string();

                let entry = results_map.entry(chunk_id.clone()).or_insert_with(|| {
                    EnhancedChunkSearchResult {
                        document_id: doc_id.clone(),
                        chunk_id: chunk_id.clone(),
                        text: String::new(),
                        context_before: String::new(),
                        context_after: String::new(),
                        score: 0.0,
                        vector_score: 0.0,
                        keyword_score: 0.0,
                        combined_score: 0.0,
                        metadata: HashMap::new(),
                        relevance_context: String::new(),
                    }
                });

                let keyword_score = (result.score as f32 / 10.0).min(1.0);
                entry.keyword_score = keyword_score;
                entry.combined_score =
                    (entry.vector_score * vector_weight) + (keyword_score * (1.0 - vector_weight));
                entry.score = entry.combined_score;

                // Get full chunk data if not already loaded
                if entry.text.is_empty() {
                    if let Some(doc) = self.get_advanced_chunked_document(&doc_id)? {
                        if let Some(chunk) = doc.chunks.iter().find(|c| c.chunk_id == chunk_id) {
                            entry.text = chunk.text.clone();
                            entry.context_before = chunk.context_before.clone();
                            entry.context_after = chunk.context_after.clone();
                            entry.metadata = chunk.metadata.clone();
                            entry.relevance_context = if include_context {
                                format!(
                                    "[Context Before]\n{}\n\n[Main Content]\n{}\n\n[Context After]\n{}",
                                    chunk.context_before, chunk.text, chunk.context_after
                                )
                            } else {
                                chunk.text.clone()
                            };
                        }
                    }
                }
            }
        }

        // Convert to vector and sort
        let mut results: Vec<EnhancedChunkSearchResult> = results_map.into_values().collect();
        results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        results.truncate(limit);

        Ok(results)
    }

    /// Get advanced chunked document
    pub fn get_advanced_chunked_document(
        &self,
        doc_id: &str,
    ) -> Result<Option<EnhancedChunkedDocument>, Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let doc_key = format!("advanced_doc:{}", doc_id);

        for store in stores.values() {
            if let Some(data) = store.get(&doc_key)? {
                let doc: EnhancedChunkedDocument = serde_json::from_slice(&data)?;
                return Ok(Some(doc));
            }
        }

        Ok(None)
    }

    /// Get chunks for RAG context window
    pub fn get_chunks_for_rag(
        &self,
        doc_id: &str,
        chunk_ids: Vec<String>,
        context_window_chars: usize,
    ) -> Result<Vec<EnhancedChunkSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();

        if let Some(doc) = self.get_advanced_chunked_document(doc_id)? {
            for chunk_id in chunk_ids {
                if let Some(chunk) = doc.chunks.iter().find(|c| c.chunk_id == chunk_id) {
                    // Expand context window
                    let start = if chunk.start_pos > context_window_chars {
                        chunk.start_pos - context_window_chars
                    } else {
                        0
                    };
                    let end = if chunk.end_pos + context_window_chars < doc.original_text.len() {
                        chunk.end_pos + context_window_chars
                    } else {
                        doc.original_text.len()
                    };

                    let expanded_context = doc.original_text[start..end].to_string();

                    results.push(EnhancedChunkSearchResult {
                        document_id: doc_id.to_string(),
                        chunk_id: chunk.chunk_id.clone(),
                        text: chunk.text.clone(),
                        context_before: chunk.context_before.clone(),
                        context_after: chunk.context_after.clone(),
                        score: 1.0,
                        vector_score: 1.0,
                        keyword_score: 1.0,
                        combined_score: 1.0,
                        metadata: chunk.metadata.clone(),
                        relevance_context: expanded_context,
                    });
                }
            }
        }

        Ok(results)
    }
    /// Clear all caches (time bucket cache and key clusters)
    pub fn clear_caches(&self) {
        let time_bucket_count = self.time_bucket_cache.read().len();
        let key_cluster_count = self.key_clusters.read().len();

        self.time_bucket_cache.write().clear();
        self.key_clusters.write().clear();

        log::debug!(
            "[Cache] Cleared {} time bucket entries and {} key cluster entries",
            time_bucket_count,
            key_cluster_count
        );
    }

    /// Clear specific cache types
    pub fn clear_cache_by_type(&self, cache_type: CacheType) {
        match cache_type {
            CacheType::TimeBucket => {
                let count = self.time_bucket_cache.read().len();
                self.time_bucket_cache.write().clear();
                log::debug!("[Cache] Cleared {} time bucket cache entries", count);
            }
            CacheType::KeyCluster => {
                let count = self.key_clusters.read().len();
                self.key_clusters.write().clear();
                log::debug!("[Cache] Cleared {} key cluster cache entries", count);
            }
            CacheType::All => {
                self.clear_caches();
            }
        }
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> CacheStats {
        CacheStats {
            time_bucket_cache_size: self.time_bucket_cache.read().len(),
            key_cluster_cache_size: self.key_clusters.read().len(),
            total_cache_size: self.time_bucket_cache.read().len() + self.key_clusters.read().len(),
        }
    }

    /// Sync all shards - ensures data consistency by clearing caches
    /// and verifying all shards are accessible
    pub fn sync_all_shards(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("[Sync] Starting sync across all shards...");

        // Clear all caches to ensure fresh reads
        self.clear_caches();

        let shard_names = self.get_all_shard_names();
        let mut failed_shards = Vec::new();

        // Verify each shard is accessible
        for shard_name in &shard_names {
            match self.verify_shard_accessibility(shard_name) {
                Ok(true) => log::debug!("[Sync] Shard '{}' is accessible", shard_name),
                Ok(false) => {
                    log::warn!("[Sync] Shard '{}' has issues", shard_name);
                    failed_shards.push(shard_name.clone());
                }
                Err(e) => {
                    log::error!("[Sync] Error accessing shard '{}': {}", shard_name, e);
                    failed_shards.push(shard_name.clone());
                }
            }
        }

        if !failed_shards.is_empty() {
            log::warn!(
                "[Sync] Warning: {} shards have issues: {:?}",
                failed_shards.len(),
                failed_shards
            );
        } else {
            log::debug!(
                "[Sync] All {} shards synced successfully",
                shard_names.len()
            );
        }

        Ok(())
    }

    /// Sync a specific shard by name
    pub fn sync_shard(
        &self,
        shard_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.shard_exists(shard_name) {
            return Err(format!("Shard '{}' not found", shard_name).into());
        }

        log::debug!("[Sync] Syncing shard: {}", shard_name);

        // Clear cache entries related to this shard
        {
            let mut time_cache = self.time_bucket_cache.write();
            let before = time_cache.len();
            time_cache.retain(|_, value| value != shard_name);
            let after = time_cache.len();
            if before > after {
                log::debug!(
                    "[Sync] Cleared {} time bucket cache entries for shard '{}'",
                    before - after,
                    shard_name
                );
            }
        }

        {
            let mut key_clusters = self.key_clusters.write();
            let before = key_clusters.len();
            key_clusters.retain(|_, cluster| cluster.shard != shard_name);
            let after = key_clusters.len();
            if before > after {
                log::debug!(
                    "[Sync] Cleared {} key cluster entries for shard '{}'",
                    before - after,
                    shard_name
                );
            }
        }

        // Verify shard accessibility
        match self.verify_shard_accessibility(shard_name) {
            Ok(true) => log::debug!("[Sync] Shard '{}' synced successfully", shard_name),
            Ok(false) => log::debug!("[Sync] Shard '{}' synced but has issues", shard_name),
            Err(e) => return Err(format!("Shard '{}' sync failed: {}", shard_name, e).into()),
        }

        Ok(())
    }
    /// Verify shard accessibility by trying to read/write a test key
    fn verify_shard_accessibility(
        &self,
        shard_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let test_key = format!(
            "__sync_test_{}",
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );

        let mut stores = self.stores.write();
        if let Some(store) = stores.get_mut(shard_name) {
            // Try to write a test key
            store.put(&test_key, b"test", None)?;

            // Try to read it back
            let data = store.get(&test_key)?;
            let success = data.is_some() && data.unwrap() == b"test";

            // Clean up
            store.remove(&test_key)?;

            Ok(success)
        } else {
            Ok(false)
        }
    }
    /// Flush all pending operations and sync
    pub fn flush_and_sync(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("[Flush] Starting flush and sync...");

        // Clear caches first
        self.clear_caches();

        // Then sync all shards
        self.sync_all_shards()?;

        log::debug!("[Flush] Flush and sync completed");
        Ok(())
    }

    /// Reset and reinitialize the entire distribution manager
    pub fn reset(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        log::debug!("[Reset] Resetting DataDistributionManager...");

        // Clear all caches
        self.clear_caches();

        // Reset round-robin counter
        self.round_robin_counter.store(0, Ordering::SeqCst);

        // Sync all shards
        self.sync_all_shards()?;

        // Reset load history
        self.load_history.write().clear();

        log::debug!("[Reset] Reset completed successfully");
        Ok(())
    }

    /// Get shard health status
    pub fn get_shard_health(&self) -> Vec<ShardHealth> {
        let stores = self.stores.read();
        let mut health_status = Vec::new();

        for (shard_name, store) in stores.iter() {
            let is_healthy = store.len().is_ok();
            let key_count = store.len().unwrap_or(0);

            health_status.push(ShardHealth {
                shard_name: shard_name.clone(),
                is_healthy,
                key_count,
                last_sync: Utc::now(),
            });
        }

        health_status
    }

    /// Optimize all shards (compact storage)
    pub fn optimize_all_shards(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stores = self.stores.read();
        let vector_stores = self.vector_stores.read();
        let search_stores = self.search_stores.read();

        log::debug!("[Optimize] Starting optimization across all shards...");

        // Optimize blob stores
        for (shard_name, store) in stores.iter() {
            log::debug!("[Optimize] Optimizing blob store for shard: {}", shard_name);
            store.optimize()?;
        }

        // Optimize vector stores
        for (shard_name, vector_store) in vector_stores.iter() {
            log::debug!(
                "[Optimize] Optimizing vector store for shard: {}",
                shard_name
            );
            vector_store.optimize()?;
        }

        // Optimize search stores
        for (shard_name, search_store) in search_stores.iter() {
            log::debug!(
                "[Optimize] Optimizing search store for shard: {}",
                shard_name
            );
            search_store.optimize()?;
        }

        log::debug!("[Optimize] Optimization completed");
        Ok(())
    }

    /// Get overall system statistics
    pub fn get_system_stats(&self) -> SystemStats {
        let shard_health = self.get_shard_health();
        let cache_stats = self.get_cache_stats();
        let distribution_stats = self.get_distribution_stats();

        SystemStats {
            shard_health,
            cache_stats,
            total_records: distribution_stats.total_records,
            shard_count: self.shard_count(),
            distribution_entropy: distribution_stats.distribution_entropy,
            load_balance_score: distribution_stats.load_balance_score,
        }
    }
}

// Helper function for Levenshtein distance
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            matrix[i][j] = std::cmp::min(
                std::cmp::min(matrix[i - 1][j] + 1, matrix[i][j - 1] + 1),
                matrix[i - 1][j - 1] + cost,
            );
        }
    }

    matrix[a_len][b_len]
}

#[derive(Debug, Clone)]
pub struct ChunkStatistics {
    pub total_documents: usize,
    pub total_chunks: usize,
    pub avg_chunks_per_doc: f64,
    pub chunks_per_shard: HashMap<String, usize>,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
}
