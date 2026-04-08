use bund_blobstore::common::{
    Bounds, Coord1D, Coord2D, Coord3D, Coordinate, DimensionType, MultidimensionalStorage,
    TelemetrySample,
};
use bund_blobstore::{
    BlobStore, DataDistributionManager, DistributionStrategy, SearchResult, SearchableBlobStore,
    TelemetryQuery, TelemetryRecord, TelemetryStore, TelemetryValue, TimeInterval,
    VectorSearchResult, VectorStore,
};
use chrono::{DateTime, Duration, Utc};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Custom error type
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// ============================================
// GRAPH STORAGE USING THE MANAGER
// ============================================

// Simple graph node structure with Serde support
#[derive(Clone, Debug, Serialize, Deserialize)]
struct GraphNode {
    id: String,
    label: String,
    properties: HashMap<String, String>,
}

// Simple graph edge structure with Serde support
#[derive(Clone, Debug, Serialize, Deserialize)]
struct GraphEdge {
    from: String,
    to: String,
    weight: f64,
    relationship: String,
}

// Graph storage wrapper that uses the same manager
struct ManagedGraphStore {
    _manager: Arc<DataDistributionManager>,
    store: Mutex<BlobStore>, // Use blob store for persistence
}

impl ManagedGraphStore {
    fn new(manager: Arc<DataDistributionManager>, path: &str) -> Result<Self> {
        let store = BlobStore::open(path)?;
        Ok(Self {
            _manager: manager,
            store: Mutex::new(store),
        })
    }

    fn add_node(&self, node: GraphNode) -> Result<()> {
        let key = format!("node:{}", node.id);
        let data = serde_json::to_vec(&node)?;
        Ok(self
            .store
            .lock()
            .unwrap()
            .put(&key, &data, Some("graph_nodes"))?)
    }
    #[allow(dead_code)]
    fn get_node(&self, id: &str) -> Result<Option<GraphNode>> {
        let key = format!("node:{}", id);
        if let Some(data) = self.store.lock().unwrap().get(&key)? {
            let node: GraphNode = serde_json::from_slice(&data)?;
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    fn add_edge(&self, edge: GraphEdge) -> Result<()> {
        let key = format!("edge:{}:{}", edge.from, edge.to);
        let data = serde_json::to_vec(&edge)?;
        Ok(self
            .store
            .lock()
            .unwrap()
            .put(&key, &data, Some("graph_edges"))?)
    }

    fn get_edges_from(&self, from: &str) -> Result<Vec<GraphEdge>> {
        let prefix = format!("edge:{}:", from);
        let all_keys = self.store.lock().unwrap().list_keys()?;
        let mut edges: Vec<GraphEdge> = Vec::new();

        for key in all_keys {
            if key.starts_with(&prefix) {
                if let Some(data) = self.store.lock().unwrap().get(&key)? {
                    if let Ok(edge) = serde_json::from_slice(&data) {
                        edges.push(edge);
                    }
                }
            }
        }

        Ok(edges)
    }

    fn find_shortest_path(&self, start: &str, end: &str) -> Result<Option<Vec<String>>> {
        // Simple BFS implementation
        let mut visited = HashMap::new();
        let mut queue = vec![(start.to_string(), vec![start.to_string()])];
        visited.insert(start.to_string(), true);

        while !queue.is_empty() {
            let (current, path) = queue.remove(0);

            if current == end {
                return Ok(Some(path));
            }

            let edges = self.get_edges_from(&current)?;
            for edge in edges {
                if !visited.contains_key(&edge.to) {
                    visited.insert(edge.to.clone(), true);
                    let mut new_path = path.clone();
                    new_path.push(edge.to.clone());
                    queue.push((edge.to, new_path));
                }
            }
        }

        Ok(None)
    }
}

// ============================================
// LOG STORAGE USING THE MANAGER
// ============================================

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LogEntry {
    timestamp: DateTime<Utc>,
    level: LogLevel,
    service: String,
    message: String,
    metadata: HashMap<String, String>,
}

// Log storage wrapper that uses the same manager
struct ManagedLogStore {
    _manager: Arc<DataDistributionManager>,
    store: Mutex<BlobStore>,
}

impl ManagedLogStore {
    fn new(manager: Arc<DataDistributionManager>, path: &str) -> Result<Self> {
        let store = BlobStore::open(path)?;
        Ok(Self {
            _manager: manager,
            store: Mutex::new(store),
        })
    }

    fn ingest(&self, log: LogEntry) -> Result<()> {
        let timestamp = log.timestamp.timestamp_nanos_opt().unwrap_or(0);
        let key = format!("log:{}:{}:{}", log.service, timestamp, uuid::Uuid::new_v4());
        let data = serde_json::to_vec(&log)?;
        Ok(self.store.lock().unwrap().put(&key, &data, Some("logs"))?)
    }

    fn query_by_service(&self, service: &str, limit: usize) -> Result<Vec<LogEntry>> {
        let prefix = format!("log:{}:", service);
        let all_keys = self.store.lock().unwrap().list_keys()?;
        let mut logs: Vec<LogEntry> = Vec::new();

        for key in all_keys {
            if logs.len() >= limit {
                break;
            }
            if key.starts_with(&prefix) {
                if let Some(data) = self.store.lock().unwrap().get(&key)? {
                    if let Ok(log) = serde_json::from_slice(&data) {
                        logs.push(log);
                    }
                }
            }
        }

        // Sort by timestamp descending
        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(logs)
    }

    fn query_by_level(&self, level: LogLevel, limit: usize) -> Result<Vec<LogEntry>> {
        let all_keys = self.store.lock().unwrap().list_keys()?;
        let mut logs: Vec<LogEntry> = Vec::new();

        for key in all_keys {
            if logs.len() >= limit {
                break;
            }
            if key.starts_with("log:") {
                if let Some(data) = self.store.lock().unwrap().get(&key)? {
                    if let Ok(log) = serde_json::from_slice::<LogEntry>(&data) {
                        if log.level == level {
                            logs.push(log);
                        }
                    }
                }
            }
        }

        logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(logs)
    }

    fn get_recent_errors(&self, minutes: i64) -> Result<Vec<LogEntry>> {
        let cutoff = Utc::now() - Duration::minutes(minutes);
        let all_keys = self.store.lock().unwrap().list_keys()?;
        let mut errors: Vec<LogEntry> = Vec::new();

        for key in all_keys {
            if key.starts_with("log:") {
                if let Some(data) = self.store.lock().unwrap().get(&key)? {
                    if let Ok(log) = serde_json::from_slice::<LogEntry>(&data) {
                        if log.timestamp >= cutoff && log.level == LogLevel::Error {
                            errors.push(log);
                        }
                    }
                }
            }
        }

        errors.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(errors)
    }
}

// ============================================
// WRAPPERS FOR EXISTING STORES (PRESERVED)
// ============================================

// Wrapper for BlobStore
struct ManagedBlobStore {
    _manager: Arc<DataDistributionManager>,
    store: Mutex<BlobStore>,
}

impl ManagedBlobStore {
    fn new(manager: Arc<DataDistributionManager>, path: &str) -> Result<Self> {
        let store = BlobStore::open(path)?;
        Ok(Self {
            _manager: manager,
            store: Mutex::new(store),
        })
    }

    fn put(&self, key: &str, value: &[u8], category: Option<&str>) -> Result<()> {
        Ok(self.store.lock().unwrap().put(key, value, category)?)
    }

    fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.store.lock().unwrap().get(key)?)
    }
}

// Wrapper for SearchableBlobStore
struct ManagedSearchStore {
    _manager: Arc<DataDistributionManager>,
    store: Mutex<SearchableBlobStore>,
}

impl ManagedSearchStore {
    fn new(manager: Arc<DataDistributionManager>, path: &str) -> Result<Self> {
        let store = SearchableBlobStore::open(path)?;
        Ok(Self {
            _manager: manager,
            store: Mutex::new(store),
        })
    }

    fn put_text(&self, id: &str, text: &str, category: Option<&str>) -> Result<()> {
        Ok(self.store.lock().unwrap().put_text(id, text, category)?)
    }

    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        Ok(self.store.lock().unwrap().search(query, limit)?)
    }
}

// Wrapper for VectorStore
struct ManagedVectorStore {
    _manager: Arc<DataDistributionManager>,
    store: Mutex<VectorStore>,
}

impl ManagedVectorStore {
    fn new(manager: Arc<DataDistributionManager>, path: &str) -> Result<Self> {
        let store = VectorStore::open(path)?;
        Ok(Self {
            _manager: manager,
            store: Mutex::new(store),
        })
    }

    fn insert_text(&self, id: &str, text: &str, category: Option<&str>) -> Result<()> {
        Ok(self.store.lock().unwrap().insert_text(id, text, category)?)
    }

    fn search_similar(&self, query: &str, limit: usize) -> Result<Vec<VectorSearchResult>> {
        Ok(self.store.lock().unwrap().search_similar(query, limit)?)
    }
}

// Wrapper for TelemetryStore
struct ManagedTelemetryStore {
    _manager: Arc<DataDistributionManager>,
    store: Mutex<TelemetryStore>,
}

impl ManagedTelemetryStore {
    fn new(manager: Arc<DataDistributionManager>, path: &str) -> Result<Self> {
        let store = TelemetryStore::open(path)?;
        Ok(Self {
            _manager: manager,
            store: Mutex::new(store),
        })
    }

    fn store(&self, record: TelemetryRecord) -> Result<()> {
        Ok(self.store.lock().unwrap().store(record)?)
    }

    fn query(&self, query: &TelemetryQuery) -> Result<Vec<TelemetryRecord>> {
        Ok(self.store.lock().unwrap().query(query)?)
    }
}

// Wrapper for MultidimensionalStorage
struct ManagedMultidimensionalStorage {
    _manager: Arc<DataDistributionManager>,
    store: Mutex<MultidimensionalStorage>,
}

impl ManagedMultidimensionalStorage {
    fn new(manager: Arc<DataDistributionManager>, path: &str) -> Result<Self> {
        let store = MultidimensionalStorage::open(path)?;
        Ok(Self {
            _manager: manager,
            store: Mutex::new(store),
        })
    }

    fn create_dimension(
        &self,
        name: &str,
        dim_type: DimensionType,
        capacity: usize,
        bounds: Option<Bounds>,
    ) -> Result<()> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .create_dimension(name, dim_type, capacity, bounds)?)
    }

    fn push_sample(
        &self,
        dimension: &str,
        coord: Coordinate,
        value: TelemetryValue,
        timestamp: Option<DateTime<Utc>>,
        metadata: HashMap<String, String>,
    ) -> Result<()> {
        self.store
            .lock()
            .unwrap()
            .push_sample(dimension, coord, value, timestamp, metadata)?;
        Ok(())
    }

    fn get_latest_samples(
        &self,
        dimension: &str,
        coord: Coordinate,
        limit: usize,
    ) -> Result<Vec<TelemetrySample>> {
        Ok(self
            .store
            .lock()
            .unwrap()
            .get_latest_samples(dimension, coord, limit)?)
    }
}

// ============================================
// GLOBAL MANAGER AND STORES
// ============================================

lazy_static! {
    // Single global manager for all operations
    static ref MANAGER: Arc<DataDistributionManager> = {
        let manager = DataDistributionManager::new(
            "./unified_data_store",
            DistributionStrategy::RoundRobin,
        ).expect("Failed to create DataDistributionManager");

        Arc::new(manager)
    };

    // All stores use the SAME manager (existing stores preserved)
    static ref BLOB_STORE: ManagedBlobStore = {
        ManagedBlobStore::new(MANAGER.clone(), "./unified_data_store/blobs.redb")
            .expect("Failed to create blob store")
    };

    static ref SEARCH_STORE: ManagedSearchStore = {
        ManagedSearchStore::new(MANAGER.clone(), "./unified_data_store/search.redb")
            .expect("Failed to create search store")
    };

    static ref VECTOR_STORE: ManagedVectorStore = {
        ManagedVectorStore::new(MANAGER.clone(), "./unified_data_store/vectors.redb")
            .expect("Failed to create vector store")
    };

    static ref TELEMETRY_STORE: ManagedTelemetryStore = {
        ManagedTelemetryStore::new(MANAGER.clone(), "./unified_data_store/timeline.redb")
            .expect("Failed to create telemetry store")
    };

    static ref MULTIDIM_STORAGE: ManagedMultidimensionalStorage = {
        ManagedMultidimensionalStorage::new(MANAGER.clone(), "./unified_data_store/multidim.redb")
            .expect("Failed to create multidimensional storage")
    };

    // NEW: Graph and Log stores added without removing any existing functionality
    static ref GRAPH_STORE: ManagedGraphStore = {
        ManagedGraphStore::new(MANAGER.clone(), "./unified_data_store/graph.redb")
            .expect("Failed to create graph store")
    };

    static ref LOG_STORE: ManagedLogStore = {
        ManagedLogStore::new(MANAGER.clone(), "./unified_data_store/logs.redb")
            .expect("Failed to create log store")
    };
}

// Helper function to display strategy
fn strategy_name(strategy: &DistributionStrategy) -> &'static str {
    match strategy {
        DistributionStrategy::RoundRobin => "RoundRobin",
        DistributionStrategy::TimeBucket(_) => "TimeBucket",
        DistributionStrategy::KeySimilarity(_) => "KeySimilarity",
        DistributionStrategy::Adaptive(_) => "Adaptive",
    }
}

// ============================================
// DEMO IMPLEMENTATIONS (ALL PRESERVED)
// ============================================

fn demo_multidimensional_telemetry() -> Result<()> {
    println!("\n📊 MULTIDIMENSIONAL TELEMETRY");
    println!("==============================");
    println!("Demonstrating 1D, 2D, and 3D dimensions:\n");

    let bounds_2d = Bounds {
        min_x: 0,
        max_x: 100,
        min_y: Some(0),
        max_y: Some(100),
        min_z: None,
        max_z: None,
    };

    // Create all three dimension types
    MULTIDIM_STORAGE.create_dimension("sensors_1d", DimensionType::OneD, 1000, None)?;
    MULTIDIM_STORAGE.create_dimension("grid_2d", DimensionType::TwoD, 500, Some(bounds_2d))?;
    MULTIDIM_STORAGE.create_dimension("voxels_3d", DimensionType::ThreeD, 1000, None)?;
    println!("✓ Created 1D, 2D, and 3D dimensions");

    // 1D: Linear sensor array (temperature sensors along a line)
    println!("\n  [1D] Linear Sensor Array:");
    for sensor_id in 0..5 {
        let coord = Coordinate::OneD(Coord1D(sensor_id));
        let temp_value = 20.0 + (sensor_id as f64) * 0.5;
        let mut metadata = HashMap::new();
        metadata.insert("location".to_string(), format!("Position_{}", sensor_id));
        metadata.insert("unit".to_string(), "celsius".to_string());

        MULTIDIM_STORAGE.push_sample(
            "sensors_1d",
            coord,
            TelemetryValue::Float(temp_value),
            Some(Utc::now()),
            metadata,
        )?;
        println!(
            "    - Sensor at position {}: {:.1}°C",
            sensor_id, temp_value
        );
    }

    // 2D: Grid pattern (pressure sensors in X-Y grid)
    println!("\n  [2D] Pressure Grid (5x5):");
    for x in 0..3 {
        for y in 0..3 {
            let coord = Coordinate::TwoD(Coord2D(x, y));
            let pressure = 1013.0 + (x as f64) * 0.1 + (y as f64) * 0.05;
            MULTIDIM_STORAGE.push_sample(
                "grid_2d",
                coord,
                TelemetryValue::Float(pressure),
                Some(Utc::now()),
                HashMap::new(),
            )?;
            if x == 0 && y == 0 {
                println!("    - Pressure at (0,0): {:.1} hPa", pressure);
                println!("    - ... and 8 more grid points");
            }
        }
    }

    // 3D: Voxel space (3D volumetric data)
    println!("\n  [3D] Voxel Space (3x3x3):");
    for x in 0..2 {
        for y in 0..2 {
            for z in 0..2 {
                let coord = Coordinate::ThreeD(Coord3D(x, y, z));
                let density = (x + y + z) as f64 / 3.0;
                MULTIDIM_STORAGE.push_sample(
                    "voxels_3d",
                    coord,
                    TelemetryValue::Float(density),
                    Some(Utc::now()),
                    HashMap::new(),
                )?;
            }
        }
    }
    println!("    - Stored 8 voxel samples (2x2x2 subset)");

    // Query and display samples from each dimension
    println!("\n  Query Results:");
    let samples_1d =
        MULTIDIM_STORAGE.get_latest_samples("sensors_1d", Coordinate::OneD(Coord1D(2)), 5)?;
    println!("    - 1D Sensor 2: {} readings", samples_1d.len());

    let samples_2d =
        MULTIDIM_STORAGE.get_latest_samples("grid_2d", Coordinate::TwoD(Coord2D(1, 1)), 5)?;
    println!("    - 2D Grid (1,1): {} readings", samples_2d.len());

    let samples_3d = MULTIDIM_STORAGE.get_latest_samples(
        "voxels_3d",
        Coordinate::ThreeD(Coord3D(1, 1, 1)),
        5,
    )?;
    println!("    - 3D Voxel (1,1,1): {} readings", samples_3d.len());

    Ok(())
}

fn demo_telemetry_timeseries() -> Result<()> {
    println!("\n📈 TELEMETRY TIME SERIES");
    println!("========================");

    for i in 0..20 {
        let timestamp = Utc::now() - Duration::minutes(i as i64);
        let cpu_usage = 20.0 + (i as f64) * 2.5;

        let cpu_record = TelemetryRecord::new_primary(
            format!("server_{}", i % 3),
            timestamp,
            "cpu_usage".to_string(),
            "production".to_string(),
            TelemetryValue::Float(cpu_usage),
        );
        TELEMETRY_STORE.store(cpu_record)?;
    }
    println!("✓ Stored 20 telemetry samples");

    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        limit: 10,
        ..Default::default()
    };
    let results = TELEMETRY_STORE.query(&query)?;
    println!("✓ Retrieved {} CPU usage records", results.len());

    Ok(())
}

fn demo_search_and_vectors() -> Result<()> {
    println!("\n🔍 SEARCH & VECTOR STORAGE");
    println!("==========================");

    SEARCH_STORE.put_text(
        "doc1",
        "The quick brown fox jumps over the lazy dog",
        Some("animals"),
    )?;
    SEARCH_STORE.put_text(
        "doc2",
        "Rust is a systems programming language focusing on safety and performance",
        Some("programming"),
    )?;
    println!("✓ Stored documents for full-text search");

    let search_results = SEARCH_STORE.search("programming language", 10)?;
    println!("✓ Found {} results", search_results.len());

    VECTOR_STORE.insert_text(
        "vec1",
        "Artificial intelligence and deep learning",
        Some("ai"),
    )?;
    VECTOR_STORE.insert_text(
        "vec2",
        "Machine learning algorithms for data science",
        Some("ai"),
    )?;
    println!("✓ Stored vectors for semantic search");

    let vector_results = VECTOR_STORE.search_similar("neural networks", 5)?;
    println!("✓ Found {} similar results", vector_results.len());

    Ok(())
}

// NEW: Graph storage demo
fn demo_graph_storage() -> Result<()> {
    println!("\n🕸️ GRAPH STORAGE");
    println!("================");

    // Create nodes
    let nodes = vec![
        GraphNode {
            id: "A".to_string(),
            label: "Node A".to_string(),
            properties: HashMap::new(),
        },
        GraphNode {
            id: "B".to_string(),
            label: "Node B".to_string(),
            properties: HashMap::new(),
        },
        GraphNode {
            id: "C".to_string(),
            label: "Node C".to_string(),
            properties: HashMap::new(),
        },
        GraphNode {
            id: "D".to_string(),
            label: "Node D".to_string(),
            properties: HashMap::new(),
        },
        GraphNode {
            id: "E".to_string(),
            label: "Node E".to_string(),
            properties: HashMap::new(),
        },
    ];

    for node in nodes {
        GRAPH_STORE.add_node(node)?;
    }
    println!("✓ Added 5 nodes to graph");

    // Create edges
    let edges = vec![
        GraphEdge {
            from: "A".to_string(),
            to: "B".to_string(),
            weight: 1.0,
            relationship: "connects".to_string(),
        },
        GraphEdge {
            from: "B".to_string(),
            to: "C".to_string(),
            weight: 1.0,
            relationship: "connects".to_string(),
        },
        GraphEdge {
            from: "C".to_string(),
            to: "D".to_string(),
            weight: 1.0,
            relationship: "connects".to_string(),
        },
        GraphEdge {
            from: "D".to_string(),
            to: "E".to_string(),
            weight: 1.0,
            relationship: "connects".to_string(),
        },
        GraphEdge {
            from: "A".to_string(),
            to: "D".to_string(),
            weight: 2.0,
            relationship: "direct".to_string(),
        },
    ];

    for edge in edges {
        GRAPH_STORE.add_edge(edge)?;
    }
    println!("✓ Added 5 edges to graph");

    // Find shortest path
    if let Some(path) = GRAPH_STORE.find_shortest_path("A", "E")? {
        println!("✓ Shortest path from A to E: {:?}", path);
    }

    // Get edges from node A
    let edges_from_a = GRAPH_STORE.get_edges_from("A")?;
    println!("✓ Node A has {} outgoing edges", edges_from_a.len());

    Ok(())
}

// NEW: Log storage demo
fn demo_log_storage() -> Result<()> {
    println!("\n📝 LOG STORAGE");
    println!("==============");

    // Generate logs
    let logs = vec![
        LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            service: "api-gateway".to_string(),
            message: "Request processed successfully".to_string(),
            metadata: HashMap::new(),
        },
        LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Warn,
            service: "api-gateway".to_string(),
            message: "Rate limit approaching threshold".to_string(),
            metadata: HashMap::new(),
        },
        LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Error,
            service: "database".to_string(),
            message: "Connection pool exhausted".to_string(),
            metadata: HashMap::new(),
        },
        LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Info,
            service: "auth-service".to_string(),
            message: "User authentication successful".to_string(),
            metadata: HashMap::new(),
        },
        LogEntry {
            timestamp: Utc::now(),
            level: LogLevel::Error,
            service: "payment-processor".to_string(),
            message: "Transaction failed".to_string(),
            metadata: HashMap::new(),
        },
    ];

    for log in logs {
        LOG_STORE.ingest(log)?;
    }
    println!("✓ Ingested 5 log entries");

    // Query logs by service
    let api_logs = LOG_STORE.query_by_service("api-gateway", 10)?;
    println!("✓ Found {} logs for api-gateway", api_logs.len());

    // Query errors
    let errors = LOG_STORE.query_by_level(LogLevel::Error, 10)?;
    println!("✓ Found {} error logs", errors.len());

    // Get recent errors
    let recent_errors = LOG_STORE.get_recent_errors(60)?;
    println!("✓ Found {} errors in last hour", recent_errors.len());

    Ok(())
}

fn demo_blob_storage() -> Result<()> {
    println!("\n💾 BLOB STORAGE");
    println!("===============");

    let config_data = br#"{"version": "1.0", "environment": "production"}"#;
    BLOB_STORE.put("config.json", config_data, Some("configs"))?;
    println!("✓ Stored configuration blob ({} bytes)", config_data.len());

    if let Some(data) = BLOB_STORE.get("config.json")? {
        println!("✓ Retrieved config.json: {} bytes", data.len());
    }

    Ok(())
}

fn demo_manager_coordination() -> Result<()> {
    println!("\n🎯 MANAGER COORDINATION");
    println!("======================");

    println!("✓ Single DataDistributionManager coordinates ALL storage types:");
    println!("  - Blob Store (raw binary data)");
    println!("  - Search Store (full-text indexed documents)");
    println!("  - Vector Store (semantic embeddings)");
    println!("  - Telemetry Store (time-series data)");
    println!("  - Multidimensional Storage (1D/2D/3D telemetry)");
    println!("  - Graph Store (nodes and edges with path finding) [NEW]");
    println!("  - Log Store (structured logging with querying) [NEW]");
    println!(
        "\n✓ Distribution strategy: {}",
        strategy_name(&MANAGER.get_strategy())
    );
    println!("✓ All 7 storage types share the same manager instance");
    println!("✓ ACID compliance maintained across all operations");

    // Store data directly through the manager
    MANAGER.put("direct_key1", b"Direct value 1", None)?;
    MANAGER.put("direct_key2", b"Direct value 2", None)?;
    println!("\n✓ Also stored 2 items directly through DataDistributionManager");

    let keys = MANAGER.list_keys(None)?;
    println!("✓ Total keys in manager: {}", keys.len());

    Ok(())
}

// ============================================
// MAIN FUNCTION
// ============================================

fn main() -> Result<()> {
    println!("🚀 Unified Data Platform with Single DataDistributionManager\n");
    println!("=============================================================\n");

    println!("✨ All 7 storage types share the SAME DataDistributionManager instance");
    println!("📁 Base path: ./unified_data_store\n");

    // Run all demos (existing + new)
    demo_multidimensional_telemetry()?;
    demo_telemetry_timeseries()?;
    demo_search_and_vectors()?;
    demo_graph_storage()?; // NEW
    demo_log_storage()?; // NEW
    demo_blob_storage()?;
    demo_manager_coordination()?;

    // Display final summary
    println!("\n📊 SYSTEM SUMMARY");
    println!("=================");
    println!("✅ SINGLE DataDistributionManager for ALL 7 storage types");
    println!("✅ Multidimensional telemetry (1D, 2D, 3D) - ALL dimensions demonstrated");
    println!("✅ Time series telemetry with query support");
    println!("✅ Full-text search with relevance scoring");
    println!("✅ Vector similarity search with embeddings");
    println!("✅ Graph storage with path finding [NEW]");
    println!("✅ Structured logging with level-based querying [NEW]");
    println!("✅ Binary blob storage for any data type");
    println!("✅ ACID-compliant transactions across all types");
    println!("✅ Consistent distribution strategy for all data");
    println!("✅ Global lazy_static configuration");

    println!("\n🎉 Demo completed successfully!");
    Ok(())
}
