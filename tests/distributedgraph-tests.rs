use bund_blobstore::{
    DistributedGraphEdge, DistributedGraphManager, DistributedGraphNode, DistributedGraphQuery,
};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_distributed_graph() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let temp_dir = TempDir::new()?;
        let manager = DistributedGraphManager::new(temp_dir.path())?;

        // Add nodes
        let node1 = DistributedGraphNode {
            id: "node1".to_string(),
            node_type: "person".to_string(),
            properties: HashMap::new(),
            shard_id: "shard1".to_string(),
            timestamp: 1234567890,
            metadata: HashMap::new(),
        };

        let node2 = DistributedGraphNode {
            id: "node2".to_string(),
            node_type: "person".to_string(),
            properties: HashMap::new(),
            shard_id: "shard2".to_string(),
            timestamp: 1234567890,
            metadata: HashMap::new(),
        };

        manager.add_node(node1)?;
        manager.add_node(node2)?;

        // Add edge between nodes on different shards
        let edge = DistributedGraphEdge {
            id: "edge1".to_string(),
            from_node: "node1".to_string(),
            to_node: "node2".to_string(),
            from_shard: "shard1".to_string(),
            to_shard: "shard2".to_string(),
            edge_type: "knows".to_string(),
            weight: Some(1.0),
            properties: HashMap::new(),
            timestamp: 1234567890,
        };

        manager.add_edge(edge)?;

        // Query nodes
        let query = DistributedGraphQuery {
            node_type: Some("person".to_string()),
            ..Default::default()
        };

        let nodes = manager.query_nodes(&query)?;
        assert_eq!(nodes.len(), 2);

        // Get partitions
        let partitions = manager.get_partitions()?;
        assert!(!partitions.is_empty());

        Ok(())
    }
}
