use bund_blobstore::{Graph, GraphNode, GraphStore};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_graph() -> Graph {
        let mut nodes = HashMap::new();
        nodes.insert(
            "node1".to_string(),
            GraphNode {
                id: "node1".to_string(),
                node_type: "person".to_string(),
                properties: HashMap::new(),
                timestamp: 1234567890,
            },
        );

        Graph {
            id: "test_graph".to_string(),
            name: "Test Graph".to_string(),
            nodes,
            edges: vec![],
            metadata: HashMap::new(),
            created_at: 1234567890,
            updated_at: 1234567890,
        }
    }

    #[test]
    fn test_save_and_load_graph() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = GraphStore::open(temp_file.path())?;

        let graph = create_test_graph();
        store.save_graph(&graph)?;

        let loaded = store.load_graph("test_graph")?;
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, graph.id);

        Ok(())
    }

    #[test]
    fn test_store_and_load_node() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let temp_file = NamedTempFile::new()?;
        let mut store = GraphStore::open(temp_file.path())?;

        let node = GraphNode {
            id: "node1".to_string(),
            node_type: "person".to_string(),
            properties: HashMap::new(),
            timestamp: 1234567890,
        };

        store.store_node("graph1", &node)?;

        let loaded = store.load_node("graph1", "node1")?;
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().id, node.id);

        Ok(())
    }
}
