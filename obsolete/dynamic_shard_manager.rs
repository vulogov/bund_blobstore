use crate::concurrent::UnifiedConcurrentStore;
use crate::sharding::ShardConfig;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A dynamic shard manager that supports adding/removing shards at runtime
pub struct DynamicShardManager {
    shards: Arc<RwLock<Vec<UnifiedConcurrentStore>>>,
    shard_configs: Arc<RwLock<Vec<ShardConfig>>>,
    shard_index: Arc<RwLock<HashMap<String, usize>>>,
}

impl DynamicShardManager {
    /// Create a new dynamic shard manager
    pub fn new() -> Self {
        DynamicShardManager {
            shards: Arc::new(RwLock::new(Vec::new())),
            shard_configs: Arc::new(RwLock::new(Vec::new())),
            shard_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a new shard
    pub fn add_shard(
        &self,
        config: ShardConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create directory if needed
        if let Some(parent) = config.db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Open the store
        let store = UnifiedConcurrentStore::open(&config.db_path)?;

        // Add to collections
        let mut shards = self.shards.write();
        let mut configs = self.shard_configs.write();
        let mut index = self.shard_index.write();

        let shard_idx = shards.len();
        shards.push(store);
        configs.push(config.clone());
        index.insert(config.name, shard_idx);

        Ok(())
    }

    /// Remove a shard by name
    pub fn remove_shard(
        &self,
        shard_name: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let mut index = self.shard_index.write();

        if let Some(&shard_idx) = index.get(shard_name) {
            let mut shards = self.shards.write();
            let mut configs = self.shard_configs.write();

            shards.remove(shard_idx);
            configs.remove(shard_idx);
            index.remove(shard_name);

            // Update indices for shards after the removed one
            for (name, idx) in index.iter_mut() {
                if *idx > shard_idx {
                    *idx -= 1;
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get a shard by index (round-robin)
    pub fn get_shard_by_index(&self, index: usize) -> Option<UnifiedConcurrentStore> {
        let shards = self.shards.read();
        shards.get(index % shards.len()).cloned()
    }

    /// Get a shard by name
    pub fn get_shard_by_name(&self, name: &str) -> Option<UnifiedConcurrentStore> {
        let index = self.shard_index.read();
        if let Some(&idx) = index.get(name) {
            let shards = self.shards.read();
            return shards.get(idx).cloned();
        }
        None
    }

    /// Get all shard names
    pub fn get_shard_names(&self) -> Vec<String> {
        let configs = self.shard_configs.read();
        configs.iter().map(|c| c.name.clone()).collect()
    }

    /// Get number of shards
    pub fn shard_count(&self) -> usize {
        self.shards.read().len()
    }

    /// Get shard details
    pub fn get_shard_details(&self) -> Vec<ShardDetail> {
        let shards = self.shards.read();
        let configs = self.shard_configs.read();

        shards
            .iter()
            .zip(configs.iter())
            .enumerate()
            .map(|(idx, (shard, config))| {
                let key_count = shard.blob().len().unwrap_or(0);
                ShardDetail {
                    name: config.name.clone(),
                    index: idx,
                    key_count,
                    strategy: crate::sharding::ShardingStrategy::ConsistentHash,
                }
            })
            .collect()
    }

    /// Get all shards as a vector (for iteration)
    pub fn get_all_shards(&self) -> Vec<UnifiedConcurrentStore> {
        self.shards.read().clone()
    }
}

#[derive(Debug, Clone)]
pub struct ShardDetail {
    pub name: String,
    pub index: usize,
    pub key_count: usize,
    pub strategy: crate::sharding::ShardingStrategy,
}

impl Default for DynamicShardManager {
    fn default() -> Self {
        Self::new()
    }
}
