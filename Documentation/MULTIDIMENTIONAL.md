# Multidimensional Storage Documentation

## Overview

The Multidimensional Storage module provides a powerful, scalable solution for storing and querying telemetry data in 1D, 2D, and 3D coordinate spaces. Each cell maintains a fixed-size FIFO queue of telemetry samples, with automatic distribution across shards using round-robin policy.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Multidimensional Storage                   │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │  1D Space   │  │  2D Grid    │  │  3D Voxel   │          │
│  │ (Sensors)   │  │  (Map)      │  │  (Space)    │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         │                │                │                  │
│         ▼                ▼                ▼                  │
│  ┌─────────────────────────────────────────────────┐         │
│  │         DataDistributionManager                 │         │
│  │    (Round-Robin Sharding)                       │         │
│  └─────────────────────────────────────────────────┘         │
│         │                │                │                  │
│         ▼                ▼                ▼                  │
│  ┌──────────┐     ┌──────────┐     ┌──────────┐             │
│  │ Shard 0  │     │ Shard 1  │     │ Shard N  │             │
│  │ Samples  │     │ Samples  │     │ Samples  │             │
│  └──────────┘     └──────────┘     └──────────┘             │
└─────────────────────────────────────────────────────────────┘
```

## Core Concepts

### Dimensions
- **1D (One-Dimensional)**: Linear space for sensors, time series, or sequential data
- **2D (Two-Dimensional)**: Grid space for maps, matrices, or spatial data
- **3D (Three-Dimensional)**: Volume space for voxels, 3D scans, or spatial-temporal data

### Cells
Each coordinate point in a dimension represents a cell that contains:
- **Fixed-size FIFO queue** of sample IDs (configurable capacity)
- **Automatic eviction** of oldest samples when capacity is reached
- **Metadata** associated with each sample

### Samples
Individual telemetry data points stored as `TelemetryValue` with:
- **Unique ID** (UUID v4)
- **Timestamp** (UTC)
- **Value** (Float, Int, String, Bool, Blob, Json, Dynamic, Null)
- **Custom metadata** (HashMap<String, String>)

## Features

### 1. Dimension Creation

Create 1D, 2D, or 3D dimensions with configurable cell capacity and optional bounds.

```rust
use bund_blobstore::common::{
    MultidimensionalStorage, DimensionType, Bounds
};

let storage = MultidimensionalStorage::open("data.db")?;

// 1D dimension (no bounds)
storage.create_dimension("sensors", DimensionType::OneD, 100, None)?;

// 2D dimension with bounds
let bounds = Bounds {
    min_x: 0,
    max_x: 100,
    min_y: Some(0),
    max_y: Some(100),
    min_z: None,
    max_z: None,
};
storage.create_dimension("grid", DimensionType::TwoD, 50, Some(bounds))?;

// 3D dimension with bounds
let bounds_3d = Bounds {
    min_x: -10,
    max_x: 10,
    min_y: Some(-10),
    max_y: Some(10),
    min_z: Some(-10),
    max_z: Some(10),
};
storage.create_dimension("voxel_space", DimensionType::ThreeD, 20, Some(bounds_3d))?;
```

### 2. Storing Telemetry Data

Push samples to specific coordinates with automatic timestamp generation.

```rust
use bund_blobstore::common::{Coordinate, Coord1D, Coord2D, Coord3D};
use bund_blobstore::TelemetryValue;
use std::collections::HashMap;

// 1D storage
let coord_1d = Coordinate::OneD(Coord1D(42));
let metadata = {
    let mut map = HashMap::new();
    map.insert("unit".to_string(), "celsius".to_string());
    map
};

storage.push_sample(
    "temperature_sensors",
    coord_1d,
    TelemetryValue::Float(23.5),
    None,  // Auto-generate timestamp
    metadata,
)?;

// 2D storage
let coord_2d = Coordinate::TwoD(Coord2D(5, 10));
storage.push_sample(
    "grid",
    coord_2d,
    TelemetryValue::Int(100),
    None,
    HashMap::new(),
)?;

// 3D storage with custom timestamp
let coord_3d = Coordinate::ThreeD(Coord3D(1, 2, 3));
let custom_time = Utc::now();
storage.push_sample(
    "voxel_space",
    coord_3d,
    TelemetryValue::String("active".to_string()),
    Some(custom_time),
    HashMap::new(),
)?;
```

### 3. Retrieving Latest Samples

Get the most recent N samples from any cell.

```rust
// Get latest 10 samples from sensor 42
let samples = storage.get_latest_samples("temperature_sensors", coord_1d, 10)?;

for sample in samples {
    match sample.value {
        TelemetryValue::Float(temp) => {
            println!("Temperature: {}°C at {}", temp, sample.timestamp);
        }
        TelemetryValue::Int(val) => {
            println!("Value: {} at {}", val, sample.timestamp);
        }
        _ => println!("Other value type"),
    }
}
```

### 4. Time Range Queries

Query samples within specific time windows.

```rust
let start_time = Utc::now() - Duration::hours(1);
let end_time = Utc::now();

let samples = storage.get_samples_in_time_range(
    "temperature_sensors",
    coord_1d,
    start_time,
    end_time,
)?;

println!("Found {} samples in the last hour", samples.len());
```

### 5. Mixed Value Types

Store various telemetry value types in the same dimension.

```rust
// Store different value types
storage.push_sample("sensors", coord.clone(), TelemetryValue::Float(98.6), None, HashMap::new())?;
storage.push_sample("sensors", coord.clone(), TelemetryValue::Int(42), None, HashMap::new())?;
storage.push_sample("sensors", coord.clone(), TelemetryValue::String("warning".to_string()), None, HashMap::new())?;
storage.push_sample("sensors", coord.clone(), TelemetryValue::Bool(true), None, HashMap::new())?;

// Store JSON as string (recommended for bincode compatibility)
let json_data = serde_json::json!({
    "status": "active",
    "count": 150,
    "tags": ["sensor", "critical"]
});
let json_string = serde_json::to_string(&json_data)?;
storage.push_sample("sensors", coord, TelemetryValue::String(json_string), None, HashMap::new())?;
```

### 6. FIFO Queue Behavior

Automatic management of queue capacity - oldest samples are evicted when capacity is exceeded.

```rust
// Create dimension with capacity 3
storage.create_dimension("fifo_test", DimensionType::OneD, 3, None)?;

let coord = Coordinate::OneD(Coord1D(1));

// Push 5 samples (only last 3 will be kept)
for i in 1..=5 {
    storage.push_sample("fifo_test", coord.clone(), TelemetryValue::Int(i), None, HashMap::new())?;
}

// Retrieve samples - only values 3, 4, 5 remain
let samples = storage.get_latest_samples("fifo_test", coord, 10)?;
assert_eq!(samples.len(), 3);
```

### 7. Vector Search for Dimensions

Find dimensions by semantic similarity using vector embeddings.

```rust
// Create multiple dimensions with descriptive names
storage.create_dimension("cpu_temperature", DimensionType::OneD, 100, None)?;
storage.create_dimension("gpu_temperature", DimensionType::OneD, 100, None)?;
storage.create_dimension("memory_usage", DimensionType::OneD, 100, None)?;
storage.create_dimension("disk_io", DimensionType::OneD, 100, None)?;

// Wait for indexing (async in production)
std::thread::sleep(std::time::Duration::from_millis(500));

// Search for temperature-related dimensions
let results = storage.search_dimensions_by_label("temperature", 5)?;
for (label, score) in results {
    println!("Found: {} (score: {:.3})", label, score);
}
// Output: cpu_temperature (0.85), gpu_temperature (0.82)
```

### 8. Metadata Preservation

Store additional information with each sample.

```rust
let mut metadata = HashMap::new();
metadata.insert("unit".to_string(), "celsius".to_string());
metadata.insert("sensor_id".to_string(), "sensor_123".to_string());
metadata.insert("location".to_string(), "room_42".to_string());
metadata.insert("calibration_date".to_string(), "2024-01-15".to_string());

storage.push_sample(
    "sensors",
    coord,
    TelemetryValue::Float(23.5),
    None,
    metadata,
)?;

// Retrieve and access metadata
let samples = storage.get_latest_samples("sensors", coord, 1)?;
if let Some(sample) = samples.first() {
    println!("Unit: {:?}", sample.metadata.get("unit"));
    println!("Sensor: {:?}", sample.metadata.get("sensor_id"));
}
```

### 9. Bounds Validation

Define and enforce bounds for dimensions.

```rust
let bounds = Bounds {
    min_x: -100,
    max_x: 100,
    min_y: Some(-100),
    max_y: Some(100),
    min_z: Some(-100),
    max_z: Some(100),
};

storage.create_dimension("bounded_space", DimensionType::ThreeD, 100, Some(bounds))?;

// Coordinates within bounds work
let valid_coord = Coordinate::ThreeD(Coord3D(50, 50, 50));
storage.push_sample("bounded_space", valid_coord, TelemetryValue::Int(42), None, HashMap::new())?;

// Coordinates outside bounds would be rejected at storage level
// (though currently not enforced, bounds are metadata only)
```

### 10. Concurrent Operations

Thread-safe operations with multiple readers and writers.

```rust
use std::sync::Arc;
use std::thread;

let storage = Arc::new(MultidimensionalStorage::open("concurrent.db")?);
storage.create_dimension("concurrent_test", DimensionType::OneD, 1000, None)?;

let coord = Coordinate::OneD(Coord1D(1));
let mut handles = vec![];

// Writer threads
for t in 0..5 {
    let storage_clone = storage.clone();
    let coord_clone = coord.clone();
    let handle = thread::spawn(move || {
        for i in 0..100 {
            storage_clone.push_sample(
                "concurrent_test",
                coord_clone.clone(),
                TelemetryValue::Float(i as f64),
                None,
                HashMap::new(),
            ).unwrap();
        }
    });
    handles.push(handle);
}

// Reader threads
for _ in 0..3 {
    let storage_clone = storage.clone();
    let coord_clone = coord.clone();
    let handle = thread::spawn(move || {
        for _ in 0..50 {
            let _ = storage_clone.get_latest_samples("concurrent_test", coord_clone.clone(), 10);
        }
    });
    handles.push(handle);
}

for handle in handles {
    handle.join().unwrap();
}
```

### 11. Dimension Management

List, query, and delete dimensions.

```rust
// List all dimensions
let dimensions = storage.list_dimensions();
for dim in dimensions {
    println!("Dimension: {}", dim.label);
    println!("  Type: {:?}", dim.dim_type);
    println!("  Capacity: {}", dim.cell_capacity);
    println!("  Created: {}", dim.created_at);
}

// Get specific dimension metadata
let metadata = storage.get_metadata("temperature_sensors")?;
println!("Cell capacity: {}", metadata.cell_capacity);

// Delete a dimension (removes all associated data)
storage.delete_dimension("old_sensors")?;
```

### 12. Advanced Querying

Combine multiple query patterns for complex analysis.

```rust
// Get latest samples, then filter by value
let all_samples = storage.get_latest_samples("sensors", coord, 100)?;
let high_temps: Vec<_> = all_samples.iter()
    .filter(|s| {
        matches!(s.value, TelemetryValue::Float(v) if v > 30.0)
    })
    .collect();

// Time-based aggregation
let hourly_data = storage.get_samples_in_time_range("sensors", coord, one_hour_ago, now)?;
let avg_temp: f64 = hourly_data.iter()
    .filter_map(|s| {
        if let TelemetryValue::Float(v) = s.value {
            Some(v)
        } else {
            None
        }
    })
    .sum::<f64>() / hourly_data.len() as f64;
```

## Performance Considerations

### Sharding
- Data is automatically distributed across shards using round-robin
- Each sample is stored independently for efficient retrieval
- Cell queues store only sample IDs (small metadata)

### Memory Usage
- Cell queues are memory-efficient (store only IDs)
- Sample data is persisted on disk
- Metadata caching for dimension information

### Concurrency
- All operations are thread-safe
- Multiple readers can access simultaneously
- Writers are serialized per cell

## Best Practices

### 1. Capacity Planning
```rust
// Choose capacity based on data retention needs
// 100 samples = ~100 data points per cell
storage.create_dimension("short_term", DimensionType::OneD, 100, None)?;

// 10000 samples = ~10K data points per cell (higher memory usage)
storage.create_dimension("long_term", DimensionType::OneD, 10000, None)?;
```

### 2. Metadata Usage
```rust
// Use metadata for filtering and categorization
let mut metadata = HashMap::new();
metadata.insert("type".to_string(), "temperature".to_string());
metadata.insert("unit".to_string(), "celsius".to_string());
metadata.insert("importance".to_string(), "critical".to_string());
```

### 3. Batch Operations
```rust
// For large datasets, consider batching
for batch in data.chunks(100) {
    for (coord, value) in batch {
        storage.push_sample("sensors", coord.clone(), value, None, HashMap::new())?;
    }
    // Optional: sync periodically
}
```

### 4. Error Handling
```rust
match storage.push_sample("sensors", coord, value, None, HashMap::new()) {
    Ok(id) => println!("Stored sample: {}", id),
    Err(e) => eprintln!("Failed to store: {}", e),
}
```

## Complete Example

```rust
use bund_blobstore::common::{
    MultidimensionalStorage, DimensionType, Coordinate, Coord1D, Bounds
};
use bund_blobstore::TelemetryValue;
use chrono::{Utc, Duration};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize storage
    let storage = MultidimensionalStorage::open("telemetry.db")?;
    
    // Create dimensions
    storage.create_dimension("sensors", DimensionType::OneD, 1000, None)?;
    
    let bounds = Bounds {
        min_x: 0,
        max_x: 100,
        min_y: Some(0),
        max_y: Some(100),
        min_z: None,
        max_z: None,
    };
    storage.create_dimension("grid", DimensionType::TwoD, 500, Some(bounds))?;
    
    // Store data
    let sensor_coord = Coordinate::OneD(Coord1D(42));
    let mut metadata = HashMap::new();
    metadata.insert("unit".to_string(), "celsius".to_string());
    
    for i in 0..100 {
        let temp = 20.0 + (i as f64) * 0.1;
        storage.push_sample(
            "sensors",
            sensor_coord.clone(),
            TelemetryValue::Float(temp),
            None,
            metadata.clone(),
        )?;
    }
    
    // Query data
    let samples = storage.get_latest_samples("sensors", sensor_coord, 10)?;
    println!("Latest 10 samples:");
    for sample in samples {
        if let TelemetryValue::Float(temp) = sample.value {
            println!("  {}°C at {}", temp, sample.timestamp);
        }
    }
    
    // Time-based query
    let one_hour_ago = Utc::now() - Duration::hours(1);
    let recent = storage.get_samples_in_time_range(
        "sensors",
        sensor_coord,
        one_hour_ago,
        Utc::now(),
    )?;
    println!("Found {} samples in last hour", recent.len());
    
    Ok(())
}
```

## Limitations and Known Issues

1. **JSON Serialization**: Bincode doesn't handle `serde_json::Value` well. Store JSON as strings.
2. **Bounds Enforcement**: Currently metadata only; no runtime validation.
3. **Vector Search**: Requires indexing time; searches may not be real-time.
4. **Capacity**: Each cell's queue is fixed-size; oldest data is lost when full.

## Troubleshooting

| Issue | Solution |
|-------|----------|
| "DeserializeAnyNotSupported" | Store JSON as String instead of Json variant |
| Slow vector search | Increase sleep time after dimension creation |
| Missing samples | Check cell capacity; older samples are evicted |
| Concurrent access errors | Use Arc<MultidimensionalStorage> for shared access |

## API Reference

### Core Types
- `MultidimensionalStorage`: Main storage interface
- `DimensionType`: OneD, TwoD, ThreeD
- `Coordinate`: Unified coordinate enum
- `TelemetrySample`: Stored data point
- `TelemetryValue`: Value variants

### Key Methods
- `open()`: Initialize storage
- `create_dimension()`: Create new dimension
- `push_sample()`: Store telemetry data
- `get_latest_samples()`: Retrieve recent samples
- `get_samples_in_time_range()`: Time-based queries
- `search_dimensions_by_label()`: Vector search
- `delete_dimension()`: Remove dimension and data

## Conclusion

The Multidimensional Storage module provides a flexible, scalable solution for telemetry data storage across 1D, 2D, and 3D spaces with automatic sharding, FIFO queue management, and powerful querying capabilities. It's ideal for IoT sensor networks, spatial data analysis, time-series databases, and real-time monitoring systems.
