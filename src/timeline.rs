use crate::blobstore::BlobStore;
use crate::serialization::{SerializationFormat, SerializationHelper};
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use parking_lot::RwLock;
use rust_dynamic::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

// Helper function for serializing DateTime to timestamp
pub fn timestamp_to_seconds(ts: &DateTime<Utc>) -> i64 {
    ts.timestamp()
}

pub fn seconds_to_timestamp(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).unwrap()
}

/// Telemetry value types - supports mixed types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TelemetryValue {
    Float(f64),
    Int(i64),
    String(String),
    Bool(bool),
    Blob(Vec<u8>),
    Json(serde_json::Value),
    Dynamic(Value),
    Null,
}

impl TelemetryValue {
    pub fn as_float(&self) -> Option<f64> {
        match self {
            TelemetryValue::Float(v) => Some(*v),
            TelemetryValue::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            TelemetryValue::Int(v) => Some(*v),
            TelemetryValue::Float(v) => Some(*v as i64),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<String> {
        match self {
            TelemetryValue::String(v) => Some(v.clone()),
            TelemetryValue::Json(v) => Some(v.to_string()),
            _ => None,
        }
    }
}

/// Telemetry record - primary or secondary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryRecord {
    pub id: String,
    pub timestamp_seconds: i64, // Store as seconds for serialization
    pub key: String,
    pub source: String,
    pub value: TelemetryValue,
    pub metadata: HashMap<String, String>,
    pub is_primary: bool,
    pub primary_id: Option<String>,
    pub secondary_ids: Vec<String>,
}

impl TelemetryRecord {
    pub fn new_primary(
        id: String,
        timestamp: DateTime<Utc>,
        key: String,
        source: String,
        value: TelemetryValue,
    ) -> Self {
        TelemetryRecord {
            id,
            timestamp_seconds: timestamp.timestamp(),
            key,
            source,
            value,
            metadata: HashMap::new(),
            is_primary: true,
            primary_id: None,
            secondary_ids: Vec::new(),
        }
    }

    pub fn new_secondary(
        id: String,
        timestamp: DateTime<Utc>,
        key: String,
        source: String,
        value: TelemetryValue,
        primary_id: String,
    ) -> Self {
        TelemetryRecord {
            id,
            timestamp_seconds: timestamp.timestamp(),
            key,
            source,
            value,
            metadata: HashMap::new(),
            is_primary: false,
            primary_id: Some(primary_id),
            secondary_ids: Vec::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.timestamp_seconds, 0).unwrap()
    }
}

/// Time interval for queries
#[derive(Debug, Clone)]
pub struct TimeInterval {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeInterval {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        TimeInterval { start, end }
    }

    pub fn last_hour() -> Self {
        let now = Utc::now();
        TimeInterval {
            start: now - Duration::hours(1),
            end: now,
        }
    }

    pub fn last_day() -> Self {
        let now = Utc::now();
        TimeInterval {
            start: now - Duration::days(1),
            end: now,
        }
    }

    pub fn last_week() -> Self {
        let now = Utc::now();
        TimeInterval {
            start: now - Duration::days(7),
            end: now,
        }
    }

    pub fn last_month() -> Self {
        let now = Utc::now();
        TimeInterval {
            start: now - Duration::days(30),
            end: now,
        }
    }
}

/// Minute-grade interval bucket
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MinuteBucket {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl MinuteBucket {
    pub fn from_timestamp(ts: DateTime<Utc>) -> Self {
        MinuteBucket {
            year: ts.year(),
            month: ts.month(),
            day: ts.day(),
            hour: ts.hour(),
            minute: ts.minute(),
        }
    }

    pub fn to_timestamp(&self) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(self.year, self.month, self.day, self.hour, self.minute, 0)
            .unwrap()
    }

    pub fn next(&self) -> Self {
        let ts = self.to_timestamp();
        MinuteBucket::from_timestamp(ts + Duration::minutes(1))
    }

    pub fn prev(&self) -> Self {
        let ts = self.to_timestamp();
        MinuteBucket::from_timestamp(ts - Duration::minutes(1))
    }
}

/// Telemetry query options
#[derive(Debug, Clone)]
pub struct TelemetryQuery {
    pub time_interval: Option<TimeInterval>,
    pub keys: Option<Vec<String>>,
    pub sources: Option<Vec<String>>,
    pub primary_only: bool,
    pub secondary_only: bool,
    pub primary_id: Option<String>,
    pub value_type: Option<String>,
    pub limit: usize,
    pub offset: usize,
    pub bucket_by_minute: bool,
}

impl Default for TelemetryQuery {
    fn default() -> Self {
        TelemetryQuery {
            time_interval: None,
            keys: None,
            sources: None,
            primary_only: false,
            secondary_only: false,
            primary_id: None,
            value_type: None,
            limit: 100,
            offset: 0,
            bucket_by_minute: false,
        }
    }
}

/// Aggregated telemetry result
#[derive(Debug, Clone)]
pub struct AggregatedTelemetry {
    pub bucket: MinuteBucket,
    pub count: usize,
    pub avg_value: Option<f64>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub sum_value: Option<f64>,
    pub records: Vec<TelemetryRecord>,
}

/// Telemetry timeline store
pub struct TelemetryStore {
    store: BlobStore,
    primary_index: Arc<RwLock<HashMap<String, String>>>,
    secondary_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    time_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    key_source_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    serializer_format: SerializationFormat,
}

impl TelemetryStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = BlobStore::open(path)?;

        let mut telemetry = TelemetryStore {
            store,
            primary_index: Arc::new(RwLock::new(HashMap::new())),
            secondary_index: Arc::new(RwLock::new(HashMap::new())),
            time_index: Arc::new(RwLock::new(HashMap::new())),
            key_source_index: Arc::new(RwLock::new(HashMap::new())),
            serializer_format: SerializationFormat::Bincode,
        };

        telemetry.load_indices()?;
        Ok(telemetry)
    }
    /// Create a telemetry store from an existing blob store
    pub fn open_with_store(
        store: BlobStore,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut telemetry = TelemetryStore {
            store,
            primary_index: Arc::new(RwLock::new(HashMap::new())),
            secondary_index: Arc::new(RwLock::new(HashMap::new())),
            time_index: Arc::new(RwLock::new(HashMap::new())),
            key_source_index: Arc::new(RwLock::new(HashMap::new())),
            serializer_format: SerializationFormat::Bincode,
        };

        telemetry.load_indices()?;
        Ok(telemetry)
    }
    pub fn store(
        &mut self,
        record: TelemetryRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("telemetry:{}", record.id);
        let serialized = SerializationHelper::serialize(&record, self.serializer_format)?;
        self.store.put(&key, &serialized, Some("telemetry"))?;

        self.update_indices(&record)?;

        Ok(())
    }

    pub fn link_primary_secondary(
        &mut self,
        primary_id: &str,
        secondary_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(mut primary) = self.get_record(primary_id)? {
            if !primary.secondary_ids.contains(&secondary_id.to_string()) {
                primary.secondary_ids.push(secondary_id.to_string());
                self.store(primary)?;
            }
        }

        if let Some(mut secondary) = self.get_record(secondary_id)? {
            secondary.primary_id = Some(primary_id.to_string());
            secondary.is_primary = false;
            self.store(secondary)?;
        }

        self.secondary_index
            .write()
            .entry(primary_id.to_string())
            .or_insert_with(Vec::new)
            .push(secondary_id.to_string());

        Ok(())
    }

    pub fn get_record(
        &self,
        id: &str,
    ) -> Result<Option<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("telemetry:{}", id);
        if let Some(data) = self.store.get(&key)? {
            let record: TelemetryRecord =
                SerializationHelper::deserialize(&data, self.serializer_format)?;
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    pub fn query(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let mut records = Vec::new();

        if let Some(time_interval) = &query.time_interval {
            // Get all records and filter by timestamp
            let all_data = self.store.get_all()?;
            for key in all_data.keys() {
                if key.starts_with("telemetry:") {
                    if let Some(record) = self.get_record(&key.replace("telemetry:", ""))? {
                        let ts = record.timestamp();
                        if ts >= time_interval.start && ts <= time_interval.end {
                            records.push(record);
                        }
                    }
                }
            }
        } else {
            // No time filter - get all records
            let all_data = self.store.get_all()?;
            for key in all_data.keys() {
                if key.starts_with("telemetry:") {
                    if let Some(record) = self.get_record(&key.replace("telemetry:", ""))? {
                        records.push(record);
                    }
                }
            }
        }

        // Apply key filter
        if let Some(keys) = &query.keys {
            records.retain(|r| keys.iter().any(|k| r.key.contains(k)));
        }

        // Apply source filter
        if let Some(sources) = &query.sources {
            records.retain(|r| sources.iter().any(|s| r.source.contains(s)));
        }

        // Apply primary/secondary filters
        if query.primary_only {
            records.retain(|r| r.is_primary);
        } else if query.secondary_only {
            records.retain(|r| !r.is_primary);
        }

        if let Some(ref primary_id) = query.primary_id {
            records.retain(|r| r.primary_id.as_ref() == Some(primary_id));
        }

        // Apply value type filter
        if let Some(value_type) = &query.value_type {
            records.retain(|r| match value_type.as_str() {
                "float" => matches!(r.value, TelemetryValue::Float(_)),
                "int" => matches!(r.value, TelemetryValue::Int(_)),
                "string" => matches!(r.value, TelemetryValue::String(_)),
                "bool" => matches!(r.value, TelemetryValue::Bool(_)),
                "json" => matches!(r.value, TelemetryValue::Json(_)),
                _ => true,
            });
        }

        // Apply limit and offset
        let start = query.offset.min(records.len());
        let end = (start + query.limit).min(records.len());
        let result = records[start..end].to_vec();

        Ok(result)
    }

    pub fn query_bucketed(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<AggregatedTelemetry>, Box<dyn std::error::Error + Send + Sync>> {
        let records = self.query(query)?;
        let mut buckets: HashMap<MinuteBucket, Vec<TelemetryRecord>> = HashMap::new();

        for record in records {
            let bucket = MinuteBucket::from_timestamp(record.timestamp());
            buckets.entry(bucket).or_insert_with(Vec::new).push(record);
        }

        let mut aggregated: Vec<AggregatedTelemetry> = buckets
            .into_iter()
            .map(|(bucket, records)| {
                let mut agg = AggregatedTelemetry {
                    bucket,
                    count: records.len(),
                    avg_value: None,
                    min_value: None,
                    max_value: None,
                    sum_value: None,
                    records,
                };

                let numeric_values: Vec<f64> = agg
                    .records
                    .iter()
                    .filter_map(|r| r.value.as_float())
                    .collect();

                if !numeric_values.is_empty() {
                    agg.sum_value = Some(numeric_values.iter().sum());
                    agg.avg_value = Some(agg.sum_value.unwrap() / numeric_values.len() as f64);
                    agg.min_value =
                        Some(numeric_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)));
                    agg.max_value = Some(
                        numeric_values
                            .iter()
                            .fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
                    );
                }

                agg
            })
            .collect();

        aggregated.sort_by(|a, b| a.bucket.to_timestamp().cmp(&b.bucket.to_timestamp()));
        Ok(aggregated)
    }

    pub fn get_secondaries(
        &self,
        primary_id: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let secondary_ids = self
            .secondary_index
            .read()
            .get(primary_id)
            .cloned()
            .unwrap_or_default();

        let mut secondaries = Vec::new();
        for id in secondary_ids {
            if let Some(record) = self.get_record(&id)? {
                secondaries.push(record);
            }
        }

        Ok(secondaries)
    }

    pub fn get_primary(
        &self,
        secondary_id: &str,
    ) -> Result<Option<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(record) = self.get_record(secondary_id)? {
            if let Some(primary_id) = record.primary_id {
                return self.get_record(&primary_id);
            }
        }
        Ok(None)
    }

    pub fn search_by_key(
        &self,
        key_pattern: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let query = TelemetryQuery {
            keys: Some(vec![key_pattern.to_string()]),
            ..Default::default()
        };
        self.query(&query)
    }

    pub fn search_by_source(
        &self,
        source: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let query = TelemetryQuery {
            sources: Some(vec![source.to_string()]),
            ..Default::default()
        };
        self.query(&query)
    }

    pub fn get_time_range(
        &self,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, Box<dyn std::error::Error + Send + Sync>>
    {
        let mut min_time = None;
        let mut max_time = None;

        let all_data = self.store.get_all()?;
        for key in all_data.keys() {
            if key.starts_with("telemetry:") {
                if let Some(record) = self.get_record(&key.replace("telemetry:", ""))? {
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

        match (min_time, max_time) {
            (Some(min), Some(max)) => Ok(Some((min, max))),
            _ => Ok(None),
        }
    }

    fn update_indices(
        &mut self,
        record: &TelemetryRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let bucket = MinuteBucket::from_timestamp(record.timestamp());
        let bucket_key = self.bucket_key(&bucket);
        self.time_index
            .write()
            .entry(bucket_key)
            .or_insert_with(Vec::new)
            .push(record.id.clone());

        let key_source_key = format!("{}:{}", record.key, record.source);
        self.key_source_index
            .write()
            .entry(key_source_key)
            .or_insert_with(Vec::new)
            .push(record.id.clone());

        if record.is_primary {
            self.primary_index
                .write()
                .insert(record.id.clone(), record.id.clone());
        }

        Ok(())
    }

    fn load_indices(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let all_data = self.store.get_all()?;
        for key in all_data.keys() {
            if key.starts_with("telemetry:") {
                if let Some(record) = self.get_record(&key.replace("telemetry:", ""))? {
                    self.update_indices(&record)?;
                }
            }
        }
        Ok(())
    }

    fn bucket_key(&self, bucket: &MinuteBucket) -> String {
        format!(
            "{:04}-{:02}-{:02}_{:02}:{:02}",
            bucket.year, bucket.month, bucket.day, bucket.hour, bucket.minute
        )
    }
}
