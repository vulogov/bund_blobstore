use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::DistributionStrategy;
use crate::data_distribution::DataDistributionManager;
use crate::serialization::{SerializationFormat, SerializationHelper};
use crate::timeline::TelemetryValue;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetrySample {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub value: TelemetryValue,
    pub metadata: HashMap<String, String>,
}

impl TelemetrySample {
    pub fn new(value: TelemetryValue) -> Self {
        TelemetrySample {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            value,
            metadata: HashMap::new(),
        }
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    pub fn with_metadata(mut self, key: &str, val: &str) -> Self {
        self.metadata.insert(key.to_string(), val.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleIdQueue {
    capacity: usize,
    ids: VecDeque<String>,
}

impl SampleIdQueue {
    pub fn new(capacity: usize) -> Self {
        SampleIdQueue {
            capacity,
            ids: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, sample_id: String) -> Option<String> {
        let evicted = if self.ids.len() == self.capacity {
            self.ids.pop_front()
        } else {
            None
        };
        self.ids.push_back(sample_id);
        evicted
    }

    pub fn get_latest_ids(&self, n: usize) -> Vec<String> {
        let take = n.min(self.ids.len());
        self.ids.iter().rev().take(take).cloned().collect()
    }

    pub fn get_all_ids(&self) -> Vec<String> {
        self.ids.clone().into_iter().collect()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DimensionType {
    OneD,
    TwoD,
    ThreeD,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bounds {
    pub min_x: i64,
    pub max_x: i64,
    pub min_y: Option<i64>,
    pub max_y: Option<i64>,
    pub min_z: Option<i64>,
    pub max_z: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionMetadata {
    pub label: String,
    pub dim_type: DimensionType,
    pub cell_capacity: usize,
    pub bounds: Option<Bounds>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Coord1D(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Coord2D(pub i64, pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Coord3D(pub i64, pub i64, pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Coordinate {
    OneD(Coord1D),
    TwoD(Coord2D),
    ThreeD(Coord3D),
}

impl Coordinate {
    fn to_key_string(&self) -> String {
        match self {
            Coordinate::OneD(c) => format!("{}", c.0),
            Coordinate::TwoD(c) => format!("{}_{}", c.0, c.1),
            Coordinate::ThreeD(c) => format!("{}_{}_{}", c.0, c.1, c.2),
        }
    }
}

pub struct MultidimensionalStorage {
    dist_manager: Arc<DataDistributionManager>,
    metadata_cache: Arc<RwLock<HashMap<String, DimensionMetadata>>>,
    serializer_format: SerializationFormat,
}

impl MultidimensionalStorage {
    pub fn open<P: AsRef<Path>>(
        base_path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let dist_manager = Arc::new(DataDistributionManager::new(
            base_path,
            DistributionStrategy::RoundRobin,
        )?);

        let storage = MultidimensionalStorage {
            dist_manager,
            metadata_cache: Arc::new(RwLock::new(HashMap::new())),
            serializer_format: SerializationFormat::MessagePack, // Use MessagePack instead of Bincode
        };
        storage.load_metadata()?;
        Ok(storage)
    }

    pub fn create_dimension(
        &self,
        label: &str,
        dim_type: DimensionType,
        cell_capacity: usize,
        bounds: Option<Bounds>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.metadata_cache.read().contains_key(label) {
            return Err(format!("Dimension '{}' already exists", label).into());
        }

        let metadata = DimensionMetadata {
            label: label.to_string(),
            dim_type,
            cell_capacity,
            bounds,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let meta_key = format!("md_meta:{}", label);
        let serialized = SerializationHelper::serialize(&metadata, self.serializer_format)?;
        self.dist_manager.put(&meta_key, &serialized, None)?;

        // Index label for vector search
        let label_key = format!("dim_label:{}", label);
        let label_text = label.to_string();
        let _ = self.dist_manager.put_vector_text(&label_key, &label_text);

        self.metadata_cache
            .write()
            .insert(label.to_string(), metadata);
        Ok(())
    }

    pub fn push_sample(
        &self,
        dimension_label: &str,
        coord: Coordinate,
        value: TelemetryValue,
        timestamp: Option<DateTime<Utc>>,
        metadata: HashMap<String, String>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let dim_meta = self.get_metadata(dimension_label)?;

        // Validate coordinate matches dimension type
        match (&dim_meta.dim_type, &coord) {
            (DimensionType::OneD, Coordinate::OneD(_)) => {}
            (DimensionType::TwoD, Coordinate::TwoD(_)) => {}
            (DimensionType::ThreeD, Coordinate::ThreeD(_)) => {}
            _ => return Err("Coordinate type does not match dimension".into()),
        }

        let sample = TelemetrySample {
            id: Uuid::new_v4().to_string(),
            timestamp: timestamp.unwrap_or_else(Utc::now),
            value,
            metadata,
        };

        // Store sample using DataDistributionManager (preserves sharding)
        let sample_key = format!("sample:{}", sample.id);
        let sample_data = SerializationHelper::serialize(&sample, self.serializer_format)?;
        self.dist_manager.put(&sample_key, &sample_data, None)?;

        // Update cell queue
        let cell_key = format!("cell:{}:{}", dimension_label, coord.to_key_string());
        let mut queue = self.load_queue(&cell_key, dim_meta.cell_capacity)?;
        let evicted = queue.push(sample.id.clone());

        let queue_data = SerializationHelper::serialize(&queue, self.serializer_format)?;
        self.dist_manager.put(&cell_key, &queue_data, None)?;

        // Clean up evicted sample
        if let Some(evicted_id) = evicted {
            let evicted_key = format!("sample:{}", evicted_id);
            let _ = self.dist_manager.delete(&evicted_key);
        }

        Ok(sample.id)
    }

    pub fn get_latest_samples(
        &self,
        dimension_label: &str,
        coord: Coordinate,
        count: usize,
    ) -> Result<Vec<TelemetrySample>, Box<dyn std::error::Error + Send + Sync>> {
        let dim_meta = self.get_metadata(dimension_label)?;
        let cell_key = format!("cell:{}:{}", dimension_label, coord.to_key_string());
        let queue = self.load_queue(&cell_key, dim_meta.cell_capacity)?;

        let sample_ids = queue.get_latest_ids(count);
        self.load_samples(&sample_ids)
    }

    pub fn get_samples_in_time_range(
        &self,
        dimension_label: &str,
        coord: Coordinate,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<Vec<TelemetrySample>, Box<dyn std::error::Error + Send + Sync>> {
        let dim_meta = self.get_metadata(dimension_label)?;
        let cell_key = format!("cell:{}:{}", dimension_label, coord.to_key_string());
        let queue = self.load_queue(&cell_key, dim_meta.cell_capacity)?;

        let sample_ids = queue.get_all_ids();
        let samples = self.load_samples(&sample_ids)?;
        let filtered: Vec<_> = samples
            .into_iter()
            .filter(|s| s.timestamp >= start && s.timestamp <= end)
            .collect();
        Ok(filtered)
    }

    pub fn search_dimensions_by_label(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<(String, f32)>, Box<dyn std::error::Error + Send + Sync>> {
        let results = self.dist_manager.vector_search(query, limit)?;
        let mut out = Vec::new();
        for res in results {
            if res.key.starts_with("dim_label:") {
                let label = res.key.replace("dim_label:", "");
                out.push((label, res.score));
            }
        }
        Ok(out)
    }

    pub fn list_dimensions(&self) -> Vec<DimensionMetadata> {
        self.metadata_cache.read().values().cloned().collect()
    }

    pub fn get_metadata(
        &self,
        label: &str,
    ) -> Result<DimensionMetadata, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(meta) = self.metadata_cache.read().get(label) {
            Ok(meta.clone())
        } else {
            Err(format!("Dimension '{}' not found", label).into())
        }
    }

    pub fn delete_dimension(
        &self,
        label: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let meta_key = format!("md_meta:{}", label);
        let _ = self.dist_manager.delete(&meta_key);

        let prefix = format!("cell:{}:", label);
        let all_keys = self.dist_manager.list_keys(None)?;
        for key in all_keys {
            if key.starts_with(&prefix) {
                let _ = self.dist_manager.delete(&key);
            }
        }

        let label_key = format!("dim_label:{}", label);
        let _ = self.dist_manager.delete(&label_key);

        self.metadata_cache.write().remove(label);
        Ok(())
    }

    fn load_queue(
        &self,
        key: &str,
        capacity: usize,
    ) -> Result<SampleIdQueue, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(data) = self.dist_manager.get(key)? {
            let queue: SampleIdQueue =
                SerializationHelper::deserialize(&data, self.serializer_format)?;
            Ok(queue)
        } else {
            Ok(SampleIdQueue::new(capacity))
        }
    }

    fn load_samples(
        &self,
        ids: &[String],
    ) -> Result<Vec<TelemetrySample>, Box<dyn std::error::Error + Send + Sync>> {
        let mut samples = Vec::with_capacity(ids.len());
        for id in ids {
            let key = format!("sample:{}", id);
            if let Some(data) = self.dist_manager.get(&key)? {
                let sample: TelemetrySample =
                    SerializationHelper::deserialize(&data, self.serializer_format)?;
                samples.push(sample);
            }
        }
        Ok(samples)
    }

    fn load_metadata(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let all_keys = self.dist_manager.list_keys(None)?;
        let mut cache = self.metadata_cache.write();
        for key in all_keys {
            if key.starts_with("md_meta:") {
                if let Some(data) = self.dist_manager.get(&key)? {
                    let meta: DimensionMetadata =
                        SerializationHelper::deserialize(&data, self.serializer_format)?;
                    cache.insert(meta.label.clone(), meta);
                }
            }
        }
        Ok(())
    }
}
