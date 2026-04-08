use crate::blobstore::{BlobMetadata, BlobStore};
use crate::serialization::{SerializationFormat, SerializationHelper};
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use strsim::{damerau_levenshtein, levenshtein};

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
    pub metadata: Option<BlobMetadata>,
}

/// Fuzzy search configuration
#[derive(Debug, Clone)]
pub struct FuzzyConfig {
    pub max_distance: usize,
    pub max_edits: usize,
    pub prefix_length: usize,
    pub use_damerau: bool,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        FuzzyConfig {
            max_distance: 3,
            max_edits: 2,
            prefix_length: 3,
            use_damerau: false,
        }
    }
}

/// Fuzzy search result
#[derive(Debug, Clone)]
pub struct FuzzySearchResult {
    pub key: String,
    pub term: String,
    pub distance: usize,
    pub score: f64,
    pub metadata: Option<BlobMetadata>,
}

/// Full-text search index
pub struct FullTextIndex {
    inverted_index: Arc<RwLock<HashMap<String, HashMap<String, usize>>>>,
    tokenizer_options: TokenizerOptions,
    index_serializer: SerializationFormat,
}

impl FullTextIndex {
    pub fn new(options: TokenizerOptions) -> Self {
        FullTextIndex {
            inverted_index: Arc::new(RwLock::new(HashMap::new())),
            tokenizer_options: options,
            index_serializer: SerializationFormat::Bincode,
        }
    }

    pub fn index_document(&self, key: &str, content: &str) {
        let tokens = self.tokenize(content);
        let mut index = self.inverted_index.write();

        let mut term_freq: HashMap<String, usize> = HashMap::new();
        for token in tokens {
            *term_freq.entry(token).or_insert(0) += 1;
        }

        for (term, freq) in term_freq {
            let doc_map = index.entry(term).or_insert_with(HashMap::new);
            *doc_map.entry(key.to_string()).or_insert(0) += freq;
        }
    }

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

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_tokens = self.tokenize(query);
        let index = self.inverted_index.read();

        let mut scores: HashMap<String, f64> = HashMap::new();
        let mut matches: HashMap<String, Vec<String>> = HashMap::new();

        for token in query_tokens {
            if let Some(docs) = index.get(&token) {
                for (doc, freq) in docs {
                    let score = *freq as f64;
                    *scores.entry(doc.clone()).or_insert(0.0) += score;
                    matches
                        .entry(doc.clone())
                        .or_insert_with(Vec::new)
                        .push(token.clone());
                }
            }
        }

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

    pub fn fuzzy_search(
        &self,
        query: &str,
        config: &FuzzyConfig,
        limit: usize,
    ) -> Vec<FuzzySearchResult> {
        let query_tokens = self.tokenize(query);
        let index = self.inverted_index.read();
        let mut results = Vec::new();

        for query_token in query_tokens {
            if let Some(docs) = index.get(&query_token) {
                for doc in docs.keys() {
                    results.push(FuzzySearchResult {
                        key: doc.clone(),
                        term: query_token.clone(),
                        distance: 0,
                        score: 1.0,
                        metadata: None,
                    });
                }
            }

            for (term, docs) in index.iter() {
                let distance = if config.use_damerau {
                    damerau_levenshtein(&query_token, term)
                } else {
                    levenshtein(&query_token, term)
                };

                if distance <= config.max_distance && distance > 0 {
                    if config.prefix_length > 0 {
                        let prefix_match = query_token.len() >= config.prefix_length
                            && term.len() >= config.prefix_length
                            && &query_token[..config.prefix_length]
                                == &term[..config.prefix_length];

                        if !prefix_match {
                            continue;
                        }
                    }

                    let score = 1.0 - (distance as f64 / config.max_distance as f64);

                    for doc in docs.keys() {
                        results.push(FuzzySearchResult {
                            key: doc.clone(),
                            term: term.clone(),
                            distance,
                            score,
                            metadata: None,
                        });
                    }
                }
            }
        }

        let mut unique_results: HashMap<String, FuzzySearchResult> = HashMap::new();
        for result in results {
            let entry = unique_results
                .entry(result.key.clone())
                .or_insert(result.clone());
            if result.score > entry.score {
                *entry = result;
            }
        }

        let mut final_results: Vec<FuzzySearchResult> = unique_results.into_values().collect();
        final_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        final_results.truncate(limit);

        final_results
    }

    pub fn save(
        &self,
        store: &mut BlobStore,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let index = self.inverted_index.read();
        let serialized = SerializationHelper::serialize(&*index, self.index_serializer)?;
        store.put("__search_index__", &serialized, Some("__system__"))?;
        Ok(())
    }

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

    pub fn tokenize_text(&self, text: &str) -> Vec<String> {
        self.tokenize(text)
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let processed_text = if self.tokenizer_options.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };

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

    fn stem_word(&self, word: &str) -> String {
        let word = word.to_lowercase();

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
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = BlobStore::open(path)?;
        let index = FullTextIndex::load(&store, TokenizerOptions::default())?;

        Ok(SearchableBlobStore {
            store,
            index,
            auto_index: true,
        })
    }

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

    /// Optimize the underlying storage
    pub fn optimize(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.store.optimize()?;
        Ok(())
    }

    /// Sync the underlying storage
    pub fn sync(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.store.sync()?;
        Ok(())
    }

    pub fn put(&mut self, key: &str, data: &[u8], prefix: Option<&str>) -> Result<(), redb::Error> {
        self.store.put(key, data, prefix)?;

        if self.auto_index {
            if let Ok(text) = std::str::from_utf8(data) {
                self.index.index_document(key, text);
            }
        }

        Ok(())
    }

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

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        self.store.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Result<bool, redb::Error> {
        let removed = self.store.remove(key)?;
        if removed {
            self.index.remove_document(key);
        }
        Ok(removed)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>, redb::Error> {
        let mut results = self.index.search(query, limit);

        for result in &mut results {
            if let Ok(metadata) = self.store.get_metadata(&result.key) {
                result.metadata = metadata;
            }
        }

        Ok(results)
    }

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

    pub fn fuzzy_search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FuzzySearchResult>, redb::Error> {
        let config = FuzzyConfig::default();
        let mut results = self.index.fuzzy_search(query, &config, limit);

        for result in &mut results {
            if let Ok(metadata) = self.store.get_metadata(&result.key) {
                result.metadata = metadata;
            }
        }

        Ok(results)
    }

    pub fn fuzzy_search_with_config(
        &self,
        query: &str,
        config: &FuzzyConfig,
        limit: usize,
    ) -> Result<Vec<FuzzySearchResult>, redb::Error> {
        let mut results = self.index.fuzzy_search(query, config, limit);

        for result in &mut results {
            if let Ok(metadata) = self.store.get_metadata(&result.key) {
                result.metadata = metadata;
            }
        }

        Ok(results)
    }

    pub fn fuzzy_search_damerau(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FuzzySearchResult>, redb::Error> {
        let mut config = FuzzyConfig::default();
        config.use_damerau = true;
        self.fuzzy_search_with_config(query, &config, limit)
    }

    pub fn save_index(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.index.save(&mut self.store)
    }

    pub fn reindex(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.index = FullTextIndex::new(TokenizerOptions::default());

        let all_data = self.store.get_all()?;
        for (key, data) in all_data {
            if let Ok(text) = String::from_utf8(data) {
                self.index.index_document(&key, &text);
            }
        }

        self.save_index()?;
        Ok(())
    }

    pub fn set_auto_index(&mut self, enabled: bool) {
        self.auto_index = enabled;
    }

    pub fn index_stats(&self) -> IndexStatistics {
        self.index.statistics()
    }

    fn highlight_text(text: &str, terms: &[String]) -> String {
        let mut result = text.to_string();
        for term in terms {
            let highlight = format!("**{}**", term);
            result = result.replace(term, &highlight);
        }
        result
    }

    /// Phrase search - find exact phrase matches
    pub fn search_phrase(
        &self,
        phrase: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, redb::Error> {
        let phrase_lower = if self.index.tokenizer_options.case_sensitive {
            phrase.to_string()
        } else {
            phrase.to_lowercase()
        };

        let results = self.search(&phrase_lower, limit * 2)?;
        let mut phrase_results = Vec::new();

        for result in results {
            if let Some(data) = self.get(&result.key)? {
                if let Ok(text) = String::from_utf8(data) {
                    let text_lower = if self.index.tokenizer_options.case_sensitive {
                        text
                    } else {
                        text.to_lowercase()
                    };

                    // Check for exact phrase match
                    if text_lower.contains(&phrase_lower) {
                        // Calculate phrase relevance score
                        let occurrences = text_lower.matches(&phrase_lower).count();
                        let phrase_score = (occurrences as f64) * 10.0; // Boost phrase matches

                        let mut enhanced_result = result.clone();
                        enhanced_result.score += phrase_score;
                        phrase_results.push(enhanced_result);
                    }
                }
            }
        }

        phrase_results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        phrase_results.truncate(limit);

        Ok(phrase_results)
    }

    /// Proximity search - find words within n words of each other
    pub fn search_proximity(
        &self,
        word1: &str,
        word2: &str,
        distance: usize,
        limit: usize,
    ) -> Result<Vec<SearchResult>, redb::Error> {
        let results = self.search(&format!("{} {}", word1, word2), limit * 2)?;
        let mut proximity_results = Vec::new();

        for result in results {
            if let Some(data) = self.get(&result.key)? {
                if let Ok(text) = String::from_utf8(data) {
                    let words: Vec<&str> = text.split_whitespace().collect();
                    let mut positions1 = Vec::new();
                    let mut positions2 = Vec::new();

                    for (i, word) in words.iter().enumerate() {
                        if word.to_lowercase() == word1.to_lowercase() {
                            positions1.push(i);
                        }
                        if word.to_lowercase() == word2.to_lowercase() {
                            positions2.push(i);
                        }
                    }

                    let mut found = false;
                    for &pos1 in &positions1 {
                        for &pos2 in &positions2 {
                            if pos1.abs_diff(pos2) <= distance {
                                found = true;
                                break;
                            }
                        }
                        if found {
                            break;
                        }
                    }

                    if found {
                        proximity_results.push(result.clone());
                    }
                }
            }
        }

        proximity_results.truncate(limit);
        Ok(proximity_results)
    }
}

/// Search result with highlighted text
#[derive(Debug, Clone)]
pub struct HighlightedResult {
    pub key: String,
    pub score: f64,
    pub highlighted_text: String,
    pub metadata: Option<BlobMetadata>,
}

/// Trie data structure for efficient fuzzy search
pub struct FuzzyTrie {
    root: TrieNode,
}

#[derive(Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    is_end: bool,
    terms: HashSet<String>,
}

impl FuzzyTrie {
    pub fn new() -> Self {
        FuzzyTrie {
            root: TrieNode::default(),
        }
    }

    pub fn insert(&mut self, term: &str) {
        let mut node = &mut self.root;
        for ch in term.chars() {
            node = node.children.entry(ch).or_insert_with(TrieNode::default);
        }
        node.is_end = true;
        node.terms.insert(term.to_string());
    }

    pub fn search(&self, query: &str, max_distance: usize) -> Vec<(String, usize)> {
        let mut results = Vec::new();
        let query_chars: Vec<char> = query.chars().collect();

        self.search_recursive(&self.root, &query_chars, 0, 0, max_distance, &mut results);

        results.sort_by(|a, b| a.1.cmp(&b.1));
        results
    }

    fn search_recursive(
        &self,
        node: &TrieNode,
        query: &[char],
        pos: usize,
        dist: usize,
        max_distance: usize,
        results: &mut Vec<(String, usize)>,
    ) {
        if dist > max_distance {
            return;
        }

        if pos == query.len() {
            if node.is_end {
                for term in &node.terms {
                    results.push((term.clone(), dist));
                }
            }
            return;
        }

        for (&ch, child) in &node.children {
            let new_dist = if ch == query[pos] { dist } else { dist + 1 };
            self.search_recursive(child, query, pos + 1, new_dist, max_distance, results);
        }

        self.search_recursive(node, query, pos + 1, dist + 1, max_distance, results);

        for child in node.children.values() {
            self.search_recursive(child, query, pos, dist + 1, max_distance, results);
        }

        if pos + 1 < query.len() {
            for (&ch1, child1) in &node.children {
                if ch1 == query[pos + 1] {
                    for (&ch2, child2) in &child1.children {
                        if ch2 == query[pos] {
                            self.search_recursive(
                                child2,
                                query,
                                pos + 2,
                                dist + 1,
                                max_distance,
                                results,
                            );
                        }
                    }
                }
            }
        }
    }
}
