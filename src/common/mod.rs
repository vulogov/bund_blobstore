// src/common/mod.rs
pub mod embeddings;
pub mod grok_integration;
pub mod json_fingerprint;
pub mod log_ingestor;
pub mod log_worker_pool;
pub mod multidimensional_storage;
pub mod root_cause_analyzer;

// Re-export commonly used types
pub use embeddings::{
    EmbeddingGenerator, average_embeddings, cosine_similarity, euclidean_distance,
    normalize_vector, zero_embedding,
};
pub use grok_integration::GrokLogParser;
pub use json_fingerprint::{
    JsonDocument, JsonFingerprintConfig, JsonFingerprintManager, JsonSearchResult, json_from_str,
    to_pretty_json,
};
pub use log_ingestor::{IngestionStats, LogIngestionConfig, LogIngestor, SimilarityConfig};
pub use log_worker_pool::{
    IngestionTask, LogWorkerPool, PoolStats, TaskResult, WorkerPoolConfig, start_worker_pool,
    start_worker_pool_with_ingestor, start_worker_pool_with_manager, stop_worker_pool,
    submit_batch, wait_for_tasks,
};
pub use root_cause_analyzer::{
    AnalysisSummary, CausalChain, CausalLink, CorrelationMatrix, EventCluster, EventOccurrence,
    EventPattern, PropagationEvent, RCAConfig, RCAReport, Recommendation, ReportMetadata,
    ReportVisualizations, RootCauseAnalyzer, RootCauseResult, TimeRange, create_event_occurrence,
};

pub use multidimensional_storage::{
    Bounds, Coord1D, Coord2D, Coord3D, Coordinate, DimensionMetadata, DimensionType,
    MultidimensionalStorage, SampleIdQueue, TelemetrySample,
};
