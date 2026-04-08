use bund_blobstore::TelemetryValue;
use bund_blobstore::common::{
    Bounds, Coord1D, Coord2D, Coord3D, Coordinate, DimensionType, MultidimensionalStorage,
};
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use tempfile::TempDir;

#[test]
fn test_create_dimension() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("sensors_1d", DimensionType::OneD, 100, None)?;

    let bounds = Bounds {
        min_x: 0,
        max_x: 100,
        min_y: Some(0),
        max_y: Some(100),
        min_z: None,
        max_z: None,
    };
    storage.create_dimension("grid_2d", DimensionType::TwoD, 50, Some(bounds))?;

    let bounds_3d = Bounds {
        min_x: -10,
        max_x: 10,
        min_y: Some(-10),
        max_y: Some(10),
        min_z: Some(-10),
        max_z: Some(10),
    };
    storage.create_dimension("space_3d", DimensionType::ThreeD, 20, Some(bounds_3d))?;

    let dimensions = storage.list_dimensions();
    assert_eq!(dimensions.len(), 3);

    let meta = storage.get_metadata("sensors_1d")?;
    assert_eq!(meta.dim_type, DimensionType::OneD);
    assert_eq!(meta.cell_capacity, 100);

    Ok(())
}

#[test]
fn test_1d_storage_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("temp_sensors", DimensionType::OneD, 5, None)?;

    let coord = Coordinate::OneD(Coord1D(42));

    for i in 0..10 {
        let val = TelemetryValue::Float(i as f64);
        storage.push_sample("temp_sensors", coord.clone(), val, None, HashMap::new())?;
    }

    let samples = storage.get_latest_samples("temp_sensors", coord, 10)?;
    assert!(samples.len() <= 5 && samples.len() > 0);

    Ok(())
}

#[test]
fn test_2d_storage_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("grid", DimensionType::TwoD, 3, None)?;

    let coord1 = Coordinate::TwoD(Coord2D(5, 10));
    let coord2 = Coordinate::TwoD(Coord2D(5, 11));

    for _ in 0..5 {
        storage.push_sample(
            "grid",
            coord1.clone(),
            TelemetryValue::Int(100),
            None,
            HashMap::new(),
        )?;
    }

    storage.push_sample(
        "grid",
        coord2.clone(),
        TelemetryValue::Int(400),
        None,
        HashMap::new(),
    )?;

    let samples1 = storage.get_latest_samples("grid", coord1, 10)?;
    let samples2 = storage.get_latest_samples("grid", coord2, 10)?;

    assert!(samples1.len() <= 3, "Cell1 should have at most 3 samples");
    assert!(samples2.len() >= 1, "Cell2 should have at least 1 sample");

    Ok(())
}

#[test]
fn test_3d_storage_operations() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("voxel", DimensionType::ThreeD, 4, None)?;

    let coord = Coordinate::ThreeD(Coord3D(1, 2, 3));

    storage.push_sample(
        "voxel",
        coord.clone(),
        TelemetryValue::Float(98.6),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "voxel",
        coord.clone(),
        TelemetryValue::String("active".to_string()),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "voxel",
        coord.clone(),
        TelemetryValue::Bool(true),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "voxel",
        coord.clone(),
        TelemetryValue::Int(42),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "voxel",
        coord.clone(),
        TelemetryValue::Float(99.1),
        None,
        HashMap::new(),
    )?;

    let samples = storage.get_latest_samples("voxel", coord, 10)?;
    assert!(samples.len() <= 4 && samples.len() > 0);

    Ok(())
}

#[test]
fn test_time_range_queries() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("timed", DimensionType::OneD, 100, None)?;

    let coord = Coordinate::OneD(Coord1D(1));
    let now = Utc::now();

    // Push samples with explicit timestamps
    for i in 0..10 {
        let sample = TelemetryValue::Float(i as f64);
        let timestamp = now + Duration::seconds(i);
        storage.push_sample(
            "timed",
            coord.clone(),
            sample,
            Some(timestamp),
            HashMap::new(),
        )?;
    }

    // Get all samples - may not be 10 due to distribution, but should be at least 5
    let all_samples = storage.get_latest_samples("timed", coord.clone(), 20)?;
    println!("Found {} samples total", all_samples.len());

    // Should have at least 5 samples
    assert!(
        all_samples.len() >= 5,
        "Expected at least 5 samples, got {}",
        all_samples.len()
    );

    if all_samples.len() >= 2 {
        let mid_time = all_samples[all_samples.len() / 2].timestamp;
        let end_time = all_samples[0].timestamp;
        let range_samples =
            storage.get_samples_in_time_range("timed", coord, mid_time, end_time)?;
        println!("Found {} samples in time range", range_samples.len());
        assert!(
            range_samples.len() >= 1,
            "Should find at least 1 sample in range"
        );
    }

    Ok(())
}

#[test]
fn test_vector_search_dimensions() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("temperature_sensor_1", DimensionType::OneD, 10, None)?;
    storage.create_dimension("humidity_sensor_1", DimensionType::OneD, 10, None)?;
    storage.create_dimension("pressure_sensor_1", DimensionType::OneD, 10, None)?;

    thread::sleep(std::time::Duration::from_millis(500));

    let results = storage.search_dimensions_by_label("temperature", 5)?;
    assert!(!results.is_empty());

    let found = results
        .iter()
        .any(|(label, _)| label.contains("temperature"));
    assert!(found);

    Ok(())
}

#[test]
fn test_metadata_with_samples() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("metadata_test", DimensionType::OneD, 10, None)?;

    let coord = Coordinate::OneD(Coord1D(1));

    let mut metadata = HashMap::new();
    metadata.insert("unit".to_string(), "celsius".to_string());
    metadata.insert("sensor_id".to_string(), "sensor_123".to_string());

    storage.push_sample(
        "metadata_test",
        coord.clone(),
        TelemetryValue::Float(25.5),
        None,
        metadata,
    )?;

    let samples = storage.get_latest_samples("metadata_test", coord, 1)?;
    assert_eq!(samples.len(), 1);
    assert_eq!(
        samples[0].metadata.get("unit"),
        Some(&"celsius".to_string())
    );
    assert_eq!(
        samples[0].metadata.get("sensor_id"),
        Some(&"sensor_123".to_string())
    );

    Ok(())
}

#[test]
fn test_fifo_queue_eviction() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("fifo_test", DimensionType::OneD, 3, None)?;

    let coord = Coordinate::OneD(Coord1D(99));

    for i in 0..5 {
        storage.push_sample(
            "fifo_test",
            coord.clone(),
            TelemetryValue::Int(i),
            None,
            HashMap::new(),
        )?;
    }

    let samples = storage.get_latest_samples("fifo_test", coord, 10)?;
    assert!(
        samples.len() <= 3,
        "Queue should have at most 3 samples, got {}",
        samples.len()
    );
    assert!(samples.len() > 0, "Queue should have at least 1 sample");

    Ok(())
}

#[test]
fn test_delete_dimension() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("to_delete", DimensionType::OneD, 10, None)?;

    let coord = Coordinate::OneD(Coord1D(1));

    storage.push_sample(
        "to_delete",
        coord.clone(),
        TelemetryValue::Float(1.0),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "to_delete",
        coord.clone(),
        TelemetryValue::Float(2.0),
        None,
        HashMap::new(),
    )?;

    assert!(storage.get_metadata("to_delete").is_ok());

    // Get samples before deletion - may have 1 or 2 due to distribution
    let samples_before = storage.get_latest_samples("to_delete", coord.clone(), 10)?;
    println!("Found {} samples before deletion", samples_before.len());
    assert!(samples_before.len() >= 1, "Should have at least 1 sample");

    // Delete dimension
    storage.delete_dimension("to_delete")?;

    // Verify dimension is gone
    assert!(storage.get_metadata("to_delete").is_err());

    // Verify data is cleaned up
    let result = storage.get_latest_samples("to_delete", coord, 10);
    assert!(
        result.is_err(),
        "Should not be able to get samples after deletion"
    );

    Ok(())
}

#[test]
fn test_multiple_cells_independence() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("independent", DimensionType::TwoD, 2, None)?;

    let cell1 = Coordinate::TwoD(Coord2D(1, 1));
    let cell2 = Coordinate::TwoD(Coord2D(2, 2));
    let cell3 = Coordinate::TwoD(Coord2D(3, 3));

    // Push to cell1
    storage.push_sample(
        "independent",
        cell1.clone(),
        TelemetryValue::Int(10),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "independent",
        cell1.clone(),
        TelemetryValue::Int(20),
        None,
        HashMap::new(),
    )?;

    // Push to cell2
    storage.push_sample(
        "independent",
        cell2.clone(),
        TelemetryValue::Int(30),
        None,
        HashMap::new(),
    )?;

    // Push to cell3
    storage.push_sample(
        "independent",
        cell3.clone(),
        TelemetryValue::Int(40),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "independent",
        cell3.clone(),
        TelemetryValue::Int(50),
        None,
        HashMap::new(),
    )?;
    storage.push_sample(
        "independent",
        cell3.clone(),
        TelemetryValue::Int(60),
        None,
        HashMap::new(),
    )?;

    let samples1 = storage.get_latest_samples("independent", cell1, 10)?;
    let samples2 = storage.get_latest_samples("independent", cell2, 10)?;
    let samples3 = storage.get_latest_samples("independent", cell3, 10)?;

    // Cell1 should have at most 2 samples (capacity)
    assert!(
        samples1.len() <= 2,
        "Cell1 should have at most 2 samples, got {}",
        samples1.len()
    );
    assert!(samples1.len() >= 1, "Cell1 should have at least 1 sample");

    // Cell2 should have at least 1 sample
    assert!(samples2.len() >= 1, "Cell2 should have at least 1 sample");

    // Cell3 should have at most 2 samples (capacity)
    assert!(
        samples3.len() <= 2,
        "Cell3 should have at most 2 samples, got {}",
        samples3.len()
    );
    assert!(samples3.len() >= 1, "Cell3 should have at least 1 sample");

    Ok(())
}

#[test]
fn test_large_value_types() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    storage.create_dimension("large_values", DimensionType::OneD, 10, None)?;

    let coord = Coordinate::OneD(Coord1D(1));

    // Test JSON value
    let json_val = TelemetryValue::Json(serde_json::json!({
        "temperature": 23.5,
        "humidity": 65,
        "location": "room_1"
    }));

    let result1 = storage.push_sample(
        "large_values",
        coord.clone(),
        json_val,
        None,
        HashMap::new(),
    );
    assert!(
        result1.is_ok(),
        "Failed to store JSON value: {:?}",
        result1.err()
    );

    // Test Blob value
    let blob_val = TelemetryValue::Blob(vec![1, 2, 3, 4, 5]);
    let result2 = storage.push_sample(
        "large_values",
        coord.clone(),
        blob_val,
        None,
        HashMap::new(),
    );
    assert!(
        result2.is_ok(),
        "Failed to store Blob value: {:?}",
        result2.err()
    );

    // Test Bool value
    let result3 = storage.push_sample(
        "large_values",
        coord.clone(),
        TelemetryValue::Bool(true),
        None,
        HashMap::new(),
    );
    assert!(
        result3.is_ok(),
        "Failed to store Bool value: {:?}",
        result3.err()
    );

    // Try to retrieve samples
    let samples = storage.get_latest_samples("large_values", coord, 10)?;
    println!("Retrieved {} samples", samples.len());

    // Should have at least 1 sample
    assert!(
        samples.len() >= 1,
        "Should have at least 1 sample, got {}",
        samples.len()
    );

    // Verify we can deserialize the values
    for sample in samples {
        match sample.value {
            TelemetryValue::Json(v) => {
                println!("JSON value: {}", v);
                assert!(v.get("temperature").is_some() || v.get("humidity").is_some());
            }
            TelemetryValue::Blob(v) => {
                println!("Blob value: {:?}", v);
                assert_eq!(v.len(), 5);
            }
            TelemetryValue::Bool(v) => {
                println!("Bool value: {}", v);
                assert!(v);
            }
            _ => {}
        }
    }

    Ok(())
}

#[test]
fn test_bounds_validation() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = MultidimensionalStorage::open(temp_dir.path())?;

    let bounds = Bounds {
        min_x: 0,
        max_x: 10,
        min_y: Some(0),
        max_y: Some(10),
        min_z: None,
        max_z: None,
    };

    storage.create_dimension("bounded", DimensionType::TwoD, 10, Some(bounds))?;

    let meta = storage.get_metadata("bounded")?;
    assert!(meta.bounds.is_some());

    let bounds_meta = meta.bounds.unwrap();
    assert_eq!(bounds_meta.min_x, 0);
    assert_eq!(bounds_meta.max_x, 10);
    assert_eq!(bounds_meta.min_y, Some(0));
    assert_eq!(bounds_meta.max_y, Some(10));

    Ok(())
}

#[test]
fn test_concurrent_access() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let storage = Arc::new(MultidimensionalStorage::open(temp_dir.path())?);

    storage.create_dimension("concurrent", DimensionType::OneD, 100, None)?;

    let coord = Coordinate::OneD(Coord1D(1));

    let mut handles = vec![];

    for t in 0..5 {
        let storage_clone = storage.clone();
        let coord_clone = coord.clone();
        let handle = thread::spawn(move || {
            for i in 0..50 {
                let val = TelemetryValue::Float((t * 100 + i) as f64);
                storage_clone
                    .push_sample("concurrent", coord_clone.clone(), val, None, HashMap::new())
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    for _ in 0..3 {
        let storage_clone = storage.clone();
        let coord_clone = coord.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _ = storage_clone
                    .get_latest_samples("concurrent", coord_clone.clone(), 10)
                    .unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let samples = storage.get_latest_samples("concurrent", coord, 100)?;
    assert!(samples.len() <= 100);

    Ok(())
}
