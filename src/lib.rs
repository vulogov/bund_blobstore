extern crate log;

pub mod blobstore;
pub mod concurrent;
pub mod faceted_search;
pub mod fuzzy_algorithms;
pub mod graph_store;
pub mod multi_modal;
pub mod search;
pub mod serialization;
pub mod vector;

// Re-export commonly used types
pub use blobstore::{BlobMetadata, BlobStore, QueryOptions};
pub use concurrent::{BatchWorker, ConcurrentBlobStore, ConnectionPool, ReadGuard, WriteGuard};
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
