use crate::blobstore::BlobStore;
use crate::serialization::{SerializationFormat, SerializationHelper};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

/// Tokenizer options for text processing
#[derive(Debug, Clone)]
pub struct TokenizerOptions {
    pub min_token_length: usize,
    pub max_token_length: usize,
    pub stop_words: HashSet<String>,
    pub stem_words: bool,
    pub case_sensitive: bool,
}

impl Default for TokenizerOptions {
    fn default() -> Self {
        let mut stop_words = HashSet::new();
        for word in [
            "a", "an", "and", "or", "the", "of", "to", "in", "for", "with", "on", "at", "by", "is",
            "are", "was", "were",
        ] {
            stop_words.insert(word.to_string());
        }

        TokenizerOptions {
            min_token_length: 2,
            max_token_length: 50,
            stop_words,
            stem_words: true,
            case_sensitive: false,
        }
    }
}

/// Search result with relevance score
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub key: String,
    pub score: f64,
    pub matches: Vec<String>,
    pub metadata: Option<crate::blobstore::BlobMetadata>,
}

/// Full-text search index
pub struct FullTextIndex {
    inverted_index: Arc<RwLock<HashMap<String, HashMap<String, usize>>>>, // term -> (key -> frequency)
    tokenizer_options: TokenizerOptions,
    index_serializer: SerializationFormat,
}

impl FullTextIndex {
    /// Create a new full-text index
    pub fn new(options: TokenizerOptions) -> Self {
        FullTextIndex {
            inverted_index: Arc::new(RwLock::new(HashMap::new())),
            tokenizer_options: options,
            index_serializer: SerializationFormat::Bincode,
        }
    }

    /// Index a document (key -> text content)
    pub fn index_document(&self, key: &str, content: &str) {
        let tokens = self.tokenize(content);
        let mut index = self.inverted_index.write();

        // Count term frequencies for this document
        let mut term_freq: HashMap<String, usize> = HashMap::new();
        for token in tokens {
            *term_freq.entry(token).or_insert(0) += 1;
        }

        // Update inverted index
        for (term, freq) in term_freq {
            let doc_map = index.entry(term).or_insert_with(HashMap::new);
            *doc_map.entry(key.to_string()).or_insert(0) += freq;
        }
    }

    /// Remove a document from the index
    pub fn remove_document(&self, key: &str) {
        let mut index = self.inverted_index.write();
        let mut to_remove = Vec::new();

        for (term, docs) in index.iter_mut() {
            docs.remove(key);
            if docs.is_empty() {
                to_remove.push(term.clone());
            }
        }

        for term in to_remove {
            index.remove(&term);
        }
    }

    /// Search for a query and return ranked results
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_tokens = self.tokenize(query);
        let index = self.inverted_index.read();

        // Calculate TF-IDF scores
        let mut scores: HashMap<String, f64> = HashMap::new();
        let mut matches: HashMap<String, Vec<String>> = HashMap::new();

        for token in query_tokens {
            if let Some(docs) = index.get(&token) {
                for (doc, freq) in docs {
                    let score = *freq as f64; // Simple term frequency
                    *scores.entry(doc.clone()).or_insert(0.0) += score;
                    matches
                        .entry(doc.clone())
                        .or_insert_with(Vec::new)
                        .push(token.clone());
                }
            }
        }

        // Sort by score and collect results
        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .map(|(key, score)| {
                let matched_terms = matches.get(&key).cloned().unwrap_or_default();
                SearchResult {
                    key,
                    score,
                    matches: matched_terms,
                    metadata: None,
                }
            })
            .collect();

        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(limit);

        results
    }

    /// Save index to disk
    pub fn save(
        &self,
        store: &mut BlobStore,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let index = self.inverted_index.read();
        let serialized = SerializationHelper::serialize(&*index, self.index_serializer)?;
        store.put("__search_index__", &serialized, Some("__system__"))?;
        Ok(())
    }

    /// Load index from disk
    pub fn load(
        store: &BlobStore,
        options: TokenizerOptions,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let index_data = store.get("__search_index__")?;

        let inverted_index = if let Some(data) = index_data {
            SerializationHelper::deserialize(&data, SerializationFormat::Bincode)?
        } else {
            HashMap::new()
        };

        Ok(FullTextIndex {
            inverted_index: Arc::new(RwLock::new(inverted_index)),
            tokenizer_options: options,
            index_serializer: SerializationFormat::Bincode,
        })
    }

    /// Get index statistics
    pub fn statistics(&self) -> IndexStatistics {
        let index = self.inverted_index.read();
        let total_terms = index.len();
        let total_docs: usize = index.values().map(|docs| docs.len()).sum();

        IndexStatistics {
            total_terms,
            total_document_references: total_docs,
            unique_terms: total_terms,
        }
    }

    /// Tokenize text for indexing/searching
    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let processed_text = if self.tokenizer_options.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };

        // Split by common delimiters
        for word in processed_text.split(|c: char| !c.is_alphanumeric()) {
            if word.is_empty() {
                continue;
            }

            let word_len = word.len();
            if word_len < self.tokenizer_options.min_token_length
                || word_len > self.tokenizer_options.max_token_length
            {
                continue;
            }

            if self.tokenizer_options.stop_words.contains(word) {
                continue;
            }

            let token = if self.tokenizer_options.stem_words {
                self.stem_word(word)
            } else {
                word.to_string()
            };

            tokens.push(token);
        }

        tokens
    }
    /// Public method to tokenize text (useful for testing)
    pub fn tokenize_text(&self, text: &str) -> Vec<String> {
        self.tokenize(text)
    }

    /// Simple stemming (remove common suffixes)
    fn stem_word(&self, word: &str) -> String {
        let word = word.to_lowercase();

        // Remove common suffixes
        let suffixes = ["ing", "ed", "s", "es", "ly", "ment", "tion", "able", "ible"];
        for suffix in suffixes {
            if word.ends_with(suffix) && word.len() > suffix.len() + 2 {
                return word[..word.len() - suffix.len()].to_string();
            }
        }

        word
    }
}

/// Index statistics
#[derive(Debug, Clone)]
pub struct IndexStatistics {
    pub total_terms: usize,
    pub total_document_references: usize,
    pub unique_terms: usize,
}

/// Enhanced blobstore with search capabilities
pub struct SearchableBlobStore {
    store: BlobStore,
    index: FullTextIndex,
    auto_index: bool,
}

impl SearchableBlobStore {
    /// Create a new searchable blobstore
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = BlobStore::open(path)?;
        let index = FullTextIndex::load(&store, TokenizerOptions::default())?;

        Ok(SearchableBlobStore {
            store,
            index,
            auto_index: true,
        })
    }
    pub fn open_with_existing_store(
        store: BlobStore,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let index = FullTextIndex::load(&store, TokenizerOptions::default())?;
        Ok(SearchableBlobStore {
            store,
            index,
            auto_index: true,
        })
    }
    /// Create with custom tokenizer options
    pub fn open_with_options<P: AsRef<Path>>(
        path: P,
        options: TokenizerOptions,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = BlobStore::open(path)?;
        let index = FullTextIndex::load(&store, options)?;

        Ok(SearchableBlobStore {
            store,
            index,
            auto_index: true,
        })
    }

    /// Store data with automatic indexing
    pub fn put(&mut self, key: &str, data: &[u8], prefix: Option<&str>) -> Result<(), redb::Error> {
        self.store.put(key, data, prefix)?;

        if self.auto_index {
            // Index text content if it looks like UTF-8 text
            if let Ok(text) = std::str::from_utf8(data) {
                self.index.index_document(key, text);
            }
        }

        Ok(())
    }

    /// Store text data with explicit indexing
    pub fn put_text(
        &mut self,
        key: &str,
        text: &str,
        prefix: Option<&str>,
    ) -> Result<(), redb::Error> {
        self.store.put(key, text.as_bytes(), prefix)?;

        if self.auto_index {
            self.index.index_document(key, text);
        }

        Ok(())
    }

    /// Retrieve data
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        self.store.get(key)
    }

    /// Remove data and update index
    pub fn remove(&mut self, key: &str) -> Result<bool, redb::Error> {
        let removed = self.store.remove(key)?;
        if removed {
            self.index.remove_document(key);
        }
        Ok(removed)
    }

    /// Search for text across all indexed documents
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, redb::Error> {
        let mut results = self.index.search(query, limit);

        // Enrich results with metadata
        for result in &mut results {
            if let Ok(metadata) = self.store.get_metadata(&result.key) {
                result.metadata = metadata;
            }
        }

        Ok(results)
    }

    /// Search with highlighting
    pub fn search_with_highlight(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<HighlightedResult>, redb::Error> {
        let results = self.search(query, limit)?;
        let mut highlighted = Vec::new();

        for result in results {
            if let Some(data) = self.store.get(&result.key)? {
                if let Ok(text) = String::from_utf8(data) {
                    let highlighted_text = Self::highlight_text(&text, &result.matches);
                    highlighted.push(HighlightedResult {
                        key: result.key,
                        score: result.score,
                        highlighted_text,
                        metadata: result.metadata,
                    });
                }
            }
        }

        Ok(highlighted)
    }

    /// Save index to disk
    pub fn save_index(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.index.save(&mut self.store)
    }

    /// Reindex all data
    pub fn reindex(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Clear existing index
        self.index = FullTextIndex::new(TokenizerOptions::default());

        // Reindex all text data
        let all_data = self.store.get_all()?;
        for (key, data) in all_data {
            if let Ok(text) = String::from_utf8(data) {
                self.index.index_document(&key, &text);
            }
        }

        self.save_index()?;
        Ok(())
    }

    /// Enable/disable automatic indexing
    pub fn set_auto_index(&mut self, enabled: bool) {
        self.auto_index = enabled;
    }

    /// Get index statistics
    pub fn index_stats(&self) -> IndexStatistics {
        self.index.statistics()
    }

    /// Highlight search terms in text
    fn highlight_text(text: &str, terms: &[String]) -> String {
        let mut result = text.to_string();
        for term in terms {
            let highlight = format!("**{}**", term);
            result = result.replace(term, &highlight);
        }
        result
    }
}

/// Search result with highlighted text
#[derive(Debug, Clone)]
pub struct HighlightedResult {
    pub key: String,
    pub score: f64,
    pub highlighted_text: String,
    pub metadata: Option<crate::blobstore::BlobMetadata>,
}
