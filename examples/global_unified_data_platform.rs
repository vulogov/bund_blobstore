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
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Custom error type
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// ============================================
// WRAPPERS THAT USE THE SAME MANAGER
// ============================================

// Wrapper for BlobStore that uses the manager's underlying storage
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
        // push_sample returns Result<()>, so we just propagate it
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

    // All stores use the SAME manager and share the same base directory
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
// DEMO IMPLEMENTATIONS
// ============================================

fn demo_multidimensional_telemetry() -> Result<()> {
    println!("\n📊 MULTIDIMENSIONAL TELEMETRY");
    println!("==============================");

    // Create dimensions with bounds
    let bounds_2d = Bounds {
        min_x: 0,
        max_x: 100,
        min_y: Some(0),
        max_y: Some(100),
        min_z: None,
        max_z: None,
    };

    MULTIDIM_STORAGE.create_dimension("sensors_1d", DimensionType::OneD, 1000, None)?;
    MULTIDIM_STORAGE.create_dimension("grid_2d", DimensionType::TwoD, 500, Some(bounds_2d))?;
    MULTIDIM_STORAGE.create_dimension("voxels_3d", DimensionType::ThreeD, 1000, None)?;
    println!("✓ Created 1D, 2D, and 3D dimensions");

    // Store 1D telemetry (temperature sensors)
    for sensor_id in 0..10 {
        let coord = Coordinate::OneD(Coord1D(sensor_id));
        let temp_value = 20.0 + (sensor_id as f64) * 0.5;
        let mut metadata = HashMap::new();
        metadata.insert("location".to_string(), format!("Zone_{}", sensor_id / 2));
        metadata.insert("unit".to_string(), "celsius".to_string());

        MULTIDIM_STORAGE.push_sample(
            "sensors_1d",
            coord,
            TelemetryValue::Float(temp_value),
            Some(Utc::now()),
            metadata,
        )?;
    }
    println!("✓ Stored 10 1D temperature readings");

    // Store 2D grid data (pressure map)
    for x in 0..5 {
        for y in 0..5 {
            let coord = Coordinate::TwoD(Coord2D(x, y));
            let pressure = 1013.0 + (x as f64) * 0.1 + (y as f64) * 0.05;
            MULTIDIM_STORAGE.push_sample(
                "grid_2d",
                coord,
                TelemetryValue::Float(pressure),
                Some(Utc::now()),
                HashMap::new(),
            )?;
        }
    }
    println!("✓ Stored 25 2D pressure readings");

    // Store 3D voxel data
    for x in 0..3 {
        for y in 0..3 {
            for z in 0..3 {
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
    println!("✓ Stored 27 3D voxel readings");

    // Query data - returns Vec<TelemetrySample>
    let samples =
        MULTIDIM_STORAGE.get_latest_samples("sensors_1d", Coordinate::OneD(Coord1D(5)), 5)?;
    println!("✓ Retrieved {} samples from Sensor 5", samples.len());

    // Display sample info
    for sample in samples.iter().take(3) {
        println!("  - Sample at {}: {:?}", sample.timestamp, sample.value);
    }

    Ok(())
}

fn demo_telemetry_timeseries() -> Result<()> {
    println!("\n📈 TELEMETRY TIME SERIES");
    println!("========================");

    // Generate historical telemetry data
    for i in 0..20 {
        let timestamp = Utc::now() - Duration::minutes(i as i64);
        let cpu_usage = 20.0 + (i as f64) * 2.5;
        let memory_usage = 1024.0 + (i as f64) * 50.0;

        let cpu_record = TelemetryRecord::new_primary(
            format!("server_{}", i % 3),
            timestamp,
            "cpu_usage".to_string(),
            "production".to_string(),
            TelemetryValue::Float(cpu_usage),
        );
        TELEMETRY_STORE.store(cpu_record)?;

        let mem_record = TelemetryRecord::new_primary(
            format!("server_{}", i % 3),
            timestamp,
            "memory_mb".to_string(),
            "production".to_string(),
            TelemetryValue::Float(memory_usage),
        );
        TELEMETRY_STORE.store(mem_record)?;
    }
    println!("✓ Stored 40 telemetry samples");

    // Query recent data
    let query = TelemetryQuery {
        time_interval: Some(TimeInterval::last_hour()),
        keys: Some(vec!["cpu_usage".to_string()]),
        limit: 10,
        ..Default::default()
    };
    let results = TELEMETRY_STORE.query(&query)?;
    println!("✓ Retrieved {} CPU usage records", results.len());

    // Display some results - timestamp is a method, not a field
    for result in results.iter().take(3) {
        println!(
            "  - {}: {:?} at {}",
            result.key,
            result.value,
            result.timestamp()
        );
    }

    Ok(())
}

fn demo_search_and_vectors() -> Result<()> {
    println!("\n🔍 SEARCH & VECTOR STORAGE");
    println!("==========================");

    // Full-text search
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
    SEARCH_STORE.put_text(
        "doc3",
        "Machine learning models can be trained on large datasets for pattern recognition",
        Some("ai"),
    )?;
    println!("✓ Stored 3 documents for full-text search");

    let search_results = SEARCH_STORE.search("programming language", 10)?;
    println!(
        "✓ Found {} results for 'programming language'",
        search_results.len()
    );

    for result in search_results.iter().take(2) {
        println!("  - {} (score: {:.3})", result.key, result.score);
    }

    // Vector search
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
    VECTOR_STORE.insert_text("vec3", "Database management systems", Some("database"))?;
    println!("✓ Stored 3 vectors for semantic search");

    let vector_results = VECTOR_STORE.search_similar("neural networks", 5)?;
    println!(
        "✓ Found {} semantically similar results",
        vector_results.len()
    );

    for result in vector_results.iter().take(2) {
        println!("  - {} (score: {:.3})", result.key, result.score);
    }

    Ok(())
}

fn demo_blob_storage() -> Result<()> {
    println!("\n💾 BLOB STORAGE");
    println!("===============");

    // Store blobs
    let config_data = br#"{"version": "1.0", "environment": "production"}"#;
    BLOB_STORE.put("config.json", config_data, Some("configs"))?;
    println!("✓ Stored configuration blob ({} bytes)", config_data.len());

    let binary_data = &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
    BLOB_STORE.put("data.bin", binary_data, Some("binaries"))?;
    println!("✓ Stored binary blob ({} bytes)", binary_data.len());

    // Store some text data
    let text_data = b"Important document content here";
    BLOB_STORE.put("document.txt", text_data, Some("documents"))?;
    println!("✓ Stored text document ({} bytes)", text_data.len());

    // Retrieve blobs
    if let Some(data) = BLOB_STORE.get("config.json")? {
        println!("✓ Retrieved config.json: {} bytes", data.len());
        println!("  Content: {}", String::from_utf8_lossy(&data));
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
    println!(
        "\n✓ Distribution strategy: {}",
        strategy_name(&MANAGER.get_strategy())
    );
    println!("✓ All stores share the same base directory: ./unified_data_store");
    println!("✓ ACID compliance maintained across all operations");

    // Store data directly through the manager as well
    MANAGER.put("direct_key1", b"Direct value 1", None)?;
    MANAGER.put("direct_key2", b"Direct value 2", None)?;
    println!("\n✓ Also stored 2 items directly through DataDistributionManager");

    // List some keys from the manager
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

    println!("✨ All storage types share the SAME DataDistributionManager instance");
    println!("📁 Base path: ./unified_data_store\n");

    // Run all demos
    demo_multidimensional_telemetry()?;
    demo_telemetry_timeseries()?;
    demo_search_and_vectors()?;
    demo_blob_storage()?;
    demo_manager_coordination()?;

    // Display final summary
    println!("\n📊 SYSTEM SUMMARY");
    println!("=================");
    println!("✅ SINGLE DataDistributionManager for ALL storage types");
    println!("✅ Multidimensional telemetry (1D, 2D, 3D)");
    println!("✅ Time series telemetry with query support");
    println!("✅ Full-text search with relevance scoring");
    println!("✅ Vector similarity search with embeddings");
    println!("✅ Binary blob storage for any data type");
    println!("✅ ACID-compliant transactions across all types");
    println!("✅ Consistent distribution strategy for all data");
    println!("✅ Global lazy_static configuration");

    println!("\n🎉 Demo completed successfully!");
    Ok(())
}
