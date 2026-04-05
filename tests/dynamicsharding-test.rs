use bund_blobstore::{AdaptiveConfig, DataDistributionManager, DistributionStrategy};
use chrono::{Duration, Utc};
use std::sync::Arc;
use std::thread;
use std::time::Duration as StdDuration;
use tempfile::TempDir;

#[test]
fn test_dynamic_shard_management() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Initial shard count should be 4
    assert_eq!(manager.shard_count(), 4);
    println!("Initial shards: {:?}", manager.get_all_shard_names());

    // Add a new shard
    let new_shard_path = temp_dir.path().join("new_shard");
    manager.add_shard("new_shard", new_shard_path.to_str().unwrap())?;
    assert_eq!(manager.shard_count(), 5);
    assert!(manager.shard_exists("new_shard"));
    println!("After adding shard: {:?}", manager.get_all_shard_names());

    // Store data after adding shards
    for i in 0..100 {
        manager.put(&format!("dynamic_key_{}", i), b"test_data", None)?;
    }

    // Verify data can be retrieved
    for i in 0..100 {
        let data = manager.get(&format!("dynamic_key_{}", i))?;
        assert!(data.is_some(), "Key dynamic_key_{} not found", i);
    }

    // Get shard details
    let details = manager.get_shard_details();
    println!("Shard details after additions:");
    for detail in details {
        println!("  {}: {} keys", detail.name, detail.key_count);
    }

    // Remove a shard
    let removed = manager.remove_shard("new_shard")?;
    assert!(removed);
    assert_eq!(manager.shard_count(), 4);
    assert!(!manager.shard_exists("new_shard"));

    println!("Shard 'new_shard' removed successfully");

    Ok(())
}

#[test]
fn test_shard_removal_with_data() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Store data
    for i in 0..50 {
        manager.put(&format!("pre_remove_key_{}", i), b"data", None)?;
    }

    // Get shard names before removal
    let shards_before = manager.get_all_shard_names();
    println!("Shards before removal: {:?}", shards_before);

    // Count accessible keys before removal
    let accessible_before = (0..50)
        .filter(|i| {
            manager
                .get(&format!("pre_remove_key_{}", i))
                .unwrap()
                .is_some()
        })
        .count();
    println!("Accessible keys before removal: {}", accessible_before);

    // Remove the first shard
    if let Some(shard_to_remove) = shards_before.first() {
        let removed = manager.remove_shard(shard_to_remove)?;
        assert!(removed);
        println!("Removed shard: {}", shard_to_remove);
    }

    let shards_after = manager.get_all_shard_names();
    println!("Shards after removal: {:?}", shards_after);
    assert_eq!(shards_after.len(), shards_before.len() - 1);

    println!("Shard removal completed successfully");

    Ok(())
}

#[test]
fn test_dynamic_shard_load_balancing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    // Store 200 records initially
    for i in 0..200 {
        manager.put(&format!("initial_key_{}", i), b"data", None)?;
    }

    let stats_before = manager.get_distribution_stats();
    println!(
        "Distribution before adding shard: {:?}",
        stats_before.shard_distribution
    );

    // Add a new shard
    let new_shard_path = temp_dir.path().join("new_shard");
    manager.add_shard("new_shard", new_shard_path.to_str().unwrap())?;

    // Store 200 more records after adding shard
    for i in 0..200 {
        manager.put(&format!("new_key_{}", i), b"data", None)?;
    }

    let stats_after = manager.get_distribution_stats();
    println!(
        "Distribution after adding shard: {:?}",
        stats_after.shard_distribution
    );

    // The new shard should have some data
    assert!(stats_after.shard_distribution.contains_key("new_shard"));
    let new_shard_count = stats_after
        .shard_distribution
        .get("new_shard")
        .unwrap_or(&0);
    assert!(*new_shard_count > 0, "New shard has no data");

    // Total records should be 400
    assert_eq!(stats_after.total_records, 400);

    Ok(())
}

#[test]
fn test_shard_operations_concurrent() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = Arc::new(DataDistributionManager::new(
        temp_dir.path(),
        DistributionStrategy::RoundRobin,
    )?);

    let mut handles = vec![];

    // Thread 1: Add shards
    let manager1 = manager.clone();
    let handle1 = thread::spawn(move || {
        for i in 0..3 {
            let shard_path = temp_dir.path().join(format!("concurrent_shard_{}", i));
            manager1
                .add_shard(
                    &format!("concurrent_shard_{}", i),
                    shard_path.to_str().unwrap(),
                )
                .unwrap();
            thread::sleep(StdDuration::from_millis(10));
        }
    });
    handles.push(handle1);

    // Thread 2: Write data
    let manager2 = manager.clone();
    let handle2 = thread::spawn(move || {
        for i in 0..100 {
            manager2
                .put(&format!("concurrent_key_{}", i), b"data", None)
                .unwrap();
            thread::sleep(StdDuration::from_millis(5));
        }
    });
    handles.push(handle2);

    // Thread 3: Read data
    let manager3 = manager.clone();
    let handle3 = thread::spawn(move || {
        for i in 0..100 {
            let _ = manager3.get(&format!("concurrent_key_{}", i)).unwrap();
            thread::sleep(StdDuration::from_millis(5));
        }
    });
    handles.push(handle3);

    // Thread 4: Get statistics
    let manager4 = manager.clone();
    let handle4 = thread::spawn(move || {
        for _ in 0..20 {
            let stats = manager4.get_distribution_stats();
            println!(
                "Stats: total={}, shards={}",
                stats.total_records,
                stats.shard_distribution.len()
            );
            thread::sleep(StdDuration::from_millis(50));
        }
    });
    handles.push(handle4);

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify final state
    let final_stats = manager.get_distribution_stats();
    assert!(final_stats.shard_distribution.len() >= 4);
    assert!(final_stats.total_records >= 100);

    Ok(())
}

#[test]
fn test_shard_get_by_key() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let test_keys = vec!["user:123", "user:456", "product:789", "order:111"];
    for key in &test_keys {
        manager.put(key, b"data", None)?;
    }

    for key in &test_keys {
        let shard_name = manager.get_shard_for_key(key)?;
        println!("Key '{}' is in shard '{}'", key, shard_name);
        assert!(manager.shard_exists(&shard_name));
    }

    Ok(())
}

#[test]
fn test_add_remove_shard_multiple() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let temp_dir = TempDir::new()?;
    let manager = DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)?;

    let initial_count = manager.shard_count();

    // Add multiple shards
    for i in 0..3 {
        let shard_path = temp_dir.path().join(format!("multi_shard_{}", i));
        manager.add_shard(&format!("multi_shard_{}", i), shard_path.to_str().unwrap())?;
    }

    assert_eq!(manager.shard_count(), initial_count + 3);

    // Remove multiple shards
    for i in 0..2 {
        let removed = manager.remove_shard(&format!("multi_shard_{}", i))?;
        assert!(removed);
    }

    assert_eq!(manager.shard_count(), initial_count + 1);

    Ok(())
}
