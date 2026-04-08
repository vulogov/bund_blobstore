// src/common/embeddings.rs
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

/// Wrapper for fastembed embedding generator with download progress tracking
pub struct EmbeddingGenerator {
    generator: Arc<RwLock<TextEmbedding>>,
    dimension: usize,
    download_complete: Arc<AtomicBool>,
}

impl EmbeddingGenerator {
    /// Create a new embedding generator with default model and download progress
    pub fn new() -> Result<Self, String> {
        Self::with_download_progress(true)
    }

    /// Create a new embedding generator with optional download progress display
    pub fn with_download_progress(show_progress: bool) -> Result<Self, String> {
        // Create cache directory
        let cache_dir = PathBuf::from("./fastembed_cache");
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir)
                .map_err(|e| format!("Failed to create cache dir: {}", e))?;
        }

        info!("Initializing FastEmbed with model: AllMiniLML6V2");
        info!("Cache directory: {:?}", cache_dir);

        let download_complete = Arc::new(AtomicBool::new(false));
        let download_complete_clone = download_complete.clone();

        // Use default initialization and then set fields
        let mut init_options = InitOptions::default();
        init_options.model_name = EmbeddingModel::AllMiniLML6V2;
        init_options.show_download_progress = show_progress;
        init_options.cache_dir = cache_dir;

        // Spawn a thread to monitor download progress
        if show_progress {
            thread::spawn(move || {
                let start = Instant::now();
                while !download_complete_clone.load(Ordering::Relaxed)
                    && start.elapsed() < Duration::from_secs(300)
                {
                    thread::sleep(Duration::from_millis(500));
                    if start.elapsed() > Duration::from_secs(10)
                        && !download_complete_clone.load(Ordering::Relaxed)
                    {
                        info!(
                            "Still downloading model... This may take a few minutes on first run"
                        );
                    }
                }
                if start.elapsed() >= Duration::from_secs(300) {
                    warn!("Model download timed out after 5 minutes");
                }
            });
        }

        match TextEmbedding::try_new(init_options) {
            Ok(generator) => {
                // Get dimension from the generator
                let dimension = 384; // AllMiniLML6V2 has 384 dimensions
                info!(
                    "Successfully initialized FastEmbed generator with dimension: {}",
                    dimension
                );
                download_complete.store(true, Ordering::Relaxed);
                Ok(Self {
                    generator: Arc::new(RwLock::new(generator)),
                    dimension,
                    download_complete,
                })
            }
            Err(e) => {
                error!("Failed to initialize FastEmbed: {}", e);
                Err(format!("Failed to initialize FastEmbed: {}", e))
            }
        }
    }

    /// Check if model download is complete
    pub fn is_download_complete(&self) -> bool {
        self.download_complete.load(Ordering::Relaxed)
    }

    /// Wait for model download to complete with timeout
    pub fn wait_for_download(&self, timeout_seconds: u64) -> Result<(), String> {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_seconds);

        while !self.is_download_complete() && start.elapsed() < timeout {
            thread::sleep(Duration::from_millis(100));
        }

        if self.is_download_complete() {
            info!("Model download completed successfully");
            Ok(())
        } else {
            Err(format!(
                "Model download timed out after {} seconds",
                timeout_seconds
            ))
        }
    }

    /// Generate embedding for a single text
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, String> {
        let embeddings = self.embed_batch(&[text])?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| "No embedding generated".to_string())
    }

    /// Generate embeddings for multiple texts in batch
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
        let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
        let mut generator = self.generator.write();
        generator
            .embed(texts, None)
            .map_err(|e| format!("Failed to generate embeddings: {}", e))
    }

    /// Get the dimension of embeddings produced by this generator
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

impl Default for EmbeddingGenerator {
    fn default() -> Self {
        Self::new().expect("Failed to create default embedding generator")
    }
}

/// Calculate cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Calculate Euclidean distance between two vectors
pub fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f32>()
        .sqrt()
}

/// Normalize a vector to unit length
pub fn normalize_vector(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Create a zero-initialized embedding of given dimension
pub fn zero_embedding(dim: usize) -> Vec<f32> {
    vec![0.0f32; dim]
}

/// Combine two embeddings by averaging
pub fn average_embeddings(embeddings: &[Vec<f32>]) -> Option<Vec<f32>> {
    if embeddings.is_empty() {
        return None;
    }

    let dim = embeddings[0].len();
    let mut result = vec![0.0f32; dim];

    for emb in embeddings {
        for (i, val) in emb.iter().enumerate() {
            result[i] += val;
        }
    }

    for val in &mut result {
        *val /= embeddings.len() as f32;
    }

    Some(result)
}
