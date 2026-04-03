use crate::sharding::ShardManager;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

/// Distributed graph node with shard location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedGraphNode {
    pub id: String,
    pub node_type: String,
    pub properties: HashMap<String, String>,
    pub shard_id: String,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

/// Distributed graph edge with source and target shards
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedGraphEdge {
    pub id: String,
    pub from_node: String,
    pub to_node: String,
    pub from_shard: String,
    pub to_shard: String,
    pub edge_type: String,
    pub weight: Option<f64>,
    pub properties: HashMap<String, String>,
    pub timestamp: u64,
}

/// Graph partition information
#[derive(Debug, Clone)]
pub struct GraphPartition {
    pub shard_id: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub nodes: HashSet<String>,
}

/// Query for distributed graph
#[derive(Debug, Clone)]
pub struct DistributedGraphQuery {
    pub node_type: Option<String>,
    pub edge_type: Option<String>,
    pub node_ids: Option<Vec<String>>,
    pub properties: Option<HashMap<String, String>>,
    pub limit: usize,
    pub offset: usize,
    pub traverse_depth: Option<usize>,
}

impl Default for DistributedGraphQuery {
    fn default() -> Self {
        DistributedGraphQuery {
            node_type: None,
            edge_type: None,
            node_ids: None,
            properties: None,
            limit: 100,
            offset: 0,
            traverse_depth: None,
        }
    }
}

/// Graph traversal result
#[derive(Debug, Clone)]
pub struct GraphTraversalResult {
    pub path: Vec<String>,
    pub total_weight: f64,
    pub nodes: Vec<DistributedGraphNode>,
    pub edges: Vec<DistributedGraphEdge>,
}

/// Distributed graph manager that spans multiple shards
pub struct DistributedGraphManager {
    shard_manager: Arc<ShardManager>,
    node_index: Arc<RwLock<HashMap<String, (String, DistributedGraphNode)>>>,
    edge_index: Arc<RwLock<HashMap<String, (String, DistributedGraphEdge)>>>,
    reverse_edge_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    type_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl DistributedGraphManager {
    /// Create a new distributed graph manager
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Create shard manager with consistent hashing for graph distribution
        let shard_manager = Arc::new(
            crate::sharding::ShardManagerBuilder::new()
                .with_strategy(crate::sharding::ShardingStrategy::ConsistentHash)
                .add_shard(
                    "graph_shard_1",
                    path.as_ref().join("graph_shard_1").to_str().unwrap(),
                )
                .add_shard(
                    "graph_shard_2",
                    path.as_ref().join("graph_shard_2").to_str().unwrap(),
                )
                .add_shard(
                    "graph_shard_3",
                    path.as_ref().join("graph_shard_3").to_str().unwrap(),
                )
                .build()?,
        );

        Ok(DistributedGraphManager {
            shard_manager,
            node_index: Arc::new(RwLock::new(HashMap::new())),
            edge_index: Arc::new(RwLock::new(HashMap::new())),
            reverse_edge_index: Arc::new(RwLock::new(HashMap::new())),
            type_index: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create with custom shard manager
    pub fn with_shard_manager(shard_manager: Arc<ShardManager>) -> Self {
        DistributedGraphManager {
            shard_manager,
            node_index: Arc::new(RwLock::new(HashMap::new())),
            edge_index: Arc::new(RwLock::new(HashMap::new())),
            reverse_edge_index: Arc::new(RwLock::new(HashMap::new())),
            type_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a node to the distributed graph
    pub fn add_node(
        &self,
        node: DistributedGraphNode,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let shard = self.shard_manager.get_shard_for_key(&node.id);
        let node_key = format!("graph_node:{}", node.id);

        // Store node in its assigned shard
        let serialized = serde_json::to_vec(&node)?;
        shard
            .blob()
            .put(&node_key, &serialized, Some("graph_nodes"))?;

        // Update indexes
        self.node_index
            .write()
            .insert(node.id.clone(), (node.shard_id.clone(), node.clone()));
        self.type_index
            .write()
            .entry(node.node_type.clone())
            .or_insert_with(Vec::new)
            .push(node.id.clone());

        Ok(())
    }

    /// Add an edge between nodes that may be on different shards
    pub fn add_edge(
        &self,
        edge: DistributedGraphEdge,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Store edge in source node's shard
        let from_shard = self.shard_manager.get_shard_for_key(&edge.from_node);
        let edge_key = format!("graph_edge:{}:{}", edge.from_node, edge.to_node);

        let serialized = serde_json::to_vec(&edge)?;
        from_shard
            .blob()
            .put(&edge_key, &serialized, Some("graph_edges"))?;

        // Update indexes
        self.edge_index
            .write()
            .insert(edge.id.clone(), (edge.from_shard.clone(), edge.clone()));
        self.reverse_edge_index
            .write()
            .entry(edge.to_node.clone())
            .or_insert_with(Vec::new)
            .push(edge.id.clone());

        Ok(())
    }

    /// Get a node by ID (may fetch from its shard)
    pub fn get_node(
        &self,
        node_id: &str,
    ) -> Result<Option<DistributedGraphNode>, Box<dyn std::error::Error + Send + Sync>> {
        // Check cache first
        if let Some((_, node)) = self.node_index.read().get(node_id) {
            return Ok(Some(node.clone()));
        }

        // Fetch from shard
        let shard = self.shard_manager.get_shard_for_key(node_id);
        let node_key = format!("graph_node:{}", node_id);

        if let Some(data) = shard.blob().get(&node_key)? {
            let node: DistributedGraphNode = serde_json::from_slice(&data)?;
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    /// Get edges from a node (may traverse across shards)
    pub fn get_outgoing_edges(
        &self,
        node_id: &str,
    ) -> Result<Vec<DistributedGraphEdge>, Box<dyn std::error::Error + Send + Sync>> {
        let shard = self.shard_manager.get_shard_for_key(node_id);
        let prefix = format!("graph_edge:{}:", node_id);

        let all_keys = shard.blob().list_keys()?;
        let mut edges = Vec::new();

        for key in all_keys {
            if key.starts_with(&prefix) {
                if let Some(data) = shard.blob().get(&key)? {
                    let edge: DistributedGraphEdge = serde_json::from_slice(&data)?;
                    edges.push(edge);
                }
            }
        }

        Ok(edges)
    }

    /// Traverse the graph from a starting node
    pub fn traverse(
        &self,
        start_node: &str,
        query: &DistributedGraphQuery,
    ) -> Result<Vec<GraphTraversalResult>, Box<dyn std::error::Error + Send + Sync>> {
        let max_depth = query.traverse_depth.unwrap_or(3);
        let mut results = Vec::new();
        let mut visited = HashSet::new();

        self.traverse_recursive(
            start_node,
            &mut Vec::new(),
            0.0,
            max_depth,
            &mut visited,
            &mut results,
            query,
        )?;

        results.sort_by(|a, b| b.total_weight.partial_cmp(&a.total_weight).unwrap());
        results.truncate(query.limit);

        Ok(results)
    }

    fn traverse_recursive(
        &self,
        current_node: &str,
        path: &mut Vec<String>,
        current_weight: f64,
        max_depth: usize,
        visited: &mut HashSet<String>,
        results: &mut Vec<GraphTraversalResult>,
        query: &DistributedGraphQuery,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if path.len() > max_depth || visited.contains(current_node) {
            return Ok(());
        }

        visited.insert(current_node.to_string());
        path.push(current_node.to_string());

        // Get current node
        if let Some(node) = self.get_node(current_node)? {
            // Apply filters
            let mut matches = true;

            if let Some(ref node_type) = query.node_type {
                if node.node_type != *node_type {
                    matches = false;
                }
            }

            if let Some(ref properties) = query.properties {
                for (k, v) in properties {
                    if node.properties.get(k) != Some(v) {
                        matches = false;
                        break;
                    }
                }
            }

            if matches && path.len() > 1 {
                // Get all edges along the path
                let mut edges = Vec::new();
                for i in 0..path.len() - 1 {
                    if let Ok(node_edges) = self.get_outgoing_edges(&path[i]) {
                        for edge in node_edges {
                            if edge.to_node == path[i + 1] {
                                edges.push(edge);
                                break;
                            }
                        }
                    }
                }

                results.push(GraphTraversalResult {
                    path: path.clone(),
                    total_weight: current_weight,
                    nodes: vec![node],
                    edges,
                });
            }

            // Continue traversal
            let edges = self.get_outgoing_edges(current_node)?;
            for edge in edges {
                if let Some(ref edge_type) = query.edge_type {
                    if edge.edge_type != *edge_type {
                        continue;
                    }
                }

                let new_weight = current_weight + edge.weight.unwrap_or(1.0);
                self.traverse_recursive(
                    &edge.to_node,
                    path,
                    new_weight,
                    max_depth,
                    visited,
                    results,
                    query,
                )?;
            }
        }

        path.pop();
        visited.remove(current_node);

        Ok(())
    }

    /// Query nodes across all shards
    pub fn query_nodes(
        &self,
        query: &DistributedGraphQuery,
    ) -> Result<Vec<DistributedGraphNode>, Box<dyn std::error::Error + Send + Sync>> {
        let mut all_nodes = Vec::new();

        // Use type index for efficient filtering
        if let Some(ref node_type) = query.node_type {
            if let Some(node_ids) = self.type_index.read().get(node_type) {
                for node_id in node_ids {
                    if let Some(node) = self.get_node(node_id)? {
                        all_nodes.push(node);
                    }
                }
            }
        } else if let Some(ref node_ids) = query.node_ids {
            for node_id in node_ids {
                if let Some(node) = self.get_node(node_id)? {
                    all_nodes.push(node);
                }
            }
        } else {
            // Scan all shards
            let shard_manager = self.shard_manager.clone();
            all_nodes = shard_manager.query_all_shards(|shard| {
                let all_keys = shard.blob().list_keys()?;
                let mut nodes = Vec::new();

                for key in all_keys {
                    if key.starts_with("graph_node:") {
                        if let Some(data) = shard.blob().get(&key)? {
                            if let Ok(node) = serde_json::from_slice::<DistributedGraphNode>(&data)
                            {
                                nodes.push(node);
                            }
                        }
                    }
                }

                Ok(nodes)
            })?;
        }

        // Apply property filters
        if let Some(ref properties) = query.properties {
            all_nodes.retain(|node| {
                for (k, v) in properties {
                    if node.properties.get(k) != Some(v) {
                        return false;
                    }
                }
                true
            });
        }

        // Apply pagination
        let start = query.offset.min(all_nodes.len());
        let end = (start + query.limit).min(all_nodes.len());

        Ok(all_nodes[start..end].to_vec())
    }

    /// Get graph partitions information
    pub fn get_partitions(
        &self,
    ) -> Result<Vec<GraphPartition>, Box<dyn std::error::Error + Send + Sync>> {
        let mut partitions: HashMap<String, GraphPartition> = HashMap::new();

        let all_nodes = self.query_nodes(&DistributedGraphQuery::default())?;

        for node in all_nodes {
            let partition = partitions
                .entry(node.shard_id.clone())
                .or_insert(GraphPartition {
                    shard_id: node.shard_id.clone(),
                    node_count: 0,
                    edge_count: 0,
                    nodes: HashSet::new(),
                });
            partition.node_count += 1;
            partition.nodes.insert(node.id);
        }

        // Count edges per partition
        for (shard_id, partition) in partitions.iter_mut() {
            let shard = self.shard_manager.get_shard_for_key(shard_id);
            let all_keys = shard.blob().list_keys()?;

            for key in all_keys {
                if key.starts_with("graph_edge:") {
                    partition.edge_count += 1;
                }
            }
        }

        Ok(partitions.into_values().collect())
    }

    /// Get shard statistics
    pub fn shard_statistics(&self) -> crate::sharding::ShardStatistics {
        self.shard_manager.shard_statistics()
    }

    /// Find shortest path between two nodes using Dijkstra across shards
    pub fn shortest_path(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<GraphTraversalResult>, Box<dyn std::error::Error + Send + Sync>> {
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;

        #[derive(Clone, PartialEq)]
        struct State {
            node: String,
            cost: f64,
            path: Vec<String>,
        }

        impl Eq for State {}

        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                other.cost.partial_cmp(&self.cost).unwrap()
            }
        }

        let mut heap = BinaryHeap::new();
        let mut distances: HashMap<String, f64> = HashMap::new();

        heap.push(State {
            node: from.to_string(),
            cost: 0.0,
            path: vec![from.to_string()],
        });
        distances.insert(from.to_string(), 0.0);

        while let Some(State { node, cost, path }) = heap.pop() {
            if node == to {
                // Build result
                let mut nodes = Vec::new();
                let mut edges = Vec::new();

                for i in 0..path.len() - 1 {
                    if let Some(node_data) = self.get_node(&path[i])? {
                        nodes.push(node_data);
                    }
                    if let Some(edge) = self.get_edge(&path[i], &path[i + 1])? {
                        edges.push(edge);
                    }
                }
                if let Some(last_node) = self.get_node(to)? {
                    nodes.push(last_node);
                }

                return Ok(Some(GraphTraversalResult {
                    path,
                    total_weight: cost,
                    nodes,
                    edges,
                }));
            }

            if cost > distances[&node] {
                continue;
            }

            let outgoing = self.get_outgoing_edges(&node)?;
            for edge in outgoing {
                let next = edge.to_node.clone();
                let next_cost = cost + edge.weight.unwrap_or(1.0);

                if next_cost < *distances.get(&next).unwrap_or(&f64::INFINITY) {
                    distances.insert(next.clone(), next_cost);
                    let mut new_path = path.clone();
                    new_path.push(next.clone());
                    heap.push(State {
                        node: next,
                        cost: next_cost,
                        path: new_path,
                    });
                }
            }
        }

        Ok(None)
    }

    fn get_edge(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<DistributedGraphEdge>, Box<dyn std::error::Error + Send + Sync>> {
        let shard = self.shard_manager.get_shard_for_key(from);
        let edge_key = format!("graph_edge:{}:{}", from, to);

        if let Some(data) = shard.blob().get(&edge_key)? {
            let edge: DistributedGraphEdge = serde_json::from_slice(&data)?;
            Ok(Some(edge))
        } else {
            Ok(None)
        }
    }
}
