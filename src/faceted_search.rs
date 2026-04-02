use crate::blobstore::{BlobMetadata, BlobStore};
use crate::serialization::{SerializationFormat, SerializationHelper};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Facet value and count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetValue {
    pub value: String,
    pub count: usize,
}

/// Facet category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Facet {
    pub name: String,
    pub values: Vec<FacetValue>,
}

/// Faceted search query
#[derive(Debug, Clone)]
pub struct FacetedQuery {
    pub text_query: Option<String>,
    pub facets: HashMap<String, HashSet<String>>,
    pub range_filters: HashMap<String, (f64, f64)>,
    pub limit: usize,
    pub offset: usize,
}

impl Default for FacetedQuery {
    fn default() -> Self {
        Self {
            text_query: None,
            facets: HashMap::new(),
            range_filters: HashMap::new(),
            limit: 20,
            offset: 0,
        }
    }
}

/// Faceted search result
#[derive(Debug, Clone)]
pub struct FacetedSearchResult {
    pub documents: Vec<FacetedDocument>,
    pub facets: Vec<Facet>,
    pub total: usize,
}

/// Document with facets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FacetedDocument {
    pub key: String,
    pub facets: HashMap<String, String>,
    pub numeric_facets: HashMap<String, f64>,
    pub content: Option<String>,
    pub metadata: Option<BlobMetadata>,
}

/// Faceted search index
pub struct FacetedSearchIndex {
    store: BlobStore,
    documents: HashMap<String, FacetedDocument>,
    facet_index: HashMap<String, HashMap<String, HashSet<String>>>,
    numeric_index: HashMap<String, HashMap<String, f64>>,
    serializer_format: SerializationFormat,
}

impl FacetedSearchIndex {
    pub fn new<P: AsRef<std::path::Path>>(
        path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let store = BlobStore::open(path)?;

        let mut index = FacetedSearchIndex {
            store,
            documents: HashMap::new(),
            facet_index: HashMap::new(),
            numeric_index: HashMap::new(),
            serializer_format: SerializationFormat::Bincode,
        };

        index.load()?;
        Ok(index)
    }

    pub fn add_document(
        &mut self,
        doc: FacetedDocument,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store document
        let serialized = SerializationHelper::serialize(&doc, self.serializer_format)?;
        self.store.put(&doc.key, &serialized, Some("faceted"))?;

        // Index facets
        for (facet_name, value) in &doc.facets {
            self.facet_index
                .entry(facet_name.clone())
                .or_insert_with(HashMap::new)
                .entry(value.clone())
                .or_insert_with(HashSet::new)
                .insert(doc.key.clone());
        }

        // Index numeric facets
        for (facet_name, value) in &doc.numeric_facets {
            self.numeric_index
                .entry(facet_name.clone())
                .or_insert_with(HashMap::new)
                .insert(doc.key.clone(), *value);
        }

        self.documents.insert(doc.key.clone(), doc);
        Ok(())
    }

    pub fn search(
        &self,
        query: &FacetedQuery,
    ) -> Result<FacetedSearchResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut doc_scores: HashMap<String, f64> = HashMap::new();

        // Filter by text query (simplified - would integrate with full-text search)
        if let Some(text) = &query.text_query {
            for (key, doc) in &self.documents {
                if let Some(content) = &doc.content {
                    if content.to_lowercase().contains(&text.to_lowercase()) {
                        *doc_scores.entry(key.clone()).or_insert(0.0) += 1.0;
                    }
                }
            }
        } else {
            for key in self.documents.keys() {
                doc_scores.insert(key.clone(), 1.0);
            }
        }

        // Filter by facet selections
        for (facet_name, selected_values) in &query.facets {
            if let Some(_facet_index) = self.facet_index.get(facet_name) {
                doc_scores.retain(|key, _| {
                    if let Some(doc) = self.documents.get(key) {
                        if let Some(doc_value) = doc.facets.get(facet_name) {
                            return selected_values.contains(doc_value);
                        }
                    }
                    false
                });
            }
        }

        // Filter by numeric ranges
        for (facet_name, (min, max)) in &query.range_filters {
            if let Some(numeric_index) = self.numeric_index.get(facet_name) {
                doc_scores.retain(|key, _| {
                    if let Some(&value) = numeric_index.get(key) {
                        return value >= *min && value <= *max;
                    }
                    false
                });
            }
        }

        // Calculate facet counts
        let mut facets = Vec::new();
        for (facet_name, index) in &self.facet_index {
            let mut values = Vec::new();
            for (value, docs) in index {
                let count = docs
                    .intersection(&doc_scores.keys().cloned().collect())
                    .count();
                if count > 0 {
                    values.push(FacetValue {
                        value: value.clone(),
                        count,
                    });
                }
            }
            values.sort_by(|a, b| b.count.cmp(&a.count));
            facets.push(Facet {
                name: facet_name.clone(),
                values,
            });
        }

        // Get paginated results
        let documents: Vec<FacetedDocument> = doc_scores
            .keys()
            .skip(query.offset)
            .take(query.limit)
            .filter_map(|key| self.documents.get(key).cloned())
            .collect();

        Ok(FacetedSearchResult {
            documents,
            facets,
            total: doc_scores.len(),
        })
    }

    pub fn load(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let all_data = self.store.get_all()?;
        for (key, data) in all_data {
            if key.starts_with("faceted:") {
                if let Ok(doc) = SerializationHelper::deserialize::<FacetedDocument>(
                    &data,
                    self.serializer_format,
                ) {
                    self.documents.insert(key, doc);
                }
            }
        }
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Documents are saved individually in add_document
        Ok(())
    }
}
