//! Comprehensive test suite for VirtualFilesystem (updated for strict mkdir behavior)
//!
//! This suite validates:
//! - Strict directory rules (no implicit parents for mkdir, error on exists).
//! - Automatic parent creation for file/document helpers.
//! - Data persistence across Manager restarts.
//! - Complex metadata extraction (word counts, fingerprints, code analysis).

use bund_blobstore::common::{VfsNodeType, VirtualFilesystem};
use bund_blobstore::{DataDistributionManager, DistributionStrategy};
use std::sync::Arc;
use tempfile::tempdir;

/// Helper function to create a clean VFS instance for testing
fn setup_vfs() -> (VirtualFilesystem, tempfile::TempDir) {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let manager = Arc::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)
            .expect("Failed to create manager"),
    );
    let vfs = VirtualFilesystem::new(manager);
    (vfs, temp_dir)
}

// ============================================
// BASIC DIRECTORY OPERATION TESTS (STRICT mkdir)
// ============================================

#[test]
fn test_mkdir_strict_rules() {
    let (vfs, _temp) = setup_vfs();

    // 1. Successful creation
    vfs.mkdir("/test").expect("Should create root-level dir");
    assert!(vfs.resolve_path("/test").unwrap().is_some());

    // 2. Fail on existing
    let res = vfs.mkdir("/test");
    assert!(res.is_err(), "Should error when directory exists");

    // 3. Fail on missing parent (Strict behavior)
    let res = vfs.mkdir("/a/b/c");
    assert!(res.is_err(), "Strict mkdir should not auto-create /a/b");
}

#[test]
fn test_mkdir_invalid_paths() {
    let (vfs, _temp) = setup_vfs();
    let cases = vec!["", "relative", "no/slash", "/trailing/"];
    for path in cases {
        assert!(
            vfs.mkdir(path).is_err(),
            "Path '{}' should be rejected",
            path
        );
    }
}

#[test]
fn test_mkdir_parent_is_file_protection() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkfile("/file.txt", b"not a dir", "text/plain").unwrap();

    let res = vfs.mkdir("/file.txt/subdir");
    assert!(
        res.is_err(),
        "Should not allow creating a subdirectory inside a file"
    );
}

#[test]
fn test_rmdir_recursive_safety() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/save.dat", b"important", "bin").unwrap();

    // Should fail because it contains save.dat
    let res = vfs.rm("/data");
    assert!(res.is_err(), "Should not delete non-empty directory");
}

// ============================================
// DOCUMENT TYPE & METADATA TESTS
// ============================================

#[test]
fn test_mktext_metadata_analysis() {
    let (vfs, _temp) = setup_vfs();
    let content = "The quick brown fox jumps over the lazy dog."; // 9 words
    vfs.mktext("/docs/story.txt", content, "english").unwrap();

    let node = vfs
        .get_node(&vfs.resolve_path("/docs/story.txt").unwrap().unwrap())
        .unwrap();
    if let VfsNodeType::TextDocument {
        word_count,
        language,
        ..
    } = node.node_type
    {
        assert_eq!(word_count, 9);
        assert_eq!(language, "english");
    } else {
        panic!("Incorrect node type stored");
    }
}

#[test]
fn test_mkjson_fingerprinting() {
    let (vfs, _temp) = setup_vfs();
    let json_data = br#"{"id": 101, "status": "active"}"#;
    let json_str = std::str::from_utf8(json_data).unwrap();
    vfs.mkjson(
        "/api/v1.json",
        &uuid::Uuid::new_v4().to_string(), // doc_id
        "test-fingerprint",                // fingerprint
        "1.0",                             // schema_version
        json_str.len(),                    // size_bytes
    )
    .unwrap();

    let node = vfs
        .get_node(&vfs.resolve_path("/api/v1.json").unwrap().unwrap())
        .unwrap();
    if let VfsNodeType::JsonDocument { fingerprint, .. } = node.node_type {
        assert!(
            !fingerprint.is_empty(),
            "JSON should have a content fingerprint"
        );
    } else {
        panic!("Expected JsonDocument");
    }
}

#[test]
fn test_mkcode_analysis() {
    let (vfs, _temp) = setup_vfs();
    let code = "fn init() {}\nfn start() {}\nfn stop() {}";
    vfs.script("/src/main.rs", code, "rust").unwrap();

    let node = vfs
        .get_node(&vfs.resolve_path("/src/main.rs").unwrap().unwrap())
        .unwrap();
    if let VfsNodeType::CodeDocument {
        lines, functions, ..
    } = node.node_type
    {
        assert_eq!(lines, 3);
        assert!(functions.contains(&"init".to_string()));
        assert!(functions.contains(&"stop".to_string()));
    } else {
        panic!("Expected CodeDocument");
    }
}

// ============================================
// CONCURRENCY & PERSISTENCE
// ============================================
#[test]
fn test_concurrent_load() {
    let (vfs, _temp) = setup_vfs();
    let vfs_arc = Arc::new(vfs);
    let mut handles = vec![];
    let shared_path = "/concurrency_test";
    for i in 0..20 {
        let vfs: Arc<VirtualFilesystem> = Arc::clone(&vfs_arc);
        handles.push(std::thread::spawn(move || {
            let path = format!("{}/file_{}.txt", &shared_path, i);
            vfs.mkfile(&path, b"data", "text/plain").unwrap();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    let parent_node = vfs_arc
        .get_node_by_path(shared_path)
        .expect("Folder should exist");
    println!("Final child count: {}", parent_node.children.len());
    assert_eq!(parent_node.children.len(), 20);
    let files = vfs_arc.as_ref().ls(shared_path).unwrap();
    assert_eq!(files.len(), 20);
}

#[test]
fn test_persistence_lifecycle() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();

    // Phase 1: Write data and shutdown
    {
        let manager =
            Arc::new(DataDistributionManager::new(path, DistributionStrategy::RoundRobin).unwrap());
        let vfs = VirtualFilesystem::new(manager);
        vfs.mkdir("/persist").unwrap();
        vfs.mkfile("/persist/test.bin", &[1, 2, 3, 4], "bin")
            .unwrap();
    }

    // Phase 2: Reopen and verify
    {
        let manager =
            Arc::new(DataDistributionManager::new(path, DistributionStrategy::RoundRobin).unwrap());
        let vfs = VirtualFilesystem::new(manager);

        let id = vfs
            .resolve_path("/persist/test.bin")
            .unwrap()
            .expect("File should persist");
        let node = vfs.get_node(&id).unwrap();

        if let VfsNodeType::BlobReference { size, .. } = node.node_type {
            assert_eq!(size, 4);
        } else {
            panic!("Node type corrupted after restart");
        }
    }
}

// ============================================
// SYMLINK & RESOLUTION
// ============================================

#[test]
fn test_symlink_integrity() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkfile("/original.txt", b"hello", "text/plain").unwrap();
    let original_id = vfs.resolve_path("/original.txt").unwrap().unwrap();

    vfs.mklink("/shortcut.txt", &original_id).unwrap();

    let link_id = vfs.resolve_path("/shortcut.txt").unwrap().unwrap();
    let link_node = vfs.get_node(&link_id).unwrap();

    assert_eq!(link_node.target_id, Some(original_id));
}

#[test]
fn test_root_invariants() {
    let (vfs, _temp) = setup_vfs();

    // Resolve root
    let root_id = vfs.resolve_path("/").unwrap();
    assert_eq!(root_id.unwrap(), "root");

    // Cannot delete root
    let res = vfs.rm("/");
    assert!(res.is_err(), "System should protect the root node");
}
