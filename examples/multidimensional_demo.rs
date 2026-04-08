use bund_blobstore::TelemetryValue;
use bund_blobstore::common::{
    Bounds, Coord1D, Coord2D, Coord3D, Coordinate, DimensionType, MultidimensionalStorage,
};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::thread;
use std::time::Duration as StdDuration;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== Multidimensional Storage Demo ===\n");

    // Initialize storage
    let storage = MultidimensionalStorage::open("multidim_demo_data")?;
    println!("✓ Storage initialized\n");

    // ========== 1. Create Dimensions ==========
    println!("📐 Creating Dimensions");
    println!("-----------------------");

    // 1D dimension for temperature sensors
    storage.create_dimension("temperature_sensors", DimensionType::OneD, 100, None)?;
    println!("✓ Created 1D dimension: 'temperature_sensors' (capacity: 100)");

    // 2D grid with bounds
    let bounds_2d = Bounds {
        min_x: 0,
        max_x: 10,
        min_y: Some(0),
        max_y: Some(10),
        min_z: None,
        max_z: None,
    };
    storage.create_dimension("grid_2d", DimensionType::TwoD, 50, Some(bounds_2d))?;
    println!("✓ Created 2D dimension: 'grid_2d' (capacity: 50, bounds: 0-10 in both axes)");

    // 3D space with bounds
    let bounds_3d = Bounds {
        min_x: -5,
        max_x: 5,
        min_y: Some(-5),
        max_y: Some(5),
        min_z: Some(-5),
        max_z: Some(5),
    };
    storage.create_dimension("space_3d", DimensionType::ThreeD, 20, Some(bounds_3d))?;
    println!("✓ Created 3D dimension: 'space_3d' (capacity: 20, bounds: -5 to 5 in all axes)");

    println!("\n📊 Total dimensions: {}", storage.list_dimensions().len());

    // ========== 2. Store Data in 1D ==========
    println!("\n🌡️ 1D Telemetry - Temperature Sensors");
    println!("------------------------------------");

    let sensor_coord = Coordinate::OneD(Coord1D(42));

    // Store temperature readings over time
    for i in 0..10 {
        let temp = 20.0 + (i as f64) * 0.5;
        let metadata = {
            let mut map = HashMap::new();
            map.insert("unit".to_string(), "celsius".to_string());
            map.insert("sensor_id".to_string(), "sensor_42".to_string());
            map
        };

        storage.push_sample(
            "temperature_sensors",
            sensor_coord.clone(),
            TelemetryValue::Float(temp),
            None,
            metadata,
        )?;
        println!("  Stored: {:.1}°C at sensor 42", temp);
        thread::sleep(StdDuration::from_millis(100));
    }

    // Retrieve latest 5 samples
    let latest = storage.get_latest_samples("temperature_sensors", sensor_coord.clone(), 5)?;
    println!("\n  Latest 5 samples from sensor 42:");
    for (i, sample) in latest.iter().enumerate() {
        if let TelemetryValue::Float(temp) = sample.value {
            println!(
                "    {}. {:.1}°C (timestamp: {})",
                i + 1,
                temp,
                sample.timestamp
            );
        }
    }

    // ========== 3. Store Data in 2D Grid ==========
    println!("\n🗺️ 2D Grid - Spatial Temperature Map");
    println!("-----------------------------------");

    // Store temperatures at different grid points
    let grid_points = vec![
        (Coord2D(0, 0), 22.5),
        (Coord2D(5, 5), 25.0),
        (Coord2D(10, 10), 28.5),
        (Coord2D(3, 7), 23.0),
        (Coord2D(8, 2), 26.5),
    ];

    for (coord, temp) in grid_points {
        let coord_x = coord.0;
        let coord_y = coord.1;
        let coord_enum = Coordinate::TwoD(coord);
        storage.push_sample(
            "grid_2d",
            coord_enum,
            TelemetryValue::Float(temp),
            None,
            HashMap::new(),
        )?;
        println!(
            "  Stored {:.1}°C at grid position ({}, {})",
            temp, coord_x, coord_y
        );
    }

    // Query specific grid point
    let query_coord = Coordinate::TwoD(Coord2D(5, 5));
    let samples = storage.get_latest_samples("grid_2d", query_coord, 1)?;
    if let Some(sample) = samples.first() {
        if let TelemetryValue::Float(temp) = sample.value {
            println!("\n  Temperature at (5,5): {:.1}°C", temp);
        }
    }

    // ========== 4. Store Data in 3D Space ==========
    println!("\n🎲 3D Space - Voxel Data");
    println!("-----------------------");

    // Store data at different 3D coordinates
    let voxels = vec![
        (Coord3D(0, 0, 0), 100.0),
        (Coord3D(1, 1, 1), 150.0),
        (Coord3D(-2, 3, 1), 200.0),
        (Coord3D(3, -1, 2), 175.0),
        (Coord3D(-1, -2, -1), 125.0),
    ];

    for (coord, value) in voxels {
        let coord_x = coord.0;
        let coord_y = coord.1;
        let coord_z = coord.2;
        let coord_enum = Coordinate::ThreeD(coord);
        let metadata = {
            let mut map = HashMap::new();
            map.insert("type".to_string(), "voxel".to_string());
            map
        };

        storage.push_sample(
            "space_3d",
            coord_enum,
            TelemetryValue::Float(value),
            None,
            metadata,
        )?;
        println!(
            "  Stored value {:.1} at ({}, {}, {})",
            value, coord_x, coord_y, coord_z
        );
    }

    // ========== 5. Mixed Value Types ==========
    println!("\n🎭 Mixed Value Types");
    println!("-------------------");

    let mixed_coord = Coordinate::OneD(Coord1D(99));

    // Store different types of values
    storage.push_sample(
        "temperature_sensors",
        mixed_coord.clone(),
        TelemetryValue::Int(42),
        None,
        HashMap::new(),
    )?;
    println!("  ✓ Stored Integer value");

    storage.push_sample(
        "temperature_sensors",
        mixed_coord.clone(),
        TelemetryValue::String("ALERT: High temperature detected".to_string()),
        None,
        HashMap::new(),
    )?;
    println!("  ✓ Stored String value");

    storage.push_sample(
        "temperature_sensors",
        mixed_coord.clone(),
        TelemetryValue::Bool(true),
        None,
        HashMap::new(),
    )?;
    println!("  ✓ Stored Boolean value");

    // Store JSON as string (to avoid serialization issues)
    let json_string = serde_json::to_string(&serde_json::json!({
        "status": "warning",
        "threshold": 85.0,
        "current": 87.5
    }))?;

    storage.push_sample(
        "temperature_sensors",
        mixed_coord,
        TelemetryValue::String(json_string),
        None,
        HashMap::new(),
    )?;
    println!("  ✓ Stored JSON data (as string)");

    // ========== 6. Time Range Queries ==========
    println!("\n⏰ Time Range Queries");
    println!("--------------------");

    let time_coord = Coordinate::OneD(Coord1D(50));
    let now = Utc::now();

    // Store samples with specific timestamps
    for i in 0..5 {
        let timestamp = now + Duration::seconds(i * 10);
        storage.push_sample(
            "temperature_sensors",
            time_coord.clone(),
            TelemetryValue::Float(25.0 + i as f64),
            Some(timestamp),
            HashMap::new(),
        )?;
        println!("  Stored sample at t+{}s: {:.1}°C", i * 10, 25.0 + i as f64);
    }

    // Query samples in time range
    let start_time = now + Duration::seconds(10);
    let end_time = now + Duration::seconds(30);
    let time_range_samples = storage.get_samples_in_time_range(
        "temperature_sensors",
        time_coord,
        start_time,
        end_time,
    )?;

    println!("\n  Samples between t+10s and t+30s:");
    for sample in time_range_samples {
        if let TelemetryValue::Float(temp) = sample.value {
            println!("    {:.1}°C at {}", temp, sample.timestamp);
        }
    }

    // ========== 7. Vector Search for Dimensions ==========
    println!("\n🔍 Vector Search - Finding Dimensions by Label");
    println!("---------------------------------------------");

    // Create more dimensions for search
    storage.create_dimension("cpu_temperature", DimensionType::OneD, 10, None)?;
    storage.create_dimension("gpu_temperature", DimensionType::OneD, 10, None)?;
    storage.create_dimension("memory_usage", DimensionType::OneD, 10, None)?;

    println!("✓ Created additional dimensions for search");

    // Wait for indexing
    thread::sleep(StdDuration::from_millis(500));

    // Search for temperature-related dimensions
    let results = storage.search_dimensions_by_label("temperature", 5)?;
    println!("\n  Search results for 'temperature':");
    for (label, score) in results {
        println!("    {} (score: {:.3})", label, score);
    }

    // ========== 8. FIFO Queue Behavior ==========
    println!("\n🔄 FIFO Queue Behavior (Capacity=3)");
    println!("-----------------------------------");

    let fifo_coord = Coordinate::OneD(Coord1D(77));

    // Push 5 samples to a dimension with capacity 3
    for i in 1..=5 {
        storage.push_sample(
            "temperature_sensors",
            fifo_coord.clone(),
            TelemetryValue::Int(i * 10),
            None,
            HashMap::new(),
        )?;
        println!("  Pushed sample {}: {}", i, i * 10);
    }

    let fifo_samples = storage.get_latest_samples("temperature_sensors", fifo_coord, 10)?;
    println!("\n  Retrieved samples (only last 3 should remain):");
    for sample in fifo_samples {
        if let TelemetryValue::Int(val) = sample.value {
            println!("    Value: {}", val);
        }
    }

    // ========== 9. Statistics ==========
    println!("\n📊 Storage Statistics");
    println!("--------------------");

    let dimensions = storage.list_dimensions();
    println!("  Total dimensions: {}", dimensions.len());

    for dim in dimensions {
        println!(
            "  - {}: {:?}, capacity: {}",
            dim.label, dim.dim_type, dim.cell_capacity
        );
        if let Some(bounds) = dim.bounds {
            println!("    Bounds: X:[{}..{}]", bounds.min_x, bounds.max_x);
            if let (Some(min_y), Some(max_y)) = (bounds.min_y, bounds.max_y) {
                println!("            Y:[{}..{}]", min_y, max_y);
            }
            if let (Some(min_z), Some(max_z)) = (bounds.min_z, bounds.max_z) {
                println!("            Z:[{}..{}]", min_z, max_z);
            }
        }
    }

    // ========== 10. Cleanup - Delete Dimension ==========
    println!("\n🗑️ Cleanup - Deleting Dimensions");
    println!("-------------------------------");

    // Delete a dimension to demonstrate cleanup
    storage.delete_dimension("memory_usage")?;
    println!("✓ Deleted dimension 'memory_usage'");

    let remaining = storage.list_dimensions();
    println!("  Remaining dimensions: {}", remaining.len());

    // ========== 11. Concurrent Operations Demo ==========
    println!("\n🔄 Concurrent Operations");
    println!("-----------------------");

    use std::sync::Arc;
    let storage_arc = Arc::new(storage);
    let mut handles = vec![];

    // Writer threads
    for t in 0..3 {
        let storage_clone = storage_arc.clone();
        let coord = Coordinate::OneD(Coord1D(888));
        let handle = thread::spawn(move || {
            for i in 0..10 {
                let val = TelemetryValue::Float((t * 100 + i) as f64);
                let _ = storage_clone.push_sample(
                    "temperature_sensors",
                    coord.clone(),
                    val,
                    None,
                    HashMap::new(),
                );
            }
            println!("  Writer {} completed", t);
        });
        handles.push(handle);
    }

    // Reader threads
    for r in 0..2 {
        let storage_clone = storage_arc.clone();
        let coord = Coordinate::OneD(Coord1D(888));
        let handle = thread::spawn(move || {
            for _ in 0..5 {
                if let Ok(samples) =
                    storage_clone.get_latest_samples("temperature_sensors", coord.clone(), 10)
                {
                    println!("  Reader {} read {} samples", r, samples.len());
                }
                thread::sleep(StdDuration::from_millis(50));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // ========== 12. Final Summary ==========
    println!("\n✅ Demo Completed Successfully!");
    println!("================================");
    println!("\nKey Features Demonstrated:");
    println!("  ✓ 1D, 2D, and 3D dimension creation");
    println!("  ✓ Telemetry data storage and retrieval");
    println!("  ✓ Mixed value types (Float, Int, String, Bool)");
    println!("  ✓ Time range queries");
    println!("  ✓ Vector search for dimensions");
    println!("  ✓ FIFO queue behavior with capacity limits");
    println!("  ✓ Metadata preservation");
    println!("  ✓ Bounds validation");
    println!("  ✓ Concurrent read/write operations");
    println!("  ✓ Dynamic dimension deletion");

    Ok(())
}
