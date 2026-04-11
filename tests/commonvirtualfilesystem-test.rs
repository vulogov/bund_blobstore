use bund_blobstore::common::{VfsNodeType, VirtualFilesystem};
use bund_blobstore::{DataDistributionManager, DistributionStrategy};
use std::sync::Arc;
use tempfile::tempdir;

fn setup_vfs() -> (VirtualFilesystem, tempfile::TempDir) {
    let temp_dir = tempdir().expect("Failed to create temp directory");
    let manager = Arc::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)
            .expect("Failed to create manager"),
    );
    let vfs = VirtualFilesystem::new(manager);
    (vfs, temp_dir)
}

// --- 1. Basic Operations ---
#[test]
fn test_mkdir_strict_rules() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkdir("/test").expect("Should create root-level dir");
    assert!(vfs.get_node_by_path("/test").is_ok());
    assert!(vfs.mkdir("/test").is_err());
    assert!(vfs.mkdir("/a/b/c").is_err());
}

#[test]
fn test_mkdir_invalid_paths() {
    let (vfs, _temp) = setup_vfs();
    for path in vec!["", "relative", "no/slash", "/trailing/"] {
        assert!(vfs.mkdir(path).is_err(), "Path '{}' should fail", path);
    }
}

#[test]
fn test_mkdir_parent_is_file_protection() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkfile("/file.txt", b"data", "text/plain").unwrap();
    assert!(vfs.mkdir("/file.txt/subdir").is_err());
}

#[test]
fn test_rmdir_recursive_safety() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/f.dat", b"val", "bin").unwrap();
    assert!(vfs.rm("/data").is_err(), "Should not delete non-empty dir");
}

// --- 2. Metadata & Document Types ---
#[test]
fn test_mktext_metadata_analysis() {
    let (vfs, _temp) = setup_vfs();
    vfs.mktext("/story.txt", "One two three", "en").unwrap();
    let node = vfs.get_node_by_path("/story.txt").unwrap();
    if let VfsNodeType::TextDocument { word_count, .. } = node.node_type {
        assert_eq!(word_count, 3);
    } else {
        panic!("Expected TextDocument");
    }
}

#[test]
fn test_mkjson_fingerprinting() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkjson("/a.json", "id1", "fp1", "1.0", 10).unwrap();
    let node = vfs.get_node_by_path("/a.json").unwrap();
    assert!(matches!(node.node_type, VfsNodeType::JsonDocument { .. }));
}

#[test]
fn test_mkcode_analysis() {
    let (vfs, _temp) = setup_vfs();
    vfs.script("/main.rs", "fn a() {}\nfn b() {}", "rust")
        .unwrap();
    let node = vfs.get_node_by_path("/main.rs").unwrap();
    if let VfsNodeType::CodeDocument { lines, .. } = node.node_type {
        assert_eq!(lines, 2);
    } else {
        panic!("Expected CodeDocument");
    }
}

// --- 3. Advanced Features ---
#[test]
fn test_symlink_integrity() {
    let (vfs, _temp) = setup_vfs();
    vfs.mkfile("/a", b"1", "t").unwrap();
    let id = vfs.get_node_by_path("/a").unwrap().id;
    vfs.mklink("/link", &id).unwrap();
    assert_eq!(vfs.get_node_by_path("/link").unwrap().target_id, Some(id));
}

#[test]
fn test_root_invariants() {
    let (vfs, _temp) = setup_vfs();
    assert_eq!(vfs.get_node_by_path("/").unwrap().id, "root");
    assert!(vfs.rm("/").is_err());
}

// --- 4. Concurrency & Persistence ---
#[test]
fn test_concurrent_load() {
    let (vfs, _temp) = setup_vfs();
    let vfs_arc = Arc::new(vfs);
    vfs_arc.mkdir("/shared").unwrap();
    let mut handles = vec![];

    for i in 0..20 {
        let v_clone = Arc::clone(&vfs_arc);
        handles.push(std::thread::spawn(move || {
            let p = format!("/shared/f{}.txt", i);
            v_clone.mkfile(&p, b"d", "t").unwrap();
        }));
    }
    for h in handles {
        h.join().unwrap();
    }
    assert_eq!(
        vfs_arc.get_node_by_path("/shared").unwrap().children.len(),
        20
    );
}

#[test]
fn test_persistence_lifecycle() {
    let temp_dir = tempdir().unwrap();
    let p = temp_dir.path();
    {
        let m =
            Arc::new(DataDistributionManager::new(p, DistributionStrategy::RoundRobin).unwrap());
        VirtualFilesystem::new(m).mkdir("/p").unwrap();
    }
    let m2 = Arc::new(DataDistributionManager::new(p, DistributionStrategy::RoundRobin).unwrap());
    let vfs2 = VirtualFilesystem::new(m2);
    assert!(vfs2.get_node_by_path("/p").is_ok());
}
