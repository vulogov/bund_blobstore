use crate::concurrent::UnifiedConcurrentStore;
use crate::timeline::TelemetryQuery;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Cache entry with expiration
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        CacheEntry {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// LRU Cache implementation
struct LRUCache<K, V> {
    cache: HashMap<K, CacheEntry<V>>,
    order: VecDeque<K>,
    capacity: usize,
    default_ttl: Duration,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> LRUCache<K, V> {
    fn new(capacity: usize, default_ttl: Duration) -> Self {
        LRUCache {
            cache: HashMap::new(),
            order: VecDeque::new(),
            capacity,
            default_ttl,
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.cache.get(key) {
            if entry.is_expired() {
                self.remove(key);
                return None;
            }

            // Move to front (most recently used)
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
                self.order.push_front(key.clone());
            }

            return Some(entry.value.clone());
        }
        None
    }

    fn put(&mut self, key: K, value: V, ttl: Option<Duration>) {
        let ttl = ttl.unwrap_or(self.default_ttl);

        // Remove if already exists
        if self.cache.contains_key(&key) {
            self.remove(&key);
        }

        // Evict oldest if at capacity
        while self.cache.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_back() {
                self.cache.remove(&oldest);
            }
        }

        self.order.push_front(key.clone());
        self.cache.insert(key, CacheEntry::new(value, ttl));
    }

    fn remove(&mut self, key: &K) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            self.order.remove(pos);
        }
        self.cache.remove(key);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.order.clear();
    }

    fn len(&self) -> usize {
        self.cache.len()
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
    pub capacity: usize,
    pub hit_rate: f64,
}

/// Cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub enabled: bool,
    pub max_size: usize,
    pub default_ttl: Duration,
    pub key_cache_ttl: Duration,
    pub time_cache_ttl: Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        CacheConfig {
            enabled: true,
            max_size: 10000,
            default_ttl: Duration::from_secs(300), // 5 minutes
            key_cache_ttl: Duration::from_secs(600), // 10 minutes
            time_cache_ttl: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Sharding strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardingStrategy {
    KeyHash,
    TimeRange,
    KeyPrefix,
    ConsistentHash,
}

/// Shard configuration
#[derive(Debug, Clone)]
pub struct ShardConfig {
    pub name: String,
    pub db_path: PathBuf,
    pub strategy: ShardingStrategy,
    pub key_range: Option<(String, String)>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

/// Shard manager for distributed UnifiedConcurrentStore with caching
pub struct ShardManager {
    shards: Vec<UnifiedConcurrentStore>,
    shard_configs: Vec<ShardConfig>,
    strategy: ShardingStrategy,
    key_cache: Arc<RwLock<LRUCache<String, usize>>>,
    time_cache: Arc<RwLock<LRUCache<String, Vec<usize>>>>,
    consistent_hash_ring: Arc<RwLock<HashMap<u64, usize>>>,
    virtual_nodes: usize,
    cache_config: CacheConfig,
    cache_hits: Arc<RwLock<u64>>,
    cache_misses: Arc<RwLock<u64>>,
}

impl ShardManager {
    pub fn new(
        shard_configs: Vec<ShardConfig>,
        strategy: ShardingStrategy,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::new_with_cache_config(shard_configs, strategy, CacheConfig::default())
    }

    pub fn new_with_cache_config(
        shard_configs: Vec<ShardConfig>,
        strategy: ShardingStrategy,
        cache_config: CacheConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut shards = Vec::new();

        for config in &shard_configs {
            // Create a unique directory for this shard
            let shard_dir = config.db_path.parent().unwrap_or(&config.db_path);
            std::fs::create_dir_all(shard_dir)?;

            // Each shard needs its own database file path
            let store = UnifiedConcurrentStore::open(&config.db_path)?;
            shards.push(store);
        }

        let key_cache = Arc::new(RwLock::new(LRUCache::new(
            cache_config.max_size,
            cache_config.key_cache_ttl,
        )));

        let time_cache = Arc::new(RwLock::new(LRUCache::new(
            cache_config.max_size,
            cache_config.time_cache_ttl,
        )));

        let mut manager = ShardManager {
            shards,
            shard_configs,
            strategy,
            key_cache,
            time_cache,
            consistent_hash_ring: Arc::new(RwLock::new(HashMap::new())),
            virtual_nodes: 150,
            cache_config,
            cache_hits: Arc::new(RwLock::new(0)),
            cache_misses: Arc::new(RwLock::new(0)),
        };

        if strategy == ShardingStrategy::ConsistentHash {
            manager.build_consistent_hash_ring();
        }

        Ok(manager)
    }

    pub fn get_shard_for_key(&self, key: &str) -> &UnifiedConcurrentStore {
        // Check cache first if enabled
        if self.cache_config.enabled {
            if let Some(shard_idx) = self.key_cache.write().get(&key.to_string()) {
                self.increment_hits();
                return &self.shards[shard_idx];
            }
            self.increment_misses();
        }

        let shard_idx = match self.strategy {
            ShardingStrategy::KeyHash => {
                let hash = self.hash_key(key);
                (hash % self.shards.len() as u64) as usize
            }
            ShardingStrategy::KeyPrefix => {
                let mut found_idx = 0;
                for (idx, config) in self.shard_configs.iter().enumerate() {
                    if let Some((start, end)) = &config.key_range {
                        // Compare string slices properly
                        if key >= start.as_str() && key <= end.as_str() {
                            found_idx = idx;
                            break;
                        }
                    }
                }
                found_idx
            }
            ShardingStrategy::ConsistentHash => {
                let hash = self.hash_key(key);
                self.find_consistent_hash_node(hash)
            }
            _ => 0,
        };

        // Cache the result if enabled
        if self.cache_config.enabled {
            self.key_cache.write().put(
                key.to_string(),
                shard_idx,
                Some(self.cache_config.key_cache_ttl),
            );
        }

        &self.shards[shard_idx]
    }

    pub fn get_shards_for_time_interval(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&UnifiedConcurrentStore> {
        let bucket_key = self.time_bucket_key(start, end);

        // Check cache first if enabled
        if self.cache_config.enabled {
            if let Some(shard_indices) = self.time_cache.write().get(&bucket_key) {
                self.increment_hits();
                return shard_indices.iter().map(|&idx| &self.shards[idx]).collect();
            }
            self.increment_misses();
        }

        let mut shard_indices = Vec::new();

        match self.strategy {
            ShardingStrategy::TimeRange => {
                for (idx, config) in self.shard_configs.iter().enumerate() {
                    if let Some((shard_start, shard_end)) = &config.time_range {
                        if start <= *shard_end && end >= *shard_start {
                            shard_indices.push(idx);
                        }
                    }
                }
            }
            _ => {
                shard_indices.extend(0..self.shards.len());
            }
        }

        // Cache the result if enabled
        if self.cache_config.enabled {
            self.time_cache.write().put(
                bucket_key,
                shard_indices.clone(),
                Some(self.cache_config.time_cache_ttl),
            );
        }

        shard_indices.iter().map(|&idx| &self.shards[idx]).collect()
    }

    pub fn write_to_all(
        &self,
        operation: &ShardOperation,
    ) -> Result<Vec<ShardResult>, Box<dyn std::error::Error + Send + Sync>> {
        let mut results = Vec::new();

        for shard in &self.shards {
            let result = match operation {
                ShardOperation::PutBlob { key, data, prefix } => {
                    shard.blob().put(key, data, *prefix)?;
                    ShardResult::Success
                }
                ShardOperation::StoreTelemetry { record } => {
                    shard.telemetry().store(record.clone())?;
                    ShardResult::Success
                }
                ShardOperation::IndexDocument { key, text, prefix } => {
                    shard.search().put_text(key, text, *prefix)?;
                    ShardResult::Success
                }
            };
            results.push(result);
        }

        Ok(results)
    }

    pub fn query_all_shards<T, F>(
        &self,
        mut query_fn: F,
    ) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>>
    where
        T: Clone + Send,
        F: FnMut(
            &UnifiedConcurrentStore,
        ) -> Result<Vec<T>, Box<dyn std::error::Error + Send + Sync>>,
    {
        let mut all_results = Vec::new();

        for shard in &self.shards {
            let results = query_fn(shard)?;
            all_results.extend(results);
        }

        Ok(all_results)
    }

    pub fn query_telemetry(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<crate::timeline::TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>>
    {
        let target_shards = if let Some(time_interval) = &query.time_interval {
            self.get_shards_for_time_interval(time_interval.start, time_interval.end)
        } else {
            self.shards.iter().collect()
        };

        let mut all_records = Vec::new();

        for shard in target_shards {
            let records = shard.telemetry().query(query)?;
            all_records.extend(records);
        }

        let start = query.offset.min(all_records.len());
        let end = (start + query.limit).min(all_records.len());

        Ok(all_records[start..end].to_vec())
    }

    pub fn add_shard(
        &mut self,
        config: ShardConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let store = UnifiedConcurrentStore::open(&config.db_path)?;
        self.shards.push(store);
        self.shard_configs.push(config);

        // Clear caches when shards change
        self.clear_caches();

        if self.strategy == ShardingStrategy::ConsistentHash {
            self.rebuild_consistent_hash_ring();
        }

        Ok(())
    }

    pub fn remove_shard(
        &mut self,
        shard_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(index) = self.shard_configs.iter().position(|c| c.name == shard_name) {
            self.shards.remove(index);
            self.shard_configs.remove(index);

            // Clear caches when shards change
            self.clear_caches();

            if self.strategy == ShardingStrategy::ConsistentHash {
                self.rebuild_consistent_hash_ring();
            }
        }

        Ok(())
    }

    pub fn shard_statistics(&self) -> ShardStatistics {
        let mut stats = ShardStatistics {
            total_shards: self.shards.len(),
            shard_details: Vec::new(),
            key_distribution: HashMap::new(),
        };

        for (idx, shard) in self.shards.iter().enumerate() {
            let key_count = shard.blob().len().unwrap_or(0);
            stats.shard_details.push(ShardDetail {
                name: self.shard_configs[idx].name.clone(),
                index: idx,
                key_count,
                strategy: self.strategy,
            });
        }

        stats
    }

    /// Get cache statistics
    pub fn cache_statistics(&self) -> CacheStats {
        let hits = *self.cache_hits.read();
        let misses = *self.cache_misses.read();
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        CacheStats {
            hits,
            misses,
            size: self.key_cache.read().len() + self.time_cache.read().len(),
            capacity: self.cache_config.max_size,
            hit_rate,
        }
    }

    /// Clear all caches
    pub fn clear_caches(&self) {
        self.key_cache.write().clear();
        self.time_cache.write().clear();
        *self.cache_hits.write() = 0;
        *self.cache_misses.write() = 0;
    }

    /// Update cache configuration
    pub fn update_cache_config(&mut self, config: CacheConfig) {
        self.cache_config = config;
        if !self.cache_config.enabled {
            self.clear_caches();
        }
    }

    /// Preload cache with common keys
    pub fn preload_cache(&self, keys: &[String]) {
        if !self.cache_config.enabled {
            return;
        }

        for key in keys {
            let shard_idx = match self.strategy {
                ShardingStrategy::KeyHash => {
                    let hash = self.hash_key(key);
                    (hash % self.shards.len() as u64) as usize
                }
                ShardingStrategy::KeyPrefix => {
                    let mut found_idx = 0;
                    for (idx, config) in self.shard_configs.iter().enumerate() {
                        if let Some((start, end)) = &config.key_range {
                            // Compare string slices properly
                            if key.as_str() >= start.as_str() && key.as_str() <= end.as_str() {
                                found_idx = idx;
                                break;
                            }
                        }
                    }
                    found_idx
                }
                ShardingStrategy::ConsistentHash => {
                    let hash = self.hash_key(key);
                    self.find_consistent_hash_node(hash)
                }
                _ => 0,
            };

            self.key_cache.write().put(
                key.clone(),
                shard_idx,
                Some(self.cache_config.key_cache_ttl),
            );
        }
    }

    fn increment_hits(&self) {
        *self.cache_hits.write() += 1;
    }

    fn increment_misses(&self) {
        *self.cache_misses.write() += 1;
    }

    fn hash_key(&self, key: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    fn build_consistent_hash_ring(&mut self) {
        let mut ring = HashMap::new();

        for (node_idx, _shard) in self.shards.iter().enumerate() {
            for vnode in 0..self.virtual_nodes {
                let key = format!("{}:vnode{}", self.shard_configs[node_idx].name, vnode);
                let hash = self.hash_key(&key);
                ring.insert(hash, node_idx);
            }
        }

        *self.consistent_hash_ring.write() = ring;
    }

    fn rebuild_consistent_hash_ring(&mut self) {
        self.build_consistent_hash_ring();
    }

    fn find_consistent_hash_node(&self, hash: u64) -> usize {
        let ring = self.consistent_hash_ring.read();

        let mut min_hash = u64::MAX;
        let mut selected_node = 0;

        for (&node_hash, &node_idx) in ring.iter() {
            if node_hash >= hash && node_hash < min_hash {
                min_hash = node_hash;
                selected_node = node_idx;
            }
        }

        if min_hash == u64::MAX {
            if let Some((_first_hash, first_node)) = ring.iter().min_by_key(|(h, _)| *h) {
                return *first_node;
            }
        }

        selected_node
    }

    fn time_bucket_key(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> String {
        format!("{}:{}", start.timestamp(), end.timestamp())
    }
}

#[derive(Debug)]
pub enum ShardOperation<'a> {
    PutBlob {
        key: &'a str,
        data: &'a [u8],
        prefix: Option<&'a str>,
    },
    StoreTelemetry {
        record: crate::timeline::TelemetryRecord,
    },
    IndexDocument {
        key: &'a str,
        text: &'a str,
        prefix: Option<&'a str>,
    },
}

#[derive(Debug)]
pub enum ShardResult {
    Success,
    Failure(String),
}

#[derive(Debug, Clone)]
pub struct ShardStatistics {
    pub total_shards: usize,
    pub shard_details: Vec<ShardDetail>,
    pub key_distribution: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct ShardDetail {
    pub name: String,
    pub index: usize,
    pub key_count: usize,
    pub strategy: ShardingStrategy,
}

#[derive(Debug, Clone)]
pub struct ShardAllocation {
    pub shard_name: String,
    pub shard_index: usize,
    pub allocation_type: AllocationType,
}

#[derive(Debug, Clone)]
pub enum AllocationType {
    KeyHash { hash: u64, total_shards: usize },
    KeyPrefix { prefix: String },
    TimeRange { bucket: String },
    ConsistentHash { node: String, vnode: u64 },
}

/// Builder for creating sharded stores
pub struct ShardManagerBuilder {
    configs: Vec<ShardConfig>,
    strategy: ShardingStrategy,
    cache_config: CacheConfig,
}

impl ShardManagerBuilder {
    pub fn new() -> Self {
        ShardManagerBuilder {
            configs: Vec::new(),
            strategy: ShardingStrategy::KeyHash,
            cache_config: CacheConfig::default(),
        }
    }

    pub fn with_strategy(mut self, strategy: ShardingStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn with_cache_config(mut self, cache_config: CacheConfig) -> Self {
        self.cache_config = cache_config;
        self
    }

    pub fn disable_cache(mut self) -> Self {
        self.cache_config.enabled = false;
        self
    }

    pub fn set_cache_ttl(mut self, ttl_seconds: u64) -> Self {
        self.cache_config.default_ttl = Duration::from_secs(ttl_seconds);
        self.cache_config.key_cache_ttl = Duration::from_secs(ttl_seconds);
        self.cache_config.time_cache_ttl = Duration::from_secs(ttl_seconds);
        self
    }

    pub fn add_shard(mut self, name: &str, db_path: &str) -> Self {
        self.configs.push(ShardConfig {
            name: name.to_string(),
            db_path: PathBuf::from(db_path),
            strategy: self.strategy,
            key_range: None,
            time_range: None,
        });
        self
    }

    pub fn add_key_range_shard(
        mut self,
        name: &str,
        db_path: &str,
        start: &str,
        end: &str,
    ) -> Self {
        self.configs.push(ShardConfig {
            name: name.to_string(),
            db_path: PathBuf::from(db_path),
            strategy: ShardingStrategy::KeyPrefix,
            key_range: Some((start.to_string(), end.to_string())),
            time_range: None,
        });
        self
    }

    pub fn add_time_range_shard(
        mut self,
        name: &str,
        db_path: &str,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Self {
        self.configs.push(ShardConfig {
            name: name.to_string(),
            db_path: PathBuf::from(db_path),
            strategy: ShardingStrategy::TimeRange,
            key_range: None,
            time_range: Some((start, end)),
        });
        self
    }

    pub fn build(self) -> Result<ShardManager, Box<dyn std::error::Error + Send + Sync>> {
        ShardManager::new_with_cache_config(self.configs, self.strategy, self.cache_config)
    }
}

impl Default for ShardManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
