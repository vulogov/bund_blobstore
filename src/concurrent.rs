use crate::blobstore::BlobStore;
use crate::faceted_search::FacetedSearchIndex;
use crate::graph_store::GraphStore;
use crate::multi_modal::MultiModalStore;
use crate::search::SearchableBlobStore;
use crate::timeline::{AggregatedTelemetry, TelemetryQuery, TelemetryRecord, TelemetryStore};
use crate::vector::VectorStore;
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Thread-safe wrapper for BlobStore
#[derive(Clone)]
pub struct ConcurrentBlobStore {
    pub inner: Arc<RwLock<BlobStore>>,
}

impl ConcurrentBlobStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, redb::Error> {
        let store = BlobStore::open(path)?;
        Ok(ConcurrentBlobStore {
            inner: Arc::new(RwLock::new(store)),
        })
    }

    pub fn read(&self) -> BlobReadGuard<'_> {
        BlobReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    pub fn write(&self) -> BlobWriteGuard<'_> {
        BlobWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    pub fn put(&self, key: &str, data: &[u8], prefix: Option<&str>) -> Result<(), redb::Error> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.put(key, data, prefix)
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.get(key)
    }

    pub fn remove(&self, key: &str) -> Result<bool, redb::Error> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.remove(key)
    }

    pub fn exists(&self, key: &str) -> Result<bool, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.exists(key)
    }

    pub fn len(&self) -> Result<usize, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.len()
    }

    pub fn verify_integrity(&self, key: &str) -> Result<bool, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.verify_integrity(key)
    }

    pub fn list_keys(&self) -> Result<Vec<String>, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.list_keys()
    }
}

pub struct BlobReadGuard<'a> {
    pub guard: RwLockReadGuard<'a, BlobStore>,
}

impl<'a> BlobReadGuard<'a> {
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        self.guard.get(key)
    }

    pub fn exists(&self, key: &str) -> Result<bool, redb::Error> {
        self.guard.exists(key)
    }

    pub fn len(&self) -> Result<usize, redb::Error> {
        self.guard.len()
    }

    pub fn list_keys(&self) -> Result<Vec<String>, redb::Error> {
        self.guard.list_keys()
    }

    pub fn get_metadata(
        &self,
        key: &str,
    ) -> Result<Option<crate::blobstore::BlobMetadata>, redb::Error> {
        self.guard.get_metadata(key)
    }

    pub fn verify_integrity(&self, key: &str) -> Result<bool, redb::Error> {
        self.guard.verify_integrity(key)
    }

    pub fn query_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, crate::blobstore::BlobMetadata)>, redb::Error> {
        self.guard.query_by_prefix(prefix)
    }
}

pub struct BlobWriteGuard<'a> {
    pub guard: RwLockWriteGuard<'a, BlobStore>,
}

impl<'a> BlobWriteGuard<'a> {
    pub fn put(&mut self, key: &str, data: &[u8], prefix: Option<&str>) -> Result<(), redb::Error> {
        self.guard.put(key, data, prefix)
    }

    pub fn remove(&mut self, key: &str) -> Result<bool, redb::Error> {
        self.guard.remove(key)
    }

    pub fn clear(&mut self) -> Result<(), redb::Error> {
        self.guard.clear()
    }

    pub fn update(
        &mut self,
        key: &str,
        data: &[u8],
        prefix: Option<&str>,
    ) -> Result<(), redb::Error> {
        self.guard.update(key, data, prefix)
    }
}

/// Thread-safe wrapper for SearchableBlobStore
#[derive(Clone)]
pub struct ConcurrentSearchStore {
    inner: Arc<RwLock<SearchableBlobStore>>,
}

impl ConcurrentSearchStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = SearchableBlobStore::open(path)?;
        Ok(ConcurrentSearchStore {
            inner: Arc::new(RwLock::new(store)),
        })
    }

    pub fn read(&self) -> SearchReadGuard<'_> {
        SearchReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    pub fn write(&self) -> SearchWriteGuard<'_> {
        SearchWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    pub fn put_text(&self, key: &str, text: &str, prefix: Option<&str>) -> Result<(), redb::Error> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.put_text(key, text, prefix)
    }

    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::search::SearchResult>, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.search(query, limit)
    }

    pub fn fuzzy_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::search::FuzzySearchResult>, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.fuzzy_search(query, limit)
    }

    pub fn search_phrase(
        &self,
        phrase: &str,
        limit: usize,
    ) -> Result<Vec<crate::search::SearchResult>, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.search_phrase(phrase, limit)
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        let read_guard = self.inner.read().unwrap();
        read_guard.get(key)
    }
}

pub struct SearchReadGuard<'a> {
    guard: RwLockReadGuard<'a, SearchableBlobStore>,
}

impl<'a> SearchReadGuard<'a> {
    pub fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::search::SearchResult>, redb::Error> {
        self.guard.search(query, limit)
    }

    pub fn fuzzy_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::search::FuzzySearchResult>, redb::Error> {
        self.guard.fuzzy_search(query, limit)
    }

    pub fn search_phrase(
        &self,
        phrase: &str,
        limit: usize,
    ) -> Result<Vec<crate::search::SearchResult>, redb::Error> {
        self.guard.search_phrase(phrase, limit)
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        self.guard.get(key)
    }
}

pub struct SearchWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, SearchableBlobStore>,
}

impl<'a> SearchWriteGuard<'a> {
    pub fn put_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), redb::Error> {
        self.guard.put_text(key, text, prefix)
    }

    pub fn remove(&mut self, key: &str) -> Result<bool, redb::Error> {
        self.guard.remove(key)
    }

    pub fn reindex(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.reindex()
    }
}

/// Thread-safe wrapper for VectorStore
#[derive(Clone)]
pub struct ConcurrentVectorStore {
    inner: Arc<RwLock<VectorStore>>,
}

impl ConcurrentVectorStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = VectorStore::open(path)?;
        Ok(ConcurrentVectorStore {
            inner: Arc::new(RwLock::new(store)),
        })
    }

    pub fn read(&self) -> VectorReadGuard<'_> {
        VectorReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    pub fn write(&self) -> VectorWriteGuard<'_> {
        VectorWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    pub fn insert_text(
        &self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.insert_text(key, text, prefix)
    }

    pub fn search_similar(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::vector::VectorSearchResult>, Box<dyn std::error::Error + Send + Sync>>
    {
        let read_guard = self.inner.read().unwrap();
        read_guard.search_similar(query, limit)
    }
}

pub struct VectorReadGuard<'a> {
    guard: RwLockReadGuard<'a, VectorStore>,
}

impl<'a> VectorReadGuard<'a> {
    pub fn search_similar(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::vector::VectorSearchResult>, Box<dyn std::error::Error + Send + Sync>>
    {
        self.guard.search_similar(query, limit)
    }

    pub fn get_text(
        &self,
        key: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.get_text(key)
    }

    pub fn statistics(&self) -> crate::vector::VectorStatistics {
        self.guard.statistics()
    }
}

pub struct VectorWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, VectorStore>,
}

impl<'a> VectorWriteGuard<'a> {
    pub fn insert_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.insert_text(key, text, prefix)
    }

    pub fn insert_batch(
        &mut self,
        items: Vec<(&str, &str, Option<&str>)>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.insert_batch(items)
    }

    pub fn remove(&mut self, key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.remove(key)
    }
}

/// Thread-safe wrapper for GraphStore
#[derive(Clone)]
pub struct ConcurrentGraphStore {
    inner: Arc<RwLock<GraphStore>>,
}

impl ConcurrentGraphStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = GraphStore::open(path)?;
        Ok(ConcurrentGraphStore {
            inner: Arc::new(RwLock::new(store)),
        })
    }

    pub fn read(&self) -> GraphReadGuard<'_> {
        GraphReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    pub fn write(&self) -> GraphWriteGuard<'_> {
        GraphWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    pub fn save_graph(
        &self,
        graph: &crate::graph_store::Graph,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.save_graph(graph)
    }

    pub fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<Option<crate::graph_store::Graph>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.load_graph(graph_id)
    }
}

pub struct GraphReadGuard<'a> {
    guard: RwLockReadGuard<'a, GraphStore>,
}

impl<'a> GraphReadGuard<'a> {
    pub fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<Option<crate::graph_store::Graph>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.load_graph(graph_id)
    }

    pub fn load_node(
        &self,
        graph_id: &str,
        node_id: &str,
    ) -> Result<Option<crate::graph_store::GraphNode>, Box<dyn std::error::Error + Send + Sync>>
    {
        self.guard.load_node(graph_id, node_id)
    }

    pub fn load_all_nodes(
        &self,
        graph_id: &str,
    ) -> Result<Vec<crate::graph_store::GraphNode>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.load_all_nodes(graph_id)
    }
}

pub struct GraphWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, GraphStore>,
}

impl<'a> GraphWriteGuard<'a> {
    pub fn save_graph(
        &mut self,
        graph: &crate::graph_store::Graph,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.save_graph(graph)
    }

    pub fn store_node(
        &mut self,
        graph_id: &str,
        node: &crate::graph_store::GraphNode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.store_node(graph_id, node)
    }

    pub fn store_edge(
        &mut self,
        graph_id: &str,
        edge: &crate::graph_store::GraphEdge,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.store_edge(graph_id, edge)
    }

    pub fn delete_graph(
        &mut self,
        graph_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.delete_graph(graph_id)
    }
}

/// Thread-safe wrapper for FacetedSearchIndex
#[derive(Clone)]
pub struct ConcurrentFacetedIndex {
    inner: Arc<RwLock<FacetedSearchIndex>>,
}

impl ConcurrentFacetedIndex {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let index = FacetedSearchIndex::new(path)?;
        Ok(ConcurrentFacetedIndex {
            inner: Arc::new(RwLock::new(index)),
        })
    }

    pub fn read(&self) -> FacetedReadGuard<'_> {
        FacetedReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    pub fn write(&self) -> FacetedWriteGuard<'_> {
        FacetedWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    pub fn add_document(
        &self,
        doc: crate::faceted_search::FacetedDocument,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.add_document(doc)
    }

    pub fn search(
        &self,
        query: &crate::faceted_search::FacetedQuery,
    ) -> Result<crate::faceted_search::FacetedSearchResult, Box<dyn std::error::Error + Send + Sync>>
    {
        let read_guard = self.inner.read().unwrap();
        read_guard.search(query)
    }
}

pub struct FacetedReadGuard<'a> {
    guard: RwLockReadGuard<'a, FacetedSearchIndex>,
}

impl<'a> FacetedReadGuard<'a> {
    pub fn search(
        &self,
        query: &crate::faceted_search::FacetedQuery,
    ) -> Result<crate::faceted_search::FacetedSearchResult, Box<dyn std::error::Error + Send + Sync>>
    {
        self.guard.search(query)
    }
}

pub struct FacetedWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, FacetedSearchIndex>,
}

impl<'a> FacetedWriteGuard<'a> {
    pub fn add_document(
        &mut self,
        doc: crate::faceted_search::FacetedDocument,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.add_document(doc)
    }
}

/// Thread-safe wrapper for MultiModalStore
#[derive(Clone)]
pub struct ConcurrentMultiModalStore {
    inner: Arc<RwLock<MultiModalStore>>,
}

impl ConcurrentMultiModalStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = MultiModalStore::open(path)?;
        Ok(ConcurrentMultiModalStore {
            inner: Arc::new(RwLock::new(store)),
        })
    }

    pub fn read(&self) -> MultiModalReadGuard<'_> {
        MultiModalReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    pub fn write(&self) -> MultiModalWriteGuard<'_> {
        MultiModalWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    pub fn insert_text(
        &self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.insert_text(key, text, prefix)
    }

    pub fn search_similar(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<
        Vec<crate::multi_modal::MultiModalSearchResult>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let read_guard = self.inner.read().unwrap();
        read_guard.search_similar(query, limit)
    }
}

pub struct MultiModalReadGuard<'a> {
    guard: RwLockReadGuard<'a, MultiModalStore>,
}

impl<'a> MultiModalReadGuard<'a> {
    pub fn search_similar(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<
        Vec<crate::multi_modal::MultiModalSearchResult>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.guard.search_similar(query, limit)
    }

    pub fn cross_modal_search(
        &self,
        query: &str,
        target_modality: crate::multi_modal::Modality,
        limit: usize,
    ) -> Result<
        Vec<crate::multi_modal::MultiModalSearchResult>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        self.guard.cross_modal_search(query, target_modality, limit)
    }
}

pub struct MultiModalWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, MultiModalStore>,
}

impl<'a> MultiModalWriteGuard<'a> {
    pub fn insert_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.insert_text(key, text, prefix)
    }

    pub fn insert_image(
        &mut self,
        key: &str,
        image_data: &[u8],
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.insert_image(key, image_data, prefix)
    }

    pub fn insert_audio(
        &mut self,
        key: &str,
        audio_data: &[u8],
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.insert_audio(key, audio_data, prefix)
    }
}

/// Thread-safe wrapper for TelemetryStore
#[derive(Clone)]
pub struct ConcurrentTelemetryStore {
    inner: Arc<RwLock<TelemetryStore>>,
}

impl ConcurrentTelemetryStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = TelemetryStore::open(path)?;
        Ok(ConcurrentTelemetryStore {
            inner: Arc::new(RwLock::new(store)),
        })
    }

    pub fn read(&self) -> TelemetryReadGuard<'_> {
        TelemetryReadGuard {
            guard: self.inner.read().unwrap(),
        }
    }

    pub fn write(&self) -> TelemetryWriteGuard<'_> {
        TelemetryWriteGuard {
            guard: self.inner.write().unwrap(),
        }
    }

    pub fn store(
        &self,
        record: TelemetryRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.store(record)
    }

    pub fn get_record(
        &self,
        id: &str,
    ) -> Result<Option<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.get_record(id)
    }

    pub fn query(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.query(query)
    }

    pub fn query_bucketed(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<AggregatedTelemetry>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.query_bucketed(query)
    }

    pub fn link_primary_secondary(
        &self,
        primary_id: &str,
        secondary_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut write_guard = self.inner.write().unwrap();
        write_guard.link_primary_secondary(primary_id, secondary_id)
    }

    pub fn get_secondaries(
        &self,
        primary_id: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.get_secondaries(primary_id)
    }

    pub fn get_primary(
        &self,
        secondary_id: &str,
    ) -> Result<Option<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.get_primary(secondary_id)
    }

    pub fn search_by_key(
        &self,
        key_pattern: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.search_by_key(key_pattern)
    }

    pub fn search_by_source(
        &self,
        source: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        let read_guard = self.inner.read().unwrap();
        read_guard.search_by_source(source)
    }

    pub fn get_time_range(
        &self,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, Box<dyn std::error::Error + Send + Sync>>
    {
        let read_guard = self.inner.read().unwrap();
        read_guard.get_time_range()
    }
}

pub struct TelemetryReadGuard<'a> {
    guard: RwLockReadGuard<'a, TelemetryStore>,
}

impl<'a> TelemetryReadGuard<'a> {
    pub fn get_record(
        &self,
        id: &str,
    ) -> Result<Option<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.get_record(id)
    }

    pub fn query(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.query(query)
    }

    pub fn query_bucketed(
        &self,
        query: &TelemetryQuery,
    ) -> Result<Vec<AggregatedTelemetry>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.query_bucketed(query)
    }

    pub fn get_secondaries(
        &self,
        primary_id: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.get_secondaries(primary_id)
    }

    pub fn get_primary(
        &self,
        secondary_id: &str,
    ) -> Result<Option<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.get_primary(secondary_id)
    }

    pub fn search_by_key(
        &self,
        key_pattern: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.search_by_key(key_pattern)
    }

    pub fn search_by_source(
        &self,
        source: &str,
    ) -> Result<Vec<TelemetryRecord>, Box<dyn std::error::Error + Send + Sync>> {
        self.guard.search_by_source(source)
    }

    pub fn get_time_range(
        &self,
    ) -> Result<Option<(DateTime<Utc>, DateTime<Utc>)>, Box<dyn std::error::Error + Send + Sync>>
    {
        self.guard.get_time_range()
    }
}

pub struct TelemetryWriteGuard<'a> {
    guard: RwLockWriteGuard<'a, TelemetryStore>,
}

impl<'a> TelemetryWriteGuard<'a> {
    pub fn store(
        &mut self,
        record: TelemetryRecord,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.store(record)
    }

    pub fn link_primary_secondary(
        &mut self,
        primary_id: &str,
        secondary_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.guard.link_primary_secondary(primary_id, secondary_id)
    }
}

/// Unified concurrent store that provides access to all storage types
#[derive(Clone)]
pub struct UnifiedConcurrentStore {
    blob: ConcurrentBlobStore,
    search: ConcurrentSearchStore,
    vector: ConcurrentVectorStore,
    graph: ConcurrentGraphStore,
    faceted: ConcurrentFacetedIndex,
    multi_modal: ConcurrentMultiModalStore,
    telemetry: ConcurrentTelemetryStore,
}

impl UnifiedConcurrentStore {
    pub fn open<P: AsRef<Path>>(
        base_path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let base_path = base_path.as_ref();

        // Create a unique directory for this unified store
        let store_dir = base_path;
        std::fs::create_dir_all(store_dir)?;

        // Each component gets its own subdirectory
        let blob_path = store_dir.join("blob.redb");
        let search_path = store_dir.join("search.redb");
        let vector_path = store_dir.join("vector.redb");
        let graph_path = store_dir.join("graph.redb");
        let faceted_path = store_dir.join("faceted.redb");
        let multi_modal_path = store_dir.join("multimodal.redb");
        let telemetry_path = store_dir.join("telemetry.redb");

        Ok(UnifiedConcurrentStore {
            blob: ConcurrentBlobStore::open(&blob_path)?,
            search: ConcurrentSearchStore::open(&search_path)?,
            vector: ConcurrentVectorStore::open(&vector_path)?,
            graph: ConcurrentGraphStore::open(&graph_path)?,
            faceted: ConcurrentFacetedIndex::open(&faceted_path)?,
            multi_modal: ConcurrentMultiModalStore::open(&multi_modal_path)?,
            telemetry: ConcurrentTelemetryStore::open(&telemetry_path)?,
        })
    }

    pub fn blob(&self) -> &ConcurrentBlobStore {
        &self.blob
    }

    pub fn search(&self) -> &ConcurrentSearchStore {
        &self.search
    }

    pub fn vector(&self) -> &ConcurrentVectorStore {
        &self.vector
    }

    pub fn graph(&self) -> &ConcurrentGraphStore {
        &self.graph
    }

    pub fn faceted(&self) -> &ConcurrentFacetedIndex {
        &self.faceted
    }

    pub fn multi_modal(&self) -> &ConcurrentMultiModalStore {
        &self.multi_modal
    }

    pub fn telemetry(&self) -> &ConcurrentTelemetryStore {
        &self.telemetry
    }
}
