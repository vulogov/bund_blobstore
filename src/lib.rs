pub mod batch;
pub mod blobstore;
pub mod concurrent;
pub mod faceted_search;
pub mod fuzzy_algorithms;
pub mod graph_store;
pub mod multi_modal;
pub mod pool;
pub mod search;
pub mod serialization;
pub mod vector;

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

// Re-export concurrent types from their respective modules
pub use concurrent::{
    BlobReadGuard, BlobWriteGuard, ConcurrentBlobStore, ConcurrentFacetedIndex,
    ConcurrentGraphStore, ConcurrentMultiModalStore, ConcurrentSearchStore, ConcurrentVectorStore,
    FacetedReadGuard, FacetedWriteGuard, GraphReadGuard, GraphWriteGuard, MultiModalReadGuard,
    MultiModalWriteGuard, SearchReadGuard, SearchWriteGuard, UnifiedConcurrentStore,
    VectorReadGuard, VectorWriteGuard,
};

// Re-export batch and pool from their modules
pub use batch::{BatchOperation, BatchWorker};
pub use pool::ConnectionPool;
