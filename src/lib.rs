pub mod batch;
pub mod blobstore;
pub mod common;
pub mod concurrent;
pub mod data_distribution;
pub mod distributed_graph;
pub mod faceted_search;
pub mod fuzzy_algorithms;
pub mod graph_algorithms;
pub mod graph_store;
pub mod multi_modal;
pub mod pool;
pub mod search;
pub mod serialization;
pub mod sharding;
pub mod timeline;
pub mod vector;
pub mod vector_timeline;
pub mod vm;

// Re-export commonly used types
pub use blobstore::{BlobMetadata, BlobStore, QueryOptions};
pub use faceted_search::{
    Facet, FacetValue, FacetedDocument, FacetedQuery, FacetedSearchIndex, FacetedSearchResult,
};
pub use fuzzy_algorithms::{AdvancedFuzzyResult, JaroWinkler, SorensenDice};
pub use graph_store::{Graph, GraphEdge, GraphNode, GraphQueryOptions, GraphStore};
pub use multi_modal::{Modality, MultiModalSearchResult, MultiModalStore};
pub use search::{
    FullTextIndex, FuzzyConfig, FuzzySearchResult, FuzzyTrie, HighlightedResult, IndexStatistics,
    SearchResult, SearchableBlobStore, TokenizerOptions,
};
pub use serialization::{SerializationFormat, SerializationHelper};
pub use vector::{
    HybridSearch, HybridSearchResult, VectorConfig, VectorSearchResult, VectorStatistics,
    VectorStore,
};

// Re-export concurrent types
pub use concurrent::{
    BlobReadGuard, BlobWriteGuard, ConcurrentBlobStore, ConcurrentFacetedIndex,
    ConcurrentGraphStore, ConcurrentMultiModalStore, ConcurrentSearchStore,
    ConcurrentTelemetryStore, ConcurrentVectorStore, ConcurrentVectorTelemetryStore,
    FacetedReadGuard, FacetedWriteGuard, GraphReadGuard, GraphWriteGuard, MultiModalReadGuard,
    MultiModalWriteGuard, SearchReadGuard, SearchWriteGuard, TelemetryReadGuard,
    TelemetryWriteGuard, UnifiedConcurrentStore, VectorReadGuard, VectorTelemetryReadGuard,
    VectorTelemetryWriteGuard, VectorWriteGuard,
};

pub use batch::{BatchOperation, BatchWorker};
pub use pool::ConnectionPool;

pub use timeline::{
    AggregatedTelemetry, MinuteBucket, TelemetryQuery, TelemetryRecord, TelemetryStore,
    TelemetryValue, TimeInterval,
};

pub use sharding::{
    AllocationType, CacheConfig, ShardAllocation, ShardConfig, ShardDetail, ShardManager,
    ShardManagerBuilder, ShardOperation, ShardResult, ShardStatistics, ShardingStrategy,
};

pub use vector_timeline::{
    TemporalPattern, VectorTelemetryRecord, VectorTelemetryStore, VectorTimeQuery, VectorTimeResult,
};

pub use distributed_graph::{
    DistributedGraphEdge, DistributedGraphManager, DistributedGraphNode, DistributedGraphQuery,
    GraphPartition, GraphTraversalResult,
};

pub use graph_algorithms::{CycleDetectionResult, GraphAlgorithms, LongestPathResult};

pub use data_distribution::{
    AdaptiveConfig,
    AdvancedChunkingConfig,
    BucketStats,
    CacheStats,
    CacheType,
    ChunkSearchResult,
    ChunkStatistics,
    ChunkedDocument,
    ChunkingConfig,
    DataDistributionManager,
    DistributionStats,
    DistributionStrategy,
    EnhancedChunkSearchResult,
    EnhancedChunkedDocument,
    EnhancedTextChunk,
    ShardHealth,
    ShardInfo,
    SimilarityCluster,
    SimilarityConfig,
    StemmingLanguage,
    SystemStats, // Add these
    TextChunk,
    TimeBucketConfig,
    TimeBucketSize,
};

pub use vm::BUND;

pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string().clone()
}
