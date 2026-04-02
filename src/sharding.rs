use crate::concurrent::UnifiedConcurrentStore;
use crate::timeline::TelemetryQuery;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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
    pub db_path: PathBuf, // Changed from 'path' to 'db_path'
    pub strategy: ShardingStrategy,
    pub key_range: Option<(String, String)>,
    pub time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

/// Shard manager for distributed UnifiedConcurrentStore
pub struct ShardManager {
    shards: Vec<UnifiedConcurrentStore>,
    shard_configs: Vec<ShardConfig>,
    strategy: ShardingStrategy,
    key_index: Arc<RwLock<HashMap<String, usize>>>,
    time_index: Arc<RwLock<HashMap<String, Vec<usize>>>>,
    consistent_hash_ring: Arc<RwLock<HashMap<u64, usize>>>,
    virtual_nodes: usize,
}

impl ShardManager {
    pub fn new(
        shard_configs: Vec<ShardConfig>,
        strategy: ShardingStrategy,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut shards = Vec::new();

        // Ensure parent directory exists for each shard's database file
        for config in &shard_configs {
            if let Some(parent) = config.db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let store = UnifiedConcurrentStore::open(&config.db_path)?;
            shards.push(store);
        }

        let mut manager = ShardManager {
            shards,
            shard_configs,
            strategy,
            key_index: Arc::new(RwLock::new(HashMap::new())),
            time_index: Arc::new(RwLock::new(HashMap::new())),
            consistent_hash_ring: Arc::new(RwLock::new(HashMap::new())),
            virtual_nodes: 150,
        };

        if strategy == ShardingStrategy::ConsistentHash {
            manager.build_consistent_hash_ring();
        }

        Ok(manager)
    }

    pub fn get_shard_for_key(&self, key: &str) -> &UnifiedConcurrentStore {
        if let Some(shard_idx) = self.key_index.read().get(key) {
            return &self.shards[*shard_idx];
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

        self.key_index.write().insert(key.to_string(), shard_idx);
        &self.shards[shard_idx]
    }

    pub fn get_shards_for_time_interval(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Vec<&UnifiedConcurrentStore> {
        let bucket_key = self.time_bucket_key(start, end);

        if let Some(shard_indices) = self.time_index.read().get(&bucket_key) {
            return shard_indices.iter().map(|&idx| &self.shards[idx]).collect();
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

        self.time_index
            .write()
            .insert(bucket_key, shard_indices.clone());
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

            self.key_index.write().clear();
            self.time_index.write().clear();

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
}

impl ShardManagerBuilder {
    pub fn new() -> Self {
        ShardManagerBuilder {
            configs: Vec::new(),
            strategy: ShardingStrategy::KeyHash,
        }
    }

    pub fn with_strategy(mut self, strategy: ShardingStrategy) -> Self {
        self.strategy = strategy;
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
        ShardManager::new(self.configs, self.strategy)
    }
}

impl Default for ShardManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
