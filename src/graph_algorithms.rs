use crate::distributed_graph::{
    DistributedGraphEdge, DistributedGraphManager, DistributedGraphNode, DistributedGraphQuery,
    GraphTraversalResult,
};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

/// Result of cycle detection
#[derive(Debug, Clone)]
pub struct CycleDetectionResult {
    pub has_cycle: bool,
    pub cycles: Vec<Vec<String>>,
    pub cycle_count: usize,
    pub affected_nodes: Vec<String>,
}

/// Result of longest path search
#[derive(Debug, Clone)]
pub struct LongestPathResult {
    pub path: Vec<String>,
    pub total_weight: f64,
    pub nodes: Vec<DistributedGraphNode>,
    pub edges: Vec<DistributedGraphEdge>,
    pub length: usize,
}

/// Graph algorithms for distributed graphs
pub struct GraphAlgorithms {
    graph_manager: Arc<DistributedGraphManager>,
}

impl GraphAlgorithms {
    pub fn new(graph_manager: Arc<DistributedGraphManager>) -> Self {
        GraphAlgorithms { graph_manager }
    }

    /// Detect cycles in the distributed graph
    pub fn detect_cycles(
        &self,
        start_node: Option<&str>,
    ) -> Result<CycleDetectionResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut visited = HashSet::new();
        let mut recursion_stack = HashSet::new();
        let mut cycles = Vec::new();
        let mut affected_nodes = Vec::new();

        // Get all nodes or start from specific node
        let query = DistributedGraphQuery {
            limit: 10000,
            ..Default::default()
        };

        let all_nodes = if let Some(start) = start_node {
            vec![self.graph_manager.get_node(start)?.unwrap()]
        } else {
            self.graph_manager.query_nodes(&query)?
        };

        for node in all_nodes {
            let mut path = Vec::new();
            if !visited.contains(&node.id) {
                self.detect_cycles_dfs(
                    &node.id,
                    &mut visited,
                    &mut recursion_stack,
                    &mut path,
                    &mut cycles,
                    &mut affected_nodes,
                )?;
            }
        }

        let cycle_count = cycles.len();

        Ok(CycleDetectionResult {
            has_cycle: !cycles.is_empty(),
            cycles: cycles.clone(),
            cycle_count,
            affected_nodes,
        })
    }

    fn detect_cycles_dfs(
        &self,
        node_id: &str,
        visited: &mut HashSet<String>,
        recursion_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
        affected_nodes: &mut Vec<String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        visited.insert(node_id.to_string());
        recursion_stack.insert(node_id.to_string());
        path.push(node_id.to_string());

        let edges = self.graph_manager.get_outgoing_edges(node_id)?;

        for edge in edges {
            let neighbor = edge.to_node;

            if !visited.contains(&neighbor) {
                self.detect_cycles_dfs(
                    &neighbor,
                    visited,
                    recursion_stack,
                    path,
                    cycles,
                    affected_nodes,
                )?;
            } else if recursion_stack.contains(&neighbor) {
                // Found a cycle
                let cycle_start = path.iter().position(|n| n == &neighbor).unwrap();
                let cycle: Vec<String> = path[cycle_start..].to_vec();
                cycles.push(cycle);
                affected_nodes.push(node_id.to_string());
            }
        }

        path.pop();
        recursion_stack.remove(node_id);

        Ok(())
    }

    /// Find shortest path using optimized Dijkstra with early termination
    pub fn shortest_path_optimized(
        &self,
        from: &str,
        to: &str,
        _use_heuristic: bool,
    ) -> Result<Option<GraphTraversalResult>, Box<dyn std::error::Error + Send + Sync>> {
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
                    if let Some(node_data) = self.graph_manager.get_node(&path[i])? {
                        nodes.push(node_data);
                    }
                    if let Some(edge) = self.get_edge_optimized(&path[i], &path[i + 1])? {
                        edges.push(edge);
                    }
                }
                if let Some(last_node) = self.graph_manager.get_node(to)? {
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

            let outgoing = self.graph_manager.get_outgoing_edges(&node)?;
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

    /// Bidirectional search for faster path finding
    pub fn bidirectional_search(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<GraphTraversalResult>, Box<dyn std::error::Error + Send + Sync>> {
        use std::collections::VecDeque;

        let mut forward_queue = VecDeque::new();
        let mut backward_queue = VecDeque::new();
        let mut forward_visited: HashMap<String, (Vec<String>, f64)> = HashMap::new();
        let mut backward_visited: HashMap<String, (Vec<String>, f64)> = HashMap::new();

        forward_queue.push_back(from.to_string());
        backward_queue.push_back(to.to_string());
        forward_visited.insert(from.to_string(), (vec![from.to_string()], 0.0));
        backward_visited.insert(to.to_string(), (vec![to.to_string()], 0.0));

        let mut meeting_point = None;

        while !forward_queue.is_empty() && !backward_queue.is_empty() {
            // Expand forward
            if let Some(current) = forward_queue.pop_front() {
                if let Some((_backward_path, backward_cost)) = backward_visited.get(&current) {
                    meeting_point = Some((current.clone(), *backward_cost));
                    break;
                }

                let edges = self.graph_manager.get_outgoing_edges(&current)?;
                for edge in edges {
                    let neighbor = edge.to_node;
                    let new_cost = forward_visited[&current].1 + edge.weight.unwrap_or(1.0);

                    if !forward_visited.contains_key(&neighbor) {
                        let mut new_path = forward_visited[&current].0.clone();
                        new_path.push(neighbor.clone());
                        forward_visited.insert(neighbor.clone(), (new_path, new_cost));
                        forward_queue.push_back(neighbor);
                    }
                }
            }

            // Expand backward
            if let Some(current) = backward_queue.pop_front() {
                if let Some((_forward_path, forward_cost)) = forward_visited.get(&current) {
                    meeting_point = Some((current.clone(), *forward_cost));
                    break;
                }

                // Get incoming edges (need to implement reverse lookup)
                let incoming_edges = self.get_incoming_edges(&current)?;
                for edge in incoming_edges {
                    let neighbor = edge.from_node;
                    let new_cost = backward_visited[&current].1 + edge.weight.unwrap_or(1.0);

                    if !backward_visited.contains_key(&neighbor) {
                        let mut new_path = backward_visited[&current].0.clone();
                        new_path.push(neighbor.clone());
                        backward_visited.insert(neighbor.clone(), (new_path, new_cost));
                        backward_queue.push_back(neighbor);
                    }
                }
            }
        }

        if let Some((meet_node, _)) = meeting_point {
            let (forward_path, forward_cost) = forward_visited.get(&meet_node).unwrap();
            let (backward_path, backward_cost) = backward_visited.get(&meet_node).unwrap();

            let mut full_path = forward_path.clone();
            let mut rev_backward = backward_path.clone();
            rev_backward.pop(); // Remove duplicate meet node
            rev_backward.reverse();
            full_path.extend(rev_backward);

            // Build result
            let mut nodes = Vec::new();
            let mut edges = Vec::new();

            for i in 0..full_path.len() - 1 {
                if let Some(node_data) = self.graph_manager.get_node(&full_path[i])? {
                    nodes.push(node_data);
                }
                if let Some(edge) = self.get_edge_optimized(&full_path[i], &full_path[i + 1])? {
                    edges.push(edge);
                }
            }
            if let Some(last_node) = self
                .graph_manager
                .get_node(&full_path[full_path.len() - 1])?
            {
                nodes.push(last_node);
            }

            return Ok(Some(GraphTraversalResult {
                path: full_path,
                total_weight: forward_cost + backward_cost,
                nodes,
                edges,
            }));
        }

        Ok(None)
    }

    /// Find longest path (using topological sort for DAG)
    pub fn find_longest_path(
        &self,
        from: &str,
        to: Option<&str>,
    ) -> Result<Option<LongestPathResult>, Box<dyn std::error::Error + Send + Sync>> {
        // First detect cycles
        let cycle_check = self.detect_cycles(None)?;

        if cycle_check.has_cycle {
            // For graphs with cycles, use DFS with memoization
            return self.find_longest_path_with_cycles(from, to);
        } else {
            // For DAG, use topological sort
            return self.find_longest_path_dag(from, to);
        }
    }

    fn find_longest_path_dag(
        &self,
        from: &str,
        to: Option<&str>,
    ) -> Result<Option<LongestPathResult>, Box<dyn std::error::Error + Send + Sync>> {
        // Get topological order
        let topo_order = self.topological_sort()?;

        let mut dist: HashMap<String, f64> = HashMap::new();
        let mut next: HashMap<String, String> = HashMap::new();

        dist.insert(from.to_string(), 0.0);

        for node in topo_order {
            if let Some(&current_dist) = dist.get(&node) {
                let edges = self.graph_manager.get_outgoing_edges(&node)?;
                for edge in edges {
                    let new_dist = current_dist + edge.weight.unwrap_or(1.0);
                    if new_dist > *dist.get(&edge.to_node).unwrap_or(&f64::NEG_INFINITY) {
                        dist.insert(edge.to_node.clone(), new_dist);
                        next.insert(edge.to_node.clone(), node.clone());
                    }
                }
            }
        }

        let target = to.unwrap_or_else(|| {
            dist.iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(k, _)| k.as_str())
                .unwrap_or(from)
        });

        if let Some(&total_weight) = dist.get(target) {
            // Reconstruct path
            let mut path = Vec::new();
            let mut current = target.to_string();
            while let Some(prev) = next.get(&current) {
                path.push(current.clone());
                current = prev.clone();
            }
            path.push(from.to_string());
            path.reverse();

            // Build result
            let mut nodes = Vec::new();
            let mut edges = Vec::new();

            for i in 0..path.len() - 1 {
                if let Some(node_data) = self.graph_manager.get_node(&path[i])? {
                    nodes.push(node_data);
                }
                if let Some(edge) = self.get_edge_optimized(&path[i], &path[i + 1])? {
                    edges.push(edge);
                }
            }
            if let Some(last_node) = self.graph_manager.get_node(target)? {
                nodes.push(last_node);
            }

            let nodes_len = nodes.len();

            return Ok(Some(LongestPathResult {
                path,
                total_weight,
                nodes: nodes.clone(),
                edges,
                length: nodes_len,
            }));
        }

        Ok(None)
    }

    fn find_longest_path_with_cycles(
        &self,
        from: &str,
        to: Option<&str>,
    ) -> Result<Option<LongestPathResult>, Box<dyn std::error::Error + Send + Sync>> {
        // For graphs with cycles, use DFS with memoization
        let mut memo: HashMap<String, Option<(f64, Vec<String>)>> = HashMap::new();

        let target = to.unwrap_or(from);

        let result = self.dfs_longest_path(from, target, &mut memo, &mut HashSet::new())?;

        if let Some((total_weight, path)) = result {
            let mut nodes = Vec::new();
            let mut edges = Vec::new();

            for i in 0..path.len() - 1 {
                if let Some(node_data) = self.graph_manager.get_node(&path[i])? {
                    nodes.push(node_data);
                }
                if let Some(edge) = self.get_edge_optimized(&path[i], &path[i + 1])? {
                    edges.push(edge);
                }
            }
            if let Some(last_node) = self.graph_manager.get_node(target)? {
                nodes.push(last_node);
            }

            let nodes_len = nodes.len();

            return Ok(Some(LongestPathResult {
                path,
                total_weight,
                nodes: nodes.clone(),
                edges,
                length: nodes_len,
            }));
        }

        Ok(None)
    }

    fn dfs_longest_path(
        &self,
        current: &str,
        target: &str,
        memo: &mut HashMap<String, Option<(f64, Vec<String>)>>,
        visited: &mut HashSet<String>,
    ) -> Result<Option<(f64, Vec<String>)>, Box<dyn std::error::Error + Send + Sync>> {
        if current == target {
            return Ok(Some((0.0, vec![current.to_string()])));
        }

        if visited.contains(current) {
            return Ok(None);
        }

        if let Some(cached) = memo.get(current) {
            return Ok(cached.clone());
        }

        visited.insert(current.to_string());

        let edges = self.graph_manager.get_outgoing_edges(current)?;
        let mut best: Option<(f64, Vec<String>)> = None;

        for edge in edges {
            if let Some((sub_weight, mut sub_path)) =
                self.dfs_longest_path(&edge.to_node, target, memo, visited)?
            {
                let total_weight = sub_weight + edge.weight.unwrap_or(1.0);
                sub_path.insert(0, current.to_string());

                if let Some((best_weight, _)) = &best {
                    if total_weight > *best_weight {
                        best = Some((total_weight, sub_path));
                    }
                } else {
                    best = Some((total_weight, sub_path));
                }
            }
        }

        visited.remove(current);
        memo.insert(current.to_string(), best.clone());

        Ok(best)
    }

    fn topological_sort(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let query = DistributedGraphQuery {
            limit: 10000,
            ..Default::default()
        };

        let all_nodes = self.graph_manager.query_nodes(&query)?;
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();

        // Build graph and compute in-degrees
        for node in &all_nodes {
            let edges = self.graph_manager.get_outgoing_edges(&node.id)?;
            for edge in edges {
                graph
                    .entry(node.id.clone())
                    .or_insert_with(Vec::new)
                    .push(edge.to_node.clone());
                *in_degree.entry(edge.to_node.clone()).or_insert(0) += 1;
            }
            in_degree.entry(node.id.clone()).or_insert(0);
        }

        // Kahn's algorithm
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter_map(|(node, &degree)| {
                if degree == 0 {
                    Some(node.clone())
                } else {
                    None
                }
            })
            .collect();

        let mut topo_order = Vec::new();

        while let Some(node) = queue.pop_front() {
            topo_order.push(node.clone());

            if let Some(neighbors) = graph.get(&node) {
                for neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        if topo_order.len() != all_nodes.len() {
            println!("Warning: Graph has cycles, returning partial topological order");
        }

        Ok(topo_order)
    }

    fn get_edge_optimized(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<DistributedGraphEdge>, Box<dyn std::error::Error + Send + Sync>> {
        let edges = self.graph_manager.get_outgoing_edges(from)?;
        Ok(edges.into_iter().find(|e| e.to_node == to))
    }

    fn get_incoming_edges(
        &self,
        node: &str,
    ) -> Result<Vec<DistributedGraphEdge>, Box<dyn std::error::Error + Send + Sync>> {
        // Query all nodes to find edges pointing to this node
        let query = DistributedGraphQuery {
            limit: 10000,
            ..Default::default()
        };

        let all_nodes = self.graph_manager.query_nodes(&query)?;
        let mut incoming = Vec::new();

        for n in all_nodes {
            let edges = self.graph_manager.get_outgoing_edges(&n.id)?;
            for edge in edges {
                if edge.to_node == node {
                    incoming.push(edge);
                }
            }
        }

        Ok(incoming)
    }

    /// Parallel cycle detection across shards
    pub fn parallel_cycle_detection(
        &self,
    ) -> Result<CycleDetectionResult, Box<dyn std::error::Error + Send + Sync>> {
        let query = DistributedGraphQuery {
            limit: 10000,
            ..Default::default()
        };

        let all_nodes = self.graph_manager.query_nodes(&query)?;

        // Process nodes in parallel
        let results: Vec<Result<Vec<Vec<String>>, Box<dyn std::error::Error + Send + Sync>>> =
            all_nodes
                .par_iter()
                .map(|node| {
                    let mut visited = HashSet::new();
                    let mut recursion_stack = HashSet::new();
                    let mut cycles = Vec::new();
                    let mut path = Vec::new();
                    let mut affected = Vec::new();

                    self.detect_cycles_dfs(
                        &node.id,
                        &mut visited,
                        &mut recursion_stack,
                        &mut path,
                        &mut cycles,
                        &mut affected,
                    )?;
                    Ok(cycles)
                })
                .collect();

        let mut all_cycles = Vec::new();
        for result in results {
            all_cycles.extend(result?);
        }

        let cycle_count = all_cycles.len();

        Ok(CycleDetectionResult {
            has_cycle: !all_cycles.is_empty(),
            cycles: all_cycles.clone(),
            cycle_count,
            affected_nodes: Vec::new(),
        })
    }
}
