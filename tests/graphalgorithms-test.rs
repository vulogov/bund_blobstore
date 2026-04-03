use bund_blobstore::GraphAlgorithms;
use bund_blobstore::distributed_graph;
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_graph() -> Result<GraphAlgorithms, Box<dyn std::error::Error + Send + Sync>> {
        let temp_dir = TempDir::new()?;
        let manager = Arc::new(crate::distributed_graph::DistributedGraphManager::new(
            temp_dir.path(),
        )?);

        // Create nodes
        let nodes = vec!["A", "B", "C", "D", "E"];
        for (i, node_id) in nodes.iter().enumerate() {
            let node = crate::distributed_graph::DistributedGraphNode {
                id: node_id.to_string(),
                node_type: "test".to_string(),
                properties: HashMap::new(),
                shard_id: format!("shard_{}", i % 3),
                timestamp: 1234567890,
                metadata: HashMap::new(),
            };
            manager.add_node(node)?;
        }

        // Create edges (A->B, B->C, C->D, D->E, and a cycle B->A)
        let edges = vec![
            ("A", "B", 1.0),
            ("B", "C", 2.0),
            ("C", "D", 3.0),
            ("D", "E", 4.0),
            ("B", "A", 1.0), // Creates a cycle
        ];

        for (from, to, weight) in edges {
            let edge = crate::distributed_graph::DistributedGraphEdge {
                id: format!("{}_{}", from, to),
                from_node: from.to_string(),
                to_node: to.to_string(),
                from_shard: format!("shard_{}", from.as_bytes()[0] as usize % 3),
                to_shard: format!("shard_{}", to.as_bytes()[0] as usize % 3),
                edge_type: "test_edge".to_string(),
                weight: Some(weight),
                properties: HashMap::new(),
                timestamp: 1234567890,
            };
            manager.add_edge(edge)?;
        }

        Ok(GraphAlgorithms::new(manager))
    }

    #[test]
    fn test_cycle_detection() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let algorithms = create_test_graph()?;
        let result = algorithms.detect_cycles(None)?;

        assert!(result.has_cycle);
        assert!(result.cycle_count > 0);

        Ok(())
    }

    #[test]
    fn test_shortest_path() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let algorithms = create_test_graph()?;
        let path = algorithms.shortest_path_optimized("A", "E", false)?;

        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.path.contains(&"A".to_string()));
        assert!(path.path.contains(&"E".to_string()));

        Ok(())
    }

    #[test]
    fn test_bidirectional_search() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let algorithms = create_test_graph()?;
        let path = algorithms.bidirectional_search("A", "E")?;

        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.path.contains(&"A".to_string()));
        assert!(path.path.contains(&"E".to_string()));

        Ok(())
    }
}
