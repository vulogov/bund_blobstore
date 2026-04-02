use crate::blobstore::{BlobStore, QueryOptions};
use crate::serialization::{SerializationFormat, SerializationHelper};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Graph node structure
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub properties: HashMap<String, String>,
    pub timestamp: u64,
}

/// Graph edge structure
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub edge_type: String,
    pub weight: Option<f64>,
    pub properties: HashMap<String, String>,
    pub timestamp: u64,
}

/// Complete graph structure
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Graph {
    pub id: String,
    pub name: String,
    pub nodes: HashMap<String, GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub metadata: HashMap<String, String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Query options for graphs
#[derive(Debug, Clone)]
pub struct GraphQueryOptions {
    pub graph_id: Option<String>,
    pub node_type: Option<String>,
    pub edge_type: Option<String>,
    pub node_id: Option<String>,
    pub from_timestamp: Option<u64>,
    pub to_timestamp: Option<u64>,
    pub limit: Option<usize>,
}

impl Default for GraphQueryOptions {
    fn default() -> Self {
        Self {
            graph_id: None,
            node_type: None,
            edge_type: None,
            node_id: None,
            from_timestamp: None,
            to_timestamp: None,
            limit: None,
        }
    }
}

/// Graph store with specialized methods for graph operations
pub struct GraphStore {
    blob_store: BlobStore,
    serialization_format: SerializationFormat,
    compress: bool,
}

impl GraphStore {
    /// Create a new graph store
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, redb::Error> {
        Ok(GraphStore {
            blob_store: BlobStore::open(path)?,
            serialization_format: SerializationFormat::Bincode,
            compress: true,
        })
    }

    /// Create a new graph store with custom serialization settings
    pub fn open_with_format<P: AsRef<Path>>(
        path: P,
        format: SerializationFormat,
        compress: bool,
    ) -> Result<Self, redb::Error> {
        Ok(GraphStore {
            blob_store: BlobStore::open(path)?,
            serialization_format: format,
            compress,
        })
    }

    /// Save a complete graph
    pub fn save_graph(
        &mut self,
        graph: &Graph,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("graph:{}", graph.id);
        SerializationHelper::store_serialized(
            &mut self.blob_store,
            &key,
            graph,
            self.serialization_format,
            self.compress,
            Some("graph"),
        )?;

        // Also store index entries for efficient querying
        self.index_graph(graph)?;

        Ok(())
    }

    /// Load a complete graph by ID
    pub fn load_graph(
        &self,
        graph_id: &str,
    ) -> Result<Option<Graph>, Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("graph:{}", graph_id);
        SerializationHelper::load_deserialized(
            &self.blob_store,
            &key,
            self.serialization_format,
            self.compress,
        )
    }

    /// Store a node in a graph (without loading entire graph)
    pub fn store_node(
        &mut self,
        graph_id: &str,
        node: &GraphNode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("graph:{}:node:{}", graph_id, node.id);
        SerializationHelper::store_serialized(
            &mut self.blob_store,
            &key,
            node,
            self.serialization_format,
            self.compress,
            Some(&format!("graph:{}", graph_id)),
        )?;
        Ok(())
    }

    /// Load a specific node from a graph
    pub fn load_node(
        &self,
        graph_id: &str,
        node_id: &str,
    ) -> Result<Option<GraphNode>, Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("graph:{}:node:{}", graph_id, node_id);
        SerializationHelper::load_deserialized(
            &self.blob_store,
            &key,
            self.serialization_format,
            self.compress,
        )
    }

    /// Store an edge in a graph
    pub fn store_edge(
        &mut self,
        graph_id: &str,
        edge: &GraphEdge,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let key = format!("graph:{}:edge:{}:{}", graph_id, edge.from, edge.to);
        SerializationHelper::store_serialized(
            &mut self.blob_store,
            &key,
            edge,
            self.serialization_format,
            self.compress,
            Some(&format!("graph:{}", graph_id)),
        )?;
        Ok(())
    }

    /// Load all nodes from a graph
    pub fn load_all_nodes(
        &self,
        graph_id: &str,
    ) -> Result<Vec<GraphNode>, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = format!("graph:{}:node:", graph_id);
        let options = QueryOptions {
            prefix: Some(prefix),
            pattern: None,
            limit: None,
            offset: None,
        };

        let results = self.blob_store.query(options)?;
        let mut nodes = Vec::new();

        for (key, _metadata) in results {
            let node_id = key.replace(&format!("graph:{}:node:", graph_id), "");
            if let Some(node) = self.load_node(graph_id, &node_id)? {
                nodes.push(node);
            }
        }

        Ok(nodes)
    }

    /// Query graphs with complex filters
    pub fn query_graphs(
        &self,
        options: GraphQueryOptions,
    ) -> Result<Vec<Graph>, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = if let Some(ref graph_id) = options.graph_id {
            format!("graph:{}", graph_id)
        } else {
            "graph:".to_string()
        };

        let query_options = QueryOptions {
            prefix: Some(prefix),
            pattern: None,
            limit: options.limit,
            offset: None,
        };

        let results = self.blob_store.query(query_options)?;
        let mut graphs = Vec::new();

        for (key, _) in results {
            let graph_id = key.replace("graph:", "");
            if let Some(graph) = self.load_graph(&graph_id)? {
                graphs.push(graph);
            }
        }

        Ok(graphs)
    }

    /// Delete an entire graph
    pub fn delete_graph(
        &mut self,
        graph_id: &str,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let prefix = format!("graph:{}", graph_id);

        // Delete all keys with this prefix
        let keys = self.blob_store.list_keys()?;
        let mut deleted = false;

        for key in keys {
            if key.starts_with(&prefix) {
                if self.blob_store.remove(&key)? {
                    deleted = true;
                }
            }
        }

        Ok(deleted)
    }

    /// Get graph metadata without loading all data
    pub fn get_graph_metadata(
        &self,
        graph_id: &str,
    ) -> Result<Option<HashMap<String, String>>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(graph) = self.load_graph(graph_id)? {
            Ok(Some(graph.metadata))
        } else {
            Ok(None)
        }
    }

    /// Index a graph for efficient querying
    fn index_graph(
        &mut self,
        graph: &Graph,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store index by node type
        for node in graph.nodes.values() {
            let index_key = format!("index:node_type:{}:{}", node.node_type, node.id);
            let index_data = format!("{}:{}", graph.id, node.id);
            self.blob_store
                .put(&index_key, index_data.as_bytes(), Some("index"))?;
        }

        // Store index by edge type
        for edge in &graph.edges {
            let index_key = format!(
                "index:edge_type:{}:{}:{}",
                edge.edge_type, edge.from, edge.to
            );
            let index_data = format!("{}:{}:{}", graph.id, edge.from, edge.to);
            self.blob_store
                .put(&index_key, index_data.as_bytes(), Some("index"))?;
        }

        Ok(())
    }
}
