use crate::blobstore::BlobStore;
use crate::search::SearchableBlobStore;
use crate::serialization::{SerializationFormat, SerializationHelper};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use ndarray::Array1;
use parking_lot::RwLock;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Vector embedding configuration
#[derive(Debug, Clone)]
pub struct VectorConfig {
    pub model: EmbeddingModel,
    pub batch_size: usize,
    pub cache_size: usize,
    pub normalize_embeddings: bool,
}

impl Default for VectorConfig {
    fn default() -> Self {
        VectorConfig {
            model: EmbeddingModel::AllMiniLML6V2,
            batch_size: 32,
            cache_size: 1000,
            normalize_embeddings: true,
        }
    }
}

/// Vector search result
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    pub key: String,
    pub score: f32,
    pub metadata: Option<crate::blobstore::BlobMetadata>,
}

/// Vector store with embedding support
pub struct VectorStore {
    store: BlobStore,
    embedding_model: Arc<RwLock<TextEmbedding>>,
    vectors: Arc<RwLock<HashMap<String, Array1<f32>>>>,
    config: VectorConfig,
    serializer_format: SerializationFormat,
}

impl VectorStore {
    /// Create a new vector store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::open_with_config(path, VectorConfig::default())
    }

    /// Create with custom configuration
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        config: VectorConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = BlobStore::open(path)?;

        // Initialize embedding model - clone the model to avoid moving
        let model = config.model.clone();
        let mut init_options = InitOptions::default();
        init_options.model_name = model;
        init_options.show_download_progress = true;

        let embedding_model = TextEmbedding::try_new(init_options)?;

        let vectors = Arc::new(RwLock::new(HashMap::new()));

        // Try to load existing vectors
        if let Some(vector_data) = store.get("__vectors__")? {
            let loaded_vectors: HashMap<String, Vec<f32>> =
                SerializationHelper::deserialize(&vector_data, SerializationFormat::Bincode)?;

            let mut vectors_lock = vectors.write();
            for (key, vec) in loaded_vectors {
                vectors_lock.insert(key, Array1::from(vec));
            }
        }

        Ok(VectorStore {
            store,
            embedding_model: Arc::new(RwLock::new(embedding_model)),
            vectors,
            config,
            serializer_format: SerializationFormat::Bincode,
        })
    }

    /// Get a reference to the underlying blob store
    pub fn get_store(&self) -> &BlobStore {
        &self.store
    }

    /// Get a mutable reference to the underlying blob store
    pub fn get_store_mut(&mut self) -> &mut BlobStore {
        &mut self.store
    }

    /// Take ownership of the underlying blob store
    pub fn into_store(self) -> BlobStore {
        self.store
    }

    /// Generate embedding for text
    pub fn embed(
        &self,
        text: &str,
    ) -> Result<Array1<f32>, Box<dyn std::error::Error + Send + Sync>> {
        let mut model = self.embedding_model.write();
        let embeddings = model.embed(vec![text], None)?;
        let embedding = embeddings.first().ok_or_else(|| {
            Box::<dyn std::error::Error + Send + Sync>::from("No embedding generated")
        })?;

        let mut vector = Array1::from(embedding.clone());

        if self.config.normalize_embeddings {
            vector = self.normalize(vector);
        }

        Ok(vector)
    }

    /// Generate embeddings for multiple texts in batch
    pub fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<Vec<Array1<f32>>, Box<dyn std::error::Error + Send + Sync>> {
        let chunks: Vec<_> = texts.chunks(self.config.batch_size).collect();
        let mut all_embeddings = Vec::new();

        for chunk in chunks {
            let mut model = self.embedding_model.write();
            let embeddings = model.embed(chunk.to_vec(), None)?;
            for embedding in embeddings {
                let mut vector = Array1::from(embedding);
                if self.config.normalize_embeddings {
                    vector = self.normalize(vector);
                }
                all_embeddings.push(vector);
            }
        }

        Ok(all_embeddings)
    }

    /// Store text with automatic vector embedding
    pub fn insert_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store the original text
        self.store.put(key, text.as_bytes(), prefix)?;

        // Generate and store vector embedding
        let vector = self.embed(text)?;
        self.vectors.write().insert(key.to_string(), vector);

        // Save vectors to disk periodically (every 100 inserts)
        if self.vectors.read().len() % 100 == 0 {
            self.save_vectors()?;
        }

        Ok(())
    }

    /// Store multiple texts in batch
    pub fn insert_batch(
        &mut self,
        items: Vec<(&str, &str, Option<&str>)>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let texts: Vec<&str> = items.iter().map(|(_, text, _)| *text).collect();
        let embeddings = self.embed_batch(&texts)?;

        // Store all items
        for ((key, text, prefix), vector) in items.into_iter().zip(embeddings) {
            self.store.put(key, text.as_bytes(), prefix)?;
            self.vectors.write().insert(key.to_string(), vector);
        }

        self.save_vectors()?;
        Ok(())
    }

    /// Search by semantic similarity
    pub fn search_similar(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let query_vector = self.embed(query)?;
        let vectors = self.vectors.read();

        // Calculate similarity scores in parallel
        let mut results: Vec<VectorSearchResult> = vectors
            .par_iter()
            .map(|(key, vector)| {
                let score = self.cosine_similarity(&query_vector, vector);
                VectorSearchResult {
                    key: key.clone(),
                    score,
                    metadata: None,
                }
            })
            .filter(|r| r.score > 0.3)
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(limit);

        // Enrich with metadata
        for result in &mut results {
            if let Ok(metadata) = self.store.get_metadata(&result.key) {
                result.metadata = metadata;
            }
        }

        Ok(results)
    }

    /// Search by example text (find similar documents)
    pub fn find_similar(
        &self,
        text: &str,
        limit: usize,
    ) -> Result<Vec<VectorSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        self.search_similar(text, limit)
    }

    /// Retrieve original text by key
    pub fn get_text(
        &self,
        key: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(data) = self.store.get(key)? {
            Ok(Some(String::from_utf8(data)?))
        } else {
            Ok(None)
        }
    }

    /// Retrieve data by key
    pub fn get(
        &self,
        key: &str,
    ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.store.get(key)?)
    }

    /// Remove a document and its vector
    pub fn remove(&mut self, key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let removed = self.store.remove(key)?;
        if removed {
            self.vectors.write().remove(key);
            self.save_vectors()?;
        }
        Ok(removed)
    }

    /// Update an existing document
    pub fn update_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.store.put(key, text.as_bytes(), prefix)?;

        let vector = self.embed(text)?;
        self.vectors.write().insert(key.to_string(), vector);

        Ok(())
    }

    /// Get vector for a key
    pub fn get_vector(&self, key: &str) -> Option<Array1<f32>> {
        self.vectors.read().get(key).cloned()
    }

    /// Save vectors to disk
    pub fn save_vectors(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let vectors_lock = self.vectors.read();
        let serializable: HashMap<String, Vec<f32>> = vectors_lock
            .iter()
            .map(|(k, v)| (k.clone(), v.to_vec()))
            .collect();

        let serialized = SerializationHelper::serialize(&serializable, self.serializer_format)?;
        self.store
            .put("__vectors__", &serialized, Some("__system__"))?;

        Ok(())
    }

    /// Get statistics about the vector store
    pub fn statistics(&self) -> VectorStatistics {
        let vectors = self.vectors.read();
        VectorStatistics {
            total_vectors: vectors.len(),
            dimension: if let Some(vector) = vectors.values().next() {
                vector.len()
            } else {
                0
            },
            model: format!("{:?}", self.config.model),
        }
    }

    /// Cosine similarity between two vectors
    fn cosine_similarity(&self, a: &Array1<f32>, b: &Array1<f32>) -> f32 {
        let dot = a.dot(b);
        let norm_a = a.dot(a).sqrt();
        let norm_b = b.dot(b).sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            (dot / (norm_a * norm_b)).max(0.0).min(1.0)
        }
    }

    /// Normalize vector to unit length
    fn normalize(&self, mut vector: Array1<f32>) -> Array1<f32> {
        let norm = vector.dot(&vector).sqrt();
        if norm > 0.0 {
            vector.mapv_inplace(|x| x / norm);
        }
        vector
    }
}

/// Vector store statistics
#[derive(Debug, Clone)]
pub struct VectorStatistics {
    pub total_vectors: usize,
    pub dimension: usize,
    pub model: String,
}

/// Hybrid search combining vector similarity and keyword search
pub struct HybridSearch {
    vector_store: VectorStore,
    search_store: SearchableBlobStore,
}

impl HybridSearch {
    /// Create a new hybrid search instance that shares the database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Open the blob store first
        let store = BlobStore::open(path.as_ref())?;

        // Create vector store with the same store (clone it since BlobStore now implements Clone)
        let vector_store = VectorStore::open_with_config(path.as_ref(), VectorConfig::default())?;

        // Create searchable store with the same store
        let search_store = SearchableBlobStore::open_with_existing_store(store)?;

        Ok(HybridSearch {
            vector_store,
            search_store,
        })
    }

    /// Insert text for hybrid search
    pub fn insert_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.vector_store.insert_text(key, text, prefix)?;
        self.search_store.put_text(key, text, prefix)?;
        Ok(())
    }

    /// Hybrid search combining vector similarity and keyword matching
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        vector_weight: f32,
    ) -> Result<Vec<HybridSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        // Get vector search results
        let vector_results = self.vector_store.search_similar(query, limit * 2)?;

        // Get keyword search results
        let keyword_results = self.search_store.search(query, limit * 2)?;

        // Combine and score results
        let mut combined: HashMap<String, HybridSearchResult> = HashMap::new();

        for result in vector_results {
            combined
                .entry(result.key.clone())
                .or_insert(HybridSearchResult {
                    key: result.key.clone(),
                    vector_score: result.score,
                    keyword_score: 0.0,
                    combined_score: result.score * vector_weight,
                    metadata: result.metadata,
                    text_preview: None,
                });
        }

        for result in keyword_results {
            let keyword_score = (result.score as f32 / 100.0).min(1.0);
            let entry = combined
                .entry(result.key.clone())
                .or_insert(HybridSearchResult {
                    key: result.key.clone(),
                    vector_score: 0.0,
                    keyword_score,
                    combined_score: keyword_score * (1.0 - vector_weight),
                    metadata: result.metadata.clone(),
                    text_preview: None,
                });

            entry.keyword_score = keyword_score;
            entry.combined_score =
                entry.vector_score * vector_weight + keyword_score * (1.0 - vector_weight);
            if entry.metadata.is_none() {
                entry.metadata = result.metadata;
            }
        }

        // Add text previews
        for result in &mut combined.values_mut() {
            if let Ok(Some(data)) = self.vector_store.get(&result.key) {
                if let Ok(text) = String::from_utf8(data) {
                    result.text_preview = Some(text.chars().take(200).collect());
                }
            }
        }

        // Sort by combined score
        let mut results: Vec<HybridSearchResult> = combined.into_values().collect();
        results.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        results.truncate(limit);

        Ok(results)
    }

    /// Get vector store statistics
    pub fn vector_statistics(&self) -> VectorStatistics {
        self.vector_store.statistics()
    }

    /// Remove a document
    pub fn remove(&mut self, key: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let removed = self.vector_store.remove(key)?;
        let _ = self.search_store.remove(key);
        Ok(removed)
    }
}

/// Hybrid search result
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    pub key: String,
    pub vector_score: f32,
    pub keyword_score: f32,
    pub combined_score: f32,
    pub metadata: Option<crate::blobstore::BlobMetadata>,
    pub text_preview: Option<String>,
}
