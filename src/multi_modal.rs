use crate::blobstore::BlobStore;
use crate::serialization::{SerializationFormat, SerializationHelper};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use image::io::Reader as ImageReader;
use ndarray::Array1;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Multi-modal embedding types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modality {
    Text,
    Image,
    Audio,
}

/// Multi-modal search result
#[derive(Debug, Clone)]
pub struct MultiModalSearchResult {
    pub key: String,
    pub modality: Modality,
    pub score: f32,
    pub metadata: Option<crate::blobstore::BlobMetadata>,
}

/// Multi-modal embedding store
pub struct MultiModalStore {
    store: BlobStore,
    text_embedder: Arc<RwLock<TextEmbedding>>,
    embeddings: Arc<RwLock<HashMap<String, (Modality, Array1<f32>)>>>,
    serializer_format: SerializationFormat,
}

impl MultiModalStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = BlobStore::open(path)?;

        // Initialize embedding model using proper method for non-exhaustive struct
        let mut init_options = InitOptions::default();
        init_options.model_name = EmbeddingModel::AllMiniLML6V2;
        init_options.show_download_progress = true;

        let text_embedder = TextEmbedding::try_new(init_options)?;

        let mut store = MultiModalStore {
            store,
            text_embedder: Arc::new(RwLock::new(text_embedder)),
            embeddings: Arc::new(RwLock::new(HashMap::new())),
            serializer_format: SerializationFormat::Bincode,
        };

        store.load_embeddings()?;
        Ok(store)
    }

    pub fn embed_text(
        &self,
        text: &str,
    ) -> Result<Array1<f32>, Box<dyn std::error::Error + Send + Sync>> {
        let mut embedder = self.text_embedder.write();
        let embeddings = embedder.embed(vec![text], None)?;
        let embedding = embeddings.first().ok_or_else(|| "No embedding generated")?;

        Ok(Array1::from(embedding.clone()))
    }

    pub fn embed_image(
        &self,
        image_data: &[u8],
    ) -> Result<Array1<f32>, Box<dyn std::error::Error + Send + Sync>> {
        // Load and preprocess image
        let img = ImageReader::new(std::io::Cursor::new(image_data))
            .with_guessed_format()?
            .decode()?;

        // Resize to standard size
        let resized = img.resize(224, 224, image::imageops::FilterType::Lanczos3);

        // Convert to RGB and normalize
        let rgb = resized.to_rgb8();
        let mut pixels = Vec::new();
        for pixel in rgb.pixels() {
            pixels.push(pixel[0] as f32 / 255.0);
            pixels.push(pixel[1] as f32 / 255.0);
            pixels.push(pixel[2] as f32 / 255.0);
        }

        // Create embedding from image features
        let mean: f32 = pixels.iter().sum::<f32>() / pixels.len() as f32;
        let std_dev: f32 =
            (pixels.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / pixels.len() as f32).sqrt();

        let mut embedding = vec![mean, std_dev];
        embedding.extend_from_slice(&pixels[..128.min(pixels.len())]);

        // Pad or truncate to fixed size
        let target_size = 384;
        if embedding.len() < target_size {
            embedding.resize(target_size, 0.0);
        } else {
            embedding.truncate(target_size);
        }

        Ok(Array1::from(embedding))
    }

    pub fn embed_audio(
        &self,
        audio_data: &[u8],
    ) -> Result<Array1<f32>, Box<dyn std::error::Error + Send + Sync>> {
        let samples: Vec<f32> = if audio_data.len() < 1000 {
            (0..384).map(|i| i as f32 / 384.0).collect()
        } else {
            let step = audio_data.len() / 384;
            (0..384)
                .map(|i| {
                    let idx = (i * step).min(audio_data.len() - 1);
                    audio_data[idx] as f32 / 255.0
                })
                .collect()
        };

        Ok(Array1::from(samples))
    }

    pub fn insert_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let embedding = self.embed_text(text)?;
        self.store.put(key, text.as_bytes(), prefix)?;
        self.embeddings
            .write()
            .insert(key.to_string(), (Modality::Text, embedding));
        self.save_embeddings()?;
        Ok(())
    }

    pub fn insert_image(
        &mut self,
        key: &str,
        image_data: &[u8],
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let embedding = self.embed_image(image_data)?;
        self.store.put(key, image_data, prefix)?;
        self.embeddings
            .write()
            .insert(key.to_string(), (Modality::Image, embedding));
        self.save_embeddings()?;
        Ok(())
    }

    pub fn insert_audio(
        &mut self,
        key: &str,
        audio_data: &[u8],
        prefix: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let embedding = self.embed_audio(audio_data)?;
        self.store.put(key, audio_data, prefix)?;
        self.embeddings
            .write()
            .insert(key.to_string(), (Modality::Audio, embedding));
        self.save_embeddings()?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        self.store.get(key)
    }

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

    pub fn search_similar(
        &self,
        query_text: &str,
        limit: usize,
    ) -> Result<Vec<MultiModalSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let query_embedding = self.embed_text(query_text)?;
        let embeddings = self.embeddings.read();

        let mut results: Vec<MultiModalSearchResult> = embeddings
            .iter()
            .map(|(key, (modality, embedding))| {
                let score = self.cosine_similarity(&query_embedding, embedding);
                MultiModalSearchResult {
                    key: key.clone(),
                    modality: *modality,
                    score,
                    metadata: None,
                }
            })
            .filter(|r| r.score > 0.3)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(limit);

        for result in &mut results {
            if let Ok(metadata) = self.store.get_metadata(&result.key) {
                result.metadata = metadata;
            }
        }

        Ok(results)
    }

    pub fn cross_modal_search(
        &self,
        query_text: &str,
        target_modality: Modality,
        limit: usize,
    ) -> Result<Vec<MultiModalSearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        let query_embedding = self.embed_text(query_text)?;
        let embeddings = self.embeddings.read();

        let mut results: Vec<MultiModalSearchResult> = embeddings
            .iter()
            .filter(|(_, (modality, _))| *modality == target_modality)
            .map(|(key, (modality, embedding))| {
                let score = self.cosine_similarity(&query_embedding, embedding);
                MultiModalSearchResult {
                    key: key.clone(),
                    modality: *modality,
                    score,
                    metadata: None,
                }
            })
            .filter(|r| r.score > 0.3)
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(limit);

        Ok(results)
    }

    fn save_embeddings(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let embeddings_lock = self.embeddings.read();
        let serializable: HashMap<String, (Modality, Vec<f32>)> = embeddings_lock
            .iter()
            .map(|(k, (modality, v))| (k.clone(), (*modality, v.to_vec())))
            .collect();

        let serialized = SerializationHelper::serialize(&serializable, self.serializer_format)?;
        self.store.put(
            "__multi_modal_embeddings__",
            &serialized,
            Some("__system__"),
        )?;
        Ok(())
    }

    fn load_embeddings(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(data) = self.store.get("__multi_modal_embeddings__")? {
            let loaded: HashMap<String, (Modality, Vec<f32>)> =
                SerializationHelper::deserialize(&data, self.serializer_format)?;

            let mut embeddings_lock = self.embeddings.write();
            for (key, (modality, vec)) in loaded {
                embeddings_lock.insert(key, (modality, Array1::from(vec)));
            }
        }
        Ok(())
    }

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
}
