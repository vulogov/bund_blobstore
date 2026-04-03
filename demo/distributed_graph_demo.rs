use bund_blobstore::{
    DistributedGraphEdge, DistributedGraphManager, DistributedGraphNode, DistributedGraphQuery,
};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let manager = DistributedGraphManager::new("distributed_graph")?;

    // Add nodes that will be automatically distributed across shards
    let node1 = DistributedGraphNode {
        id: "user_001".to_string(),
        node_type: "user".to_string(),
        properties: {
            let mut map = HashMap::new();
            map.insert("name".to_string(), "Alice".to_string());
            map
        },
        shard_id: "shard1".to_string(),
        timestamp: 1234567890,
        metadata: HashMap::new(),
    };
    manager.add_node(node1)?;

    let node2 = DistributedGraphNode {
        id: "user_002".to_string(),
        node_type: "user".to_string(),
        properties: {
            let mut map = HashMap::new();
            map.insert("name".to_string(), "Bob".to_string());
            map
        },
        shard_id: "shard2".to_string(),
        timestamp: 1234567890,
        metadata: HashMap::new(),
    };
    manager.add_node(node2)?;

    // Add edge between nodes on different shards
    let edge = DistributedGraphEdge {
        id: "friendship_001".to_string(),
        from_node: "user_001".to_string(),
        to_node: "user_002".to_string(),
        from_shard: "shard1".to_string(),
        to_shard: "shard2".to_string(),
        edge_type: "friend".to_string(),
        weight: Some(1.0),
        properties: HashMap::new(),
        timestamp: 1234567890,
    };
    manager.add_edge(edge)?;

    // Traverse the graph
    let query = DistributedGraphQuery {
        node_type: Some("user".to_string()),
        traverse_depth: Some(3),
        limit: 10,
        ..Default::default()
    };

    let results = manager.traverse("user_001", &query)?;
    for result in results {
        println!("Path: {:?}, Weight: {}", result.path, result.total_weight);
    }

    // Find shortest path
    if let Some(path) = manager.shortest_path("user_001", "user_002")? {
        println!("Shortest path: {:?}", path.path);
    }

    // Get partition information
    let partitions = manager.get_partitions()?;
    for partition in partitions {
        println!(
            "Shard {}: {} nodes, {} edges",
            partition.shard_id, partition.node_count, partition.edge_count
        );
    }

    Ok(())
}
