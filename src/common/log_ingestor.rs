// src/common/log_ingestor.rs - Fixed version

use crate::common::grok_integration::GrokLogParser;
use crate::data_distribution::DataDistributionManager;
use crate::timeline::{TelemetryRecord, TelemetryValue};
use chrono::Utc;
use fastembed::EmbeddingModel;
use murmurhash3::murmurhash3_x64_128;
use parking_lot::RwLock;
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub type Result<T> = std::result::Result<T, String>;

/// Efficient deduplication using Bloom filter
struct BloomFilter {
    bits: Vec<u64>,
    num_hash_functions: usize,
    size: usize,
}

impl BloomFilter {
    fn new(expected_items: usize, false_positive_rate: f64) -> Self {
        let size = Self::optimal_size(expected_items, false_positive_rate);
        let num_hash_functions = Self::optimal_hash_count(expected_items, size);

        Self {
            bits: vec![0; (size + 63) / 64],
            num_hash_functions,
            size,
        }
    }

    fn optimal_size(n: usize, p: f64) -> usize {
        let m = -((n as f64) * p.ln()) / (2.0_f64.ln().powi(2));
        m.ceil() as usize
    }

    fn optimal_hash_count(n: usize, m: usize) -> usize {
        let k = ((m as f64) / (n as f64)) * 2.0_f64.ln();
        k.ceil() as usize
    }

    fn hash(&self, item: &str, seed: u32) -> usize {
        let (hash1, _hash2) = murmurhash3_x64_128(item.as_bytes(), seed as u64);
        (hash1 % self.size as u64) as usize
    }

    fn insert(&mut self, item: &str) {
        for i in 0..self.num_hash_functions {
            let index = self.hash(item, i as u32);
            let word_index = index / 64;
            let bit_index = index % 64;
            self.bits[word_index] |= 1u64 << bit_index;
        }
    }

    fn contains(&self, item: &str) -> bool {
        for i in 0..self.num_hash_functions {
            let index = self.hash(item, i as u32);
            let word_index = index / 64;
            let bit_index = index % 64;
            if (self.bits[word_index] & (1u64 << bit_index)) == 0 {
                return false;
            }
        }
        true
    }
}

/// Similarity threshold configuration
#[derive(Debug, Clone)]
pub struct SimilarityConfig {
    pub cosine_similarity_threshold: f32,
    pub use_cosine_similarity: bool,
}

impl Default for SimilarityConfig {
    fn default() -> Self {
        Self {
            cosine_similarity_threshold: 0.85,
            use_cosine_similarity: true,
        }
    }
}

/// Configuration for log ingestion
#[derive(Debug, Clone)]
pub struct LogIngestionConfig {
    pub batch_size: usize,
    pub shard_interval_seconds: i64,
    pub max_retries: u32,
    pub download_timeout_seconds: u64,
    pub delete_after_ingest: bool,
    pub temp_dir: PathBuf,
    pub batch_delay_ms: u64,
    pub auto_sharding: bool,
    pub enable_deduplication: bool,
    pub enable_embedding: bool,
    pub enable_similarity_matching: bool,
    pub bloom_filter_size: usize,
    pub bloom_filter_false_positive_rate: f64,
    pub similarity_config: SimilarityConfig,
    pub embedding_model: EmbeddingModel,
    pub embedding_batch_size: usize,
}

impl Default for LogIngestionConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            shard_interval_seconds: 3600,
            max_retries: 3,
            download_timeout_seconds: 30,
            delete_after_ingest: true,
            temp_dir: PathBuf::from("/tmp/bund_ingestor"),
            batch_delay_ms: 100,
            auto_sharding: true,
            enable_deduplication: true,
            enable_embedding: true,
            enable_similarity_matching: true,
            bloom_filter_size: 1000000,
            bloom_filter_false_positive_rate: 0.01,
            similarity_config: SimilarityConfig::default(),
            embedding_model: EmbeddingModel::AllMiniLML6V2,
            embedding_batch_size: 32,
        }
    }
}

/// Statistics about ingestion
#[derive(Debug, Clone, Default)]
pub struct IngestionStats {
    pub total_lines_read: usize,
    pub total_records_parsed: usize,
    pub total_records_stored: usize,
    pub failed_parses: usize,
    pub batches_processed: usize,
    pub shards_created: usize,
    pub download_size_bytes: u64,
    pub ingestion_duration_ms: u64,
    pub duplicates_filtered: usize,
    pub primary_records: usize,
    pub secondary_records: usize,
    pub embeddings_computed: usize,
    pub similarity_matches: usize,
}

/// In-memory cache of primary records for similarity matching
struct PrimaryRecordCache {
    records: Vec<TelemetryRecord>,
    embeddings: Vec<Vec<f32>>,
    max_size: usize,
}

impl PrimaryRecordCache {
    fn new(max_size: usize) -> Self {
        Self {
            records: Vec::with_capacity(max_size),
            embeddings: Vec::with_capacity(max_size),
            max_size,
        }
    }

    fn add(&mut self, record: TelemetryRecord, embedding: Vec<f32>) {
        if self.records.len() >= self.max_size {
            self.records.remove(0);
            self.embeddings.remove(0);
        }
        self.records.push(record);
        self.embeddings.push(embedding);
    }

    fn find_similar(&self, embedding: &[f32], config: &SimilarityConfig) -> Option<(usize, f32)> {
        let mut best_match = None;
        let mut best_score = 0.0;

        for (i, existing_embedding) in self.embeddings.iter().enumerate() {
            let similarity = if config.use_cosine_similarity {
                Self::cosine_similarity(embedding, existing_embedding)
            } else {
                Self::euclidean_similarity(embedding, existing_embedding)
            };

            if similarity > config.cosine_similarity_threshold as f64 && similarity > best_score {
                best_score = similarity;
                best_match = Some((i, similarity as f32));
            }
        }

        best_match
    }

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        (dot / (norm_a * norm_b)) as f64
    }

    fn euclidean_similarity(a: &[f32], b: &[f32]) -> f64 {
        let distance: f32 = a
            .iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt();
        1.0 - (distance as f64)
    }

    fn get(&self, index: usize) -> Option<&TelemetryRecord> {
        self.records.get(index)
    }
}

/// Helper to convert TelemetryValue to string for display
fn telemetry_value_to_string(value: &TelemetryValue) -> String {
    match value {
        TelemetryValue::Int(v) => v.to_string(),
        TelemetryValue::Float(v) => v.to_string(),
        TelemetryValue::Bool(v) => v.to_string(),
        TelemetryValue::String(v) => v.clone(),
        TelemetryValue::Blob(v) => format!("blob:{} bytes", v.len()),
        TelemetryValue::Json(v) => v.to_string(),
        TelemetryValue::Dynamic(v) => format!("{:?}", v),
        TelemetryValue::Null => "null".to_string(),
    }
}

/// Simple wrapper for fastembed embedding generator
struct EmbeddingGenerator {
    _model: EmbeddingModel,
}

impl EmbeddingGenerator {
    fn new(model: EmbeddingModel) -> Result<Self> {
        Ok(Self { _model: model })
    }

    fn generate_embeddings(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // Simple hash-based embedding for now
        // In production, you would use the actual fastembed API
        let mut embeddings = Vec::with_capacity(texts.len());

        for text in texts {
            let mut embedding = vec![0.0f32; 384];
            let bytes = text.as_bytes();

            for (i, &byte) in bytes.iter().enumerate() {
                let idx = i % 384;
                embedding[idx] += (byte as f32) / 255.0;
            }

            // Normalize
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut embedding {
                    *x /= norm;
                }
            }

            embeddings.push(embedding);
        }

        Ok(embeddings)
    }
}

/// Log file ingestor with embeddings and deduplication
pub struct LogIngestor {
    distribution_manager: Arc<RwLock<DataDistributionManager>>,
    grok_parser: GrokLogParser,
    embedding_generator: Option<Arc<EmbeddingGenerator>>,
    config: LogIngestionConfig,
    http_client: Client,
    bloom_filter: Arc<RwLock<BloomFilter>>,
    primary_cache: Arc<RwLock<PrimaryRecordCache>>,
}

impl LogIngestor {
    pub fn new(
        distribution_manager: Arc<RwLock<DataDistributionManager>>,
        grok_parser: GrokLogParser,
        config: LogIngestionConfig,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(
                config.download_timeout_seconds,
            ))
            .build()
            .unwrap();

        let bloom_filter = Arc::new(RwLock::new(BloomFilter::new(
            config.bloom_filter_size,
            config.bloom_filter_false_positive_rate,
        )));

        let primary_cache = Arc::new(RwLock::new(PrimaryRecordCache::new(10000)));

        let embedding_generator = if config.enable_embedding {
            match EmbeddingGenerator::new(config.embedding_model.clone()) {
                Ok(generator) => Some(Arc::new(generator)),
                Err(e) => {
                    error!("Failed to create embedding generator: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            distribution_manager,
            grok_parser,
            embedding_generator,
            config,
            http_client,
            bloom_filter,
            primary_cache,
        }
    }

    /// Generate a unique fingerprint for a log entry
    fn generate_fingerprint(&self, record: &TelemetryRecord) -> String {
        format!(
            "{}:{}:{}",
            record.key,
            record.source,
            telemetry_value_to_string(&record.value)
        )
    }

    /// Check if record is duplicate using Bloom filter
    fn is_duplicate(&self, fingerprint: &str) -> bool {
        if !self.config.enable_deduplication {
            return false;
        }
        self.bloom_filter.read().contains(fingerprint)
    }

    /// Mark duplicate and add to Bloom filter
    fn mark_as_seen(&self, fingerprint: &str) {
        if self.config.enable_deduplication {
            self.bloom_filter.write().insert(fingerprint);
        }
    }

    /// Prepare text for embedding from a log record
    fn prepare_embedding_text(&self, record: &TelemetryRecord) -> String {
        format!(
            "{}: {} from {} at timestamp {} metadata: {:?}",
            record.key,
            telemetry_value_to_string(&record.value),
            record.source,
            record.timestamp_seconds,
            record.metadata
        )
    }

    /// Generate embeddings for multiple texts in batch
    fn generate_embeddings_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let generator = self
            .embedding_generator
            .as_ref()
            .ok_or_else(|| "Embedding generator not available".to_string())?;

        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        generator.generate_embeddings(&text_refs)
    }

    /// Download a log file from URL
    fn download_log_file(&self, url: &str) -> Result<PathBuf> {
        if !self.config.temp_dir.exists() {
            std::fs::create_dir_all(&self.config.temp_dir)
                .map_err(|e| format!("Failed to create temp dir: {}", e))?;
        }

        let file_name = format!("log_{}.tmp", Utc::now().timestamp());
        let file_path = self.config.temp_dir.join(file_name);

        info!("Downloading log file from {} to {:?}", url, file_path);

        let mut retries = 0;
        loop {
            match self.download_file(url, &file_path) {
                Ok(size) => {
                    info!("Successfully downloaded {} bytes to {:?}", size, file_path);
                    return Ok(file_path);
                }
                Err(e) => {
                    retries += 1;
                    if retries >= self.config.max_retries {
                        error!("Failed to download after {} retries: {}", retries, e);
                        return Err(format!("Download failed: {}", e));
                    }
                    warn!("Download attempt {} failed: {}, retrying...", retries, e);
                    std::thread::sleep(std::time::Duration::from_secs(2u64.pow(retries)));
                }
            }
        }
    }

    fn download_file(&self, url: &str, file_path: &Path) -> Result<u64> {
        use std::io::Read;

        let mut response = self
            .http_client
            .get(url)
            .send()
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        let total_size = response.content_length();
        let mut file =
            File::create(file_path).map_err(|e| format!("Failed to create file: {}", e))?;

        let mut downloaded = 0u64;
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = response
                .read(&mut buffer)
                .map_err(|e| format!("Failed to read response: {}", e))?;
            if bytes_read == 0 {
                break;
            }
            file.write_all(&buffer[..bytes_read])
                .map_err(|e| format!("Failed to write to file: {}", e))?;
            downloaded += bytes_read as u64;

            if let Some(total) = total_size {
                if downloaded % (1024 * 1024) == 0 {
                    debug!("Downloaded {}/{} bytes", downloaded, total);
                }
            }
        }

        Ok(downloaded)
    }

    /// Process a batch of records with batched embedding generation
    fn process_batch(
        &self,
        batch: &mut Vec<TelemetryRecord>,
        log_type: &str,
    ) -> Result<IngestionStats> {
        let mut stats = IngestionStats::default();

        if batch.is_empty() {
            return Ok(stats);
        }

        debug!("Processing batch of {} records", batch.len());

        // Prepare texts for batch embedding generation
        let texts: Vec<String> = batch
            .iter()
            .map(|record| self.prepare_embedding_text(record))
            .collect();

        // Generate embeddings in batch if enabled
        let embeddings = if self.config.enable_embedding {
            match self.generate_embeddings_batch(&texts) {
                Ok(embs) => Some(embs),
                Err(e) => {
                    warn!("Failed to generate batch embeddings: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // Process each record with its embedding
        let mut processed_records = Vec::new();
        for (idx, mut record) in batch.drain(..).enumerate() {
            let fingerprint = self.generate_fingerprint(&record);

            // Check for duplicates
            if self.is_duplicate(&fingerprint) {
                stats.duplicates_filtered += 1;
                continue;
            }

            // Get embedding for this record if available
            let has_embedding = embeddings.as_ref().and_then(|embs| embs.get(idx)).is_some();
            let embedding_data = embeddings.as_ref().and_then(|embs| embs.get(idx).cloned());

            // Check for similarity with primary records
            if self.config.enable_similarity_matching {
                if let Some(embedding) = &embedding_data {
                    if let Some((primary_idx, similarity)) = self
                        .primary_cache
                        .read()
                        .find_similar(embedding, &self.config.similarity_config)
                    {
                        if let Some(primary) = self.primary_cache.read().get(primary_idx) {
                            record.is_primary = false;
                            record.primary_id = Some(primary.id.clone());
                            record.secondary_ids.push(primary.id.clone());
                            stats.similarity_matches += 1;
                            debug!("Record marked as secondary: similarity={:.3}", similarity);
                        }
                    } else {
                        record.is_primary = true;
                        record.primary_id = Some(record.id.clone());
                        if let Some(emb) = embedding_data {
                            self.primary_cache.write().add(record.clone(), emb);
                        }
                    }
                } else {
                    record.is_primary = true;
                    record.primary_id = Some(record.id.clone());
                }
            } else {
                record.is_primary = true;
                record.primary_id = Some(record.id.clone());
            }

            // Update stats
            if record.is_primary {
                stats.primary_records += 1;
            } else {
                stats.secondary_records += 1;
            }

            if has_embedding {
                stats.embeddings_computed += 1;
            }

            // Mark as seen
            self.mark_as_seen(&fingerprint);
            processed_records.push(record);
        }

        // Group records by shard key
        let mut shard_groups: HashMap<i64, Vec<TelemetryRecord>> = HashMap::new();

        if self.config.auto_sharding {
            for record in processed_records.iter() {
                let shard_key = record.timestamp_seconds / self.config.shard_interval_seconds;
                shard_groups
                    .entry(shard_key)
                    .or_insert_with(Vec::new)
                    .push(record.clone());
            }
        } else {
            shard_groups.insert(0, processed_records);
        }

        // Store records
        let distribution_manager = self.distribution_manager.write();

        for (_shard_key, records) in shard_groups.iter() {
            for record in records {
                let key = format!("{}:{}:{}", log_type, record.key, record.timestamp_seconds);

                // Serialize the record to bytes
                if let Ok(data) = serde_json::to_vec(record) {
                    if let Err(e) = distribution_manager.put(&key, &data, Some(Utc::now())) {
                        warn!("Failed to store record: {}", e);
                    } else {
                        stats.total_records_stored += 1;
                    }
                } else {
                    warn!("Failed to serialize record");
                }
            }
            stats.shards_created += 1;
        }

        stats.batches_processed = 1;

        Ok(stats)
    }

    /// Ingest log file from URL
    pub fn ingest_from_url(&self, url: &str, log_type: &str) -> Result<IngestionStats> {
        info!(
            "Starting ingestion from URL: {} with type: {}",
            url, log_type
        );

        let file_path = self.download_log_file(url)?;
        let stats = self.ingest_log_file(&file_path, log_type)?;

        if self.config.delete_after_ingest {
            std::fs::remove_file(&file_path)
                .map_err(|e| format!("Failed to delete temp file {:?}: {}", file_path, e))?;
            info!("Deleted temporary file: {:?}", file_path);
        }

        Ok(stats)
    }

    /// Ingest a local log file
    pub fn ingest_log_file(&self, file_path: &Path, log_type: &str) -> Result<IngestionStats> {
        let start_time = std::time::Instant::now();
        let mut stats = IngestionStats::default();

        info!(
            "Starting ingestion of {:?} with log type: {}",
            file_path, log_type
        );

        let file = File::open(file_path)
            .map_err(|e| format!("Failed to open file {:?}: {}", file_path, e))?;
        let reader = BufReader::new(file);

        let mut batch = Vec::new();
        let mut line_number = 0;

        for line_result in reader.lines() {
            let line =
                line_result.map_err(|e| format!("Failed to read line {}: {}", line_number, e))?;

            stats.total_lines_read += 1;
            line_number += 1;

            match self.grok_parser.process_log_line(&line) {
                Ok(record) => {
                    stats.total_records_parsed += 1;
                    batch.push(record);
                }
                Err(e) => {
                    stats.failed_parses += 1;
                    debug!("Failed to parse line {}: {} - {}", line_number, e, line);
                }
            }

            if batch.len() >= self.config.batch_size {
                let batch_stats = self.process_batch(&mut batch, log_type)?;
                stats.batches_processed += batch_stats.batches_processed;
                stats.total_records_stored += batch_stats.total_records_stored;
                stats.shards_created += batch_stats.shards_created;
                stats.duplicates_filtered += batch_stats.duplicates_filtered;
                stats.primary_records += batch_stats.primary_records;
                stats.secondary_records += batch_stats.secondary_records;
                stats.embeddings_computed += batch_stats.embeddings_computed;
                stats.similarity_matches += batch_stats.similarity_matches;

                if self.config.batch_delay_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(
                        self.config.batch_delay_ms,
                    ));
                }
            }
        }

        if !batch.is_empty() {
            let batch_stats = self.process_batch(&mut batch, log_type)?;
            stats.batches_processed += batch_stats.batches_processed;
            stats.total_records_stored += batch_stats.total_records_stored;
            stats.shards_created += batch_stats.shards_created;
            stats.duplicates_filtered += batch_stats.duplicates_filtered;
            stats.primary_records += batch_stats.primary_records;
            stats.secondary_records += batch_stats.secondary_records;
            stats.embeddings_computed += batch_stats.embeddings_computed;
            stats.similarity_matches += batch_stats.similarity_matches;
        }

        stats.ingestion_duration_ms = start_time.elapsed().as_millis() as u64;

        info!("Ingestion completed: {:?}", stats);
        Ok(stats)
    }

    /// Ingest log lines directly from a vector of strings
    pub fn ingest_log_lines(
        &self,
        log_lines: Vec<String>,
        log_type: &str,
    ) -> Result<IngestionStats> {
        let start_time = std::time::Instant::now();
        let mut stats = IngestionStats::default();

        info!(
            "Starting ingestion of {} log lines with type: {}",
            log_lines.len(),
            log_type
        );

        let mut batch = Vec::new();
        let mut line_number = 0;

        for line in log_lines {
            stats.total_lines_read += 1;
            line_number += 1;

            match self.grok_parser.process_log_line(&line) {
                Ok(record) => {
                    stats.total_records_parsed += 1;
                    batch.push(record);
                }
                Err(e) => {
                    stats.failed_parses += 1;
                    debug!("Failed to parse line {}: {} - {}", line_number, e, line);
                }
            }

            if batch.len() >= self.config.batch_size {
                let batch_stats = self.process_batch(&mut batch, log_type)?;
                stats.batches_processed += batch_stats.batches_processed;
                stats.total_records_stored += batch_stats.total_records_stored;
                stats.shards_created += batch_stats.shards_created;
                stats.duplicates_filtered += batch_stats.duplicates_filtered;
                stats.primary_records += batch_stats.primary_records;
                stats.secondary_records += batch_stats.secondary_records;
                stats.embeddings_computed += batch_stats.embeddings_computed;
                stats.similarity_matches += batch_stats.similarity_matches;

                if self.config.batch_delay_ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(
                        self.config.batch_delay_ms,
                    ));
                }
            }
        }

        if !batch.is_empty() {
            let batch_stats = self.process_batch(&mut batch, log_type)?;
            stats.batches_processed += batch_stats.batches_processed;
            stats.total_records_stored += batch_stats.total_records_stored;
            stats.shards_created += batch_stats.shards_created;
            stats.duplicates_filtered += batch_stats.duplicates_filtered;
            stats.primary_records += batch_stats.primary_records;
            stats.secondary_records += batch_stats.secondary_records;
            stats.embeddings_computed += batch_stats.embeddings_computed;
            stats.similarity_matches += batch_stats.similarity_matches;
        }

        stats.ingestion_duration_ms = start_time.elapsed().as_millis() as u64;

        info!("Ingestion completed: {:?}", stats);
        Ok(stats)
    }
}
