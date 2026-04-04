use crate::blobstore::BlobMetadata;
use crate::search::{FuzzySearchResult, SearchResult};
use crate::sharding::{CacheConfig, ShardManager, ShardManagerBuilder, ShardingStrategy};
use crate::timeline::{TelemetryQuery, TelemetryRecord};
use crate::vector::VectorSearchResult;
use crate::vector_timeline::{VectorTimeQuery, VectorTimeResult};
use chrono::{DateTime, Datelike, LocalResult, TimeZone, Timelike, Utc};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Distribution strategy types
#[derive(Debug, Clone)]
pub enum DistributionStrategy {
    RoundRobin,
    TimeBucket(TimeBucketConfig),
    KeySimilarity(SimilarityConfig),
    Adaptive(AdaptiveConfig),
}

/// Time bucket configuration
#[derive(Debug, Clone, PartialEq)]
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
    Minutes(u32), // 1-60 minutes
    Hours(u32),   // 1-24 hours
    Days(u32),    // 1-30 days
    Weeks(u32),   // 1-4 weeks
    Months(u32),  // 1-12 months
}

/// Key similarity configuration
#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
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

/// Data distribution manager
pub struct DataDistributionManager {
    shard_manager: Arc<ShardManager>,
    strategy: Arc<RwLock<DistributionStrategy>>,
    time_bucket_cache: Arc<RwLock<HashMap<String, String>>>,
    key_clusters: Arc<RwLock<HashMap<String, SimilarityCluster>>>,
    load_history: Arc<RwLock<VecDeque<HashMap<String, usize>>>>,
    adaptive_config: AdaptiveConfig,
    round_robin_counter: Arc<AtomicUsize>,
}

impl DataDistributionManager {
    /// Create a new data distribution manager
    pub fn new<P: AsRef<Path>>(
        base_path: P,
        strategy: DistributionStrategy,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let base_path = base_path.as_ref();
        std::fs::create_dir_all(base_path)?;

        let mut builder = ShardManagerBuilder::new()
            .with_strategy(ShardingStrategy::ConsistentHash)
            .with_cache_config(CacheConfig::default());

        // Explicitly add 4 shards with unique paths
        for i in 0..4 {
            let shard_path = base_path.join(format!("shard_{}", i));
            std::fs::create_dir_all(&shard_path)?;
            builder = builder.add_shard(&format!("shard_{}", i), shard_path.to_str().unwrap());
        }

        let shard_manager = Arc::new(builder.build()?);

        Ok(DataDistributionManager {
            shard_manager,
            strategy: Arc::new(RwLock::new(strategy)),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
            time_bucket_cache: Arc::new(RwLock::new(HashMap::new())),
            key_clusters: Arc::new(RwLock::new(HashMap::new())),
            load_history: Arc::new(RwLock::new(VecDeque::new())),
            adaptive_config: AdaptiveConfig::default(),
        })
    }

    /// Create with custom number of shards
    pub fn with_shards<P: AsRef<Path>>(
        base_path: P,
        strategy: DistributionStrategy,
        num_shards: usize,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let base_path = base_path.as_ref();
        std::fs::create_dir_all(base_path)?;

        let mut builder = ShardManagerBuilder::new()
            .with_strategy(ShardingStrategy::ConsistentHash)
            .with_cache_config(CacheConfig::default());

        for i in 0..num_shards {
            let shard_path = base_path.join(format!("shard_{}", i));
            std::fs::create_dir_all(&shard_path)?;
            builder = builder.add_shard(&format!("shard_{}", i), shard_path.to_str().unwrap());
        }

        let shard_manager = Arc::new(builder.build()?);

        Ok(DataDistributionManager {
            shard_manager,
            strategy: Arc::new(RwLock::new(strategy)),
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
            time_bucket_cache: Arc::new(RwLock::new(HashMap::new())),
            key_clusters: Arc::new(RwLock::new(HashMap::new())),
            load_history: Arc::new(RwLock::new(VecDeque::new())),
            adaptive_config: AdaptiveConfig::default(),
        })
    }

    /// Store data with automatic distribution
    pub fn put(
        &self,
        key: &str,
        data: &[u8],
        _timestamp: Option<DateTime<Utc>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let strategy = self.strategy.read().clone();

        let shard_name = match strategy {
            DistributionStrategy::RoundRobin => self.round_robin_distribution()?,
            DistributionStrategy::TimeBucket(config) => {
                self.time_bucket_distribution(key, _timestamp, &config)?
            }
            DistributionStrategy::KeySimilarity(config) => {
                self.key_similarity_distribution(key, &config)?
            }
            DistributionStrategy::Adaptive(config) => {
                self.adaptive_distribution(key, _timestamp, &config)?
            }
        };

        // Get the shard using the shard name as a key
        let shard = self.shard_manager.get_shard_for_key(&shard_name);
        shard.blob().put(key, data, None)?;

        self.update_load_history();

        Ok(())
    }

    /// Store telemetry record with automatic distribution
    pub fn put_telemetry(
        &self,
        record: TelemetryRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let timestamp = Some(record.timestamp());
        let shard_id = self.determine_shard(&record.key, timestamp)?;
        let shard = self.shard_manager.get_shard_for_key(&shard_id);
        shard.telemetry().store(record)?;

        self.update_load_history();

        Ok(())
    }

    /// Determine which shard to use based on strategy
    fn determine_shard(
        &self,
        key: &str,
        timestamp: Option<DateTime<Utc>>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let strategy = self.strategy.read().clone();

        match strategy {
            DistributionStrategy::RoundRobin => self.round_robin_distribution(),
            DistributionStrategy::TimeBucket(config) => {
                self.time_bucket_distribution(key, timestamp, &config)
            }
            DistributionStrategy::KeySimilarity(config) => {
                self.key_similarity_distribution(key, &config)
            }
            DistributionStrategy::Adaptive(config) => {
                self.adaptive_distribution(key, timestamp, &config)
            }
        }
    }

    /// Round robin distribution
    fn round_robin_distribution(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let shards = self.get_shard_names();

        if shards.is_empty() {
            return Err("No shards available".into());
        }

        // Atomically fetch and increment the counter
        let counter = self.round_robin_counter.fetch_add(1, Ordering::SeqCst);
        let idx = counter % shards.len();
        let shard_name = shards[idx].clone();

        log::debug!(
            "Round-robin: counter={}, idx={}, shard={}, total_shards={}",
            counter,
            idx,
            shard_name,
            shards.len()
        );

        Ok(shard_name)
    }

    /// Time bucket distribution
    fn time_bucket_distribution(
        &self,
        _key: &str,
        timestamp: Option<DateTime<Utc>>,
        config: &TimeBucketConfig,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let ts = timestamp.unwrap_or_else(Utc::now);
        let bucket_key = self.get_time_bucket_key(ts, config);

        // Check cache
        if let Some(shard) = self.time_bucket_cache.read().get(&bucket_key) {
            return Ok(shard.clone());
        }

        // Determine shard based on bucket hash
        let shards = self.get_shard_names();
        let hash = self.hash_string(&bucket_key);
        let shard_idx = hash % shards.len();
        let shard_name = shards[shard_idx].clone();

        // Cache the result
        self.time_bucket_cache
            .write()
            .insert(bucket_key, shard_name.clone());

        Ok(shard_name)
    }

    /// Key similarity distribution
    fn key_similarity_distribution(
        &self,
        key: &str,
        config: &SimilarityConfig,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Find existing cluster
        let clusters = self.key_clusters.read();

        // Try to find similar existing key
        for (existing_key, cluster) in clusters.iter() {
            let similarity = self.calculate_key_similarity(key, existing_key, config);
            if similarity >= config.min_similarity && cluster.size < config.max_cluster_size {
                return Ok(cluster.shard.clone());
            }
        }

        // Create new cluster
        let shards = self.get_shard_names();
        let hash = self.hash_string(key);
        let shard_idx = hash % shards.len();
        let shard_name = shards[shard_idx].clone();

        let cluster = SimilarityCluster {
            cluster_id: format!("cluster_{}", hash),
            keys: vec![key.to_string()],
            shard: shard_name.clone(),
            size: 1,
            similarity_score: 1.0,
        };

        drop(clusters);
        self.key_clusters.write().insert(key.to_string(), cluster);

        Ok(shard_name)
    }

    /// Adaptive distribution based on load
    fn adaptive_distribution(
        &self,
        _key: &str,
        _timestamp: Option<DateTime<Utc>>,
        config: &AdaptiveConfig,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let shards = self.get_shard_names();
        let loads = self.get_shard_loads();

        // Find least loaded shard
        let min_load_shard = loads
            .iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| shards[0].clone());

        // Check if rebalancing is needed
        if self.should_rebalance(&loads, config) {
            self.trigger_rebalance();
        }

        Ok(min_load_shard)
    }

    /// Calculate key similarity using various methods
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

    /// Get time bucket key
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

    /// Get shard loads
    fn get_shard_loads(&self) -> HashMap<String, f64> {
        let stats = self.shard_manager.shard_statistics();
        let total_keys: usize = stats.shard_details.iter().map(|d| d.key_count).sum();

        stats
            .shard_details
            .iter()
            .map(|detail| {
                let load = if total_keys > 0 {
                    detail.key_count as f64 / total_keys as f64
                } else {
                    0.0
                };
                (detail.name.clone(), load)
            })
            .collect()
    }

    /// Check if rebalancing is needed
    fn should_rebalance(&self, loads: &HashMap<String, f64>, config: &AdaptiveConfig) -> bool {
        let loads_vec: Vec<f64> = loads.values().cloned().collect();
        if loads_vec.is_empty() {
            return false;
        }

        let max_load = loads_vec.iter().fold(0.0_f64, |a, &b| a.max(b));
        let min_load = loads_vec.iter().fold(f64::INFINITY, |a, &b| a.min(b));

        max_load - min_load > config.rebalance_threshold
    }

    /// Trigger rebalancing
    fn trigger_rebalance(&self) {
        // Clear caches
        self.time_bucket_cache.write().clear();
        self.key_clusters.write().clear();
    }

    /// Update load history
    fn update_load_history(&self) {
        let loads = self.get_shard_loads();
        let mut history = self.load_history.write();

        // Convert f64 loads to usize for history
        let loads_usize: HashMap<String, usize> = loads
            .iter()
            .map(|(k, v)| (k.clone(), (*v * 1000.0) as usize))
            .collect();

        history.push_back(loads_usize);

        while history.len() > self.adaptive_config.history_size {
            history.pop_front();
        }
    }

    /// Get distribution statistics
    pub fn get_distribution_stats(&self) -> DistributionStats {
        let stats = self.shard_manager.shard_statistics();
        let shard_distribution: HashMap<String, usize> = stats
            .shard_details
            .iter()
            .map(|d| (d.name.clone(), d.key_count))
            .collect();

        let total_records: usize = shard_distribution.values().sum();

        // Calculate entropy (distribution uniformity)
        let entropy = if total_records > 0 {
            let num_shards = shard_distribution.len() as f64;
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
        let load_balance_score = if total_records > 0 {
            let max_load = *shard_distribution.values().max().unwrap_or(&0) as f64;
            let avg_load = total_records as f64 / shard_distribution.len() as f64;
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
            similarity_clusters: self.get_similarity_clusters(),
        }
    }

    fn get_similarity_clusters(&self) -> Vec<SimilarityCluster> {
        self.key_clusters.read().values().cloned().collect()
    }

    /// Change distribution strategy at runtime
    pub fn set_strategy(&self, strategy: DistributionStrategy) {
        *self.strategy.write() = strategy;
    }

    /// Get current strategy
    pub fn get_strategy(&self) -> DistributionStrategy {
        self.strategy.read().clone()
    }

    fn get_shard_names(&self) -> Vec<String> {
        let stats = self.shard_manager.shard_statistics();
        let mut names: Vec<String> = stats.shard_details.iter().map(|d| d.name.clone()).collect();
        names.sort(); // Sort alphabetically to ensure consistent order
        // Debug output to verify shards
        if names.len() < 4 {
            eprintln!("Warning: Only {} shards available, expected 4", names.len());
        }
        names
    }

    /// Simple hash function
    fn hash_string(&self, s: &str) -> usize {
        s.bytes().fold(0usize, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as usize)
        })
    }

    /// Get underlying shard manager
    pub fn shard_manager(&self) -> Arc<ShardManager> {
        self.shard_manager.clone()
    }

    // ========== Unified Retrieval Interface ==========

    /// Unified get operation - automatically routes to correct shard
    pub fn get(
        &self,
        key: &str,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        // First try the shard that would be used for writing (for round-robin, this is deterministic)
        // This optimizes the common case
        let strategy = self.strategy.read().clone();
        let predicted_shard = match strategy {
            DistributionStrategy::RoundRobin => {
                // For round-robin, we can't predict which shard a key was written to without state
                // So we'll scan all shards
                None
            }
            DistributionStrategy::KeySimilarity(_) => {
                // For key similarity, we can compute the shard
                Some(self.key_similarity_distribution(key, &SimilarityConfig::default())?)
            }
            _ => None,
        };

        if let Some(shard_id) = predicted_shard {
            let shard = self.shard_manager.get_shard_for_key(&shard_id);
            if let Some(data) = shard.blob().get(key)? {
                return Ok(Some(data));
            }
        }

        // Fall back to scanning all shards
        let shard_names = self.get_all_shard_names();
        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            if let Some(data) = shard.blob().get(key)? {
                return Ok(Some(data));
            }
        }

        Ok(None)
    }

    /// Unified get with metadata
    pub fn get_with_metadata(
        &self,
        key: &str,
    ) -> Result<Option<(Vec<u8>, BlobMetadata)>, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            if let Some(data) = shard.blob().get(key)? {
                let read_guard = shard.blob().read();
                if let Ok(Some(metadata)) = read_guard.get_metadata(key) {
                    return Ok(Some((data, metadata)));
                }
            }
        }

        Ok(None)
    }

    /// Unified telemetry query across all shards
    pub fn query_telemetry(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();
        let mut all_records = Vec::new();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            let records = shard.telemetry().query(query)?;
            all_records.extend(records);
        }

        // Apply limit and offset after merging
        let start = query.offset.min(all_records.len());
        let end = (start + query.limit).min(all_records.len());

        Ok(all_records[start..end].to_vec())
    }

    /// Unified search across all shards
    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();
        let mut all_results = Vec::new();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            let results = shard.search().search(query, limit)?;
            all_results.extend(results);
        }

        // Sort by score and truncate
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_results.truncate(limit);

        Ok(all_results)
    }

    /// Unified fuzzy search across all shards
    pub fn fuzzy_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FuzzySearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();
        let mut all_results = Vec::new();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            let results = shard.search().fuzzy_search(query, limit)?;
            all_results.extend(results);
        }

        // Sort by score and truncate
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_results.truncate(limit);

        Ok(all_results)
    }

    /// Unified vector search across all shards
    pub fn vector_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();
        let mut all_results = Vec::new();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            let results = shard.vector().search_similar(query, limit)?;
            all_results.extend(results);
        }

        // Sort by score and truncate
        all_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        all_results.truncate(limit);

        Ok(all_results)
    }

    /// Unified telemetry search with time and vector constraints
    pub fn search_vector_time(
        &self,
        query: &VectorTimeQuery,
    ) -> Result<Vec<VectorTimeResult>, Box<dyn std::error::Error + Send + Sync>> {
        let target_shards = if let Some(time_interval) = &query.time_interval {
            self.get_shards_for_time_interval(time_interval.start, time_interval.end)
        } else {
            self.get_all_shard_names()
        };

        let mut all_results = Vec::new();

        for shard_name in target_shards {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            let results = shard.vector_telemetry().search_vector_time(query)?;
            all_results.extend(results);
        }

        // Sort by combined score and truncate
        all_results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        all_results.truncate(query.limit);

        Ok(all_results)
    }

    /// Delete a key across all shards (finds and deletes)
    pub fn delete(&self, key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            if shard.blob().exists(key)? {
                return Ok(shard.blob().remove(key)?);
            }
        }

        Ok(false)
    }

    /// Check if a key exists across all shards
    pub fn exists(&self, key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            if shard.blob().exists(key)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// List all keys matching a pattern across all shards
    pub fn list_keys(
        &self,
        pattern: Option<&str>,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let shard_names = self.get_all_shard_names();
        let mut all_keys = Vec::new();

        for shard_name in shard_names {
            let shard = self.shard_manager.get_shard_for_key(&shard_name);
            let keys = shard.blob().list_keys()?;

            if let Some(pattern) = pattern {
                let matched_keys: Vec<String> =
                    keys.into_iter().filter(|k| k.contains(pattern)).collect();
                all_keys.extend(matched_keys);
            } else {
                all_keys.extend(keys);
            }
        }

        Ok(all_keys)
    }

    /// Get statistics about data distribution
    pub fn get_stats(&self) -> DistributionStats {
        self.get_distribution_stats()
    }

    /// Helper: Get shard for a specific key
    #[allow(dead_code)]
    fn get_shard_for_key(
        &self,
        key: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // For determining which shard to use for storing a new key,
        // we use the distribution strategy directly without looking up an existing shard
        let strategy = self.strategy.read().clone();

        match strategy {
            DistributionStrategy::RoundRobin => self.round_robin_distribution(),
            DistributionStrategy::TimeBucket(config) => {
                self.time_bucket_distribution(key, None, &config)
            }
            DistributionStrategy::KeySimilarity(config) => {
                self.key_similarity_distribution(key, &config)
            }
            DistributionStrategy::Adaptive(config) => {
                self.adaptive_distribution(key, None, &config)
            }
        }
    }

    /// Helper: Get shards for a time interval
    fn get_shards_for_time_interval(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<String> {
        let strategy = self.strategy.read().clone();

        match strategy {
            DistributionStrategy::TimeBucket(config) => {
                let mut current = start;
                let mut shards = HashSet::new();

                while current <= end {
                    let bucket_key = self.get_time_bucket_key(current, &config);
                    let hash = self.hash_string(&bucket_key);
                    let shard_names = self.get_shard_names();
                    let shard_idx = hash % shard_names.len();
                    shards.insert(shard_names[shard_idx].clone());

                    current = match config.bucket_size {
                        TimeBucketSize::Minutes(minutes) => {
                            current + chrono::Duration::minutes(minutes as i64)
                        }
                        TimeBucketSize::Hours(hours) => {
                            current + chrono::Duration::hours(hours as i64)
                        }
                        TimeBucketSize::Days(days) => current + chrono::Duration::days(days as i64),
                        TimeBucketSize::Weeks(weeks) => {
                            current + chrono::Duration::weeks(weeks as i64)
                        }
                        TimeBucketSize::Months(months) => {
                            // Simple month addition
                            let year = current.year();
                            let month = current.month();
                            let new_month = month + months;
                            let result = if new_month > 12 {
                                Utc.with_ymd_and_hms(year + 1, new_month - 12, 1, 0, 0, 0)
                            } else {
                                Utc.with_ymd_and_hms(year, new_month, 1, 0, 0, 0)
                            };

                            // Handle LocalResult properly
                            match result {
                                LocalResult::Single(dt) => dt,
                                LocalResult::Ambiguous(_, _) | LocalResult::None => {
                                    current + chrono::Duration::days(30)
                                }
                            }
                        }
                    };
                }

                shards.into_iter().collect()
            }
            _ => self.get_all_shard_names(),
        }
    }

    /// Helper: Get all shard names
    pub fn get_all_shard_names(&self) -> Vec<String> {
        let stats = self.shard_manager.shard_statistics();
        let mut names: Vec<String> = stats.shard_details.iter().map(|d| d.name.clone()).collect();
        names.sort();
        names
    }
}
