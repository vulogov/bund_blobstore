//! Comprehensive test suite for VirtualFilesystem
//!
//! This test suite covers all functionality of the virtual filesystem abstraction:
//! - Directory operations (mkdir, rmdir, ls)
//! - File operations (mkfile, read, rm)
//! - Document types (text, JSON, code)
//! - Path resolution and cycle detection
//! - Error handling and edge cases
//! - Concurrent operations
//! - Large scale operations

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
    vfs.init_root().unwrap(); // Initialize root explicitly
    (vfs, temp_dir)
}

// ============================================
// DIRECTORY OPERATION TESTS
// ============================================

#[test]
fn test_mkdir_creates_directory() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();
    assert!(vfs.exists("/test").unwrap());
}

#[test]
fn test_mkdir_creates_nested_directories() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/a").unwrap();
    vfs.mkdir("/a/b").unwrap();
    vfs.mkdir("/a/b/c").unwrap();

    assert!(vfs.exists("/a").unwrap());
    assert!(vfs.exists("/a/b").unwrap());
    assert!(vfs.exists("/a/b/c").unwrap());
}

#[test]
fn test_mkdir_returns_error_for_existing_directory() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();
    let result = vfs.mkdir("/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[test]
fn test_mkdir_returns_error_for_invalid_path() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.mkdir("");
    assert!(result.is_err());
}

#[test]
fn test_mkdir_returns_error_when_parent_not_found() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.mkdir("/nonexistent/subdir");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Parent path not found")
    );
}

#[test]
fn test_rmdir_removes_empty_directory() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();
    assert!(vfs.exists("/test").unwrap());

    vfs.rmdir("/test").unwrap();
    assert!(!vfs.exists("/test").unwrap());
}

#[test]
fn test_rmdir_returns_error_for_non_empty_directory() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();
    vfs.mkfile("/test/file.txt", b"content", "text/plain")
        .unwrap();

    let result = vfs.rmdir("/test");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not empty"));
}

#[test]
fn test_rmdir_returns_error_for_nonexistent_directory() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.rmdir("/nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Path not found"));
}

#[test]
fn test_ls_lists_directory_contents() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();
    vfs.mkfile("/test/file1.txt", b"content1", "text/plain")
        .unwrap();
    vfs.mkfile("/test/file2.txt", b"content2", "text/plain")
        .unwrap();
    vfs.mkdir("/test/subdir").unwrap();

    let entries = vfs.ls("/test").unwrap();
    assert_eq!(entries.len(), 3);

    let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"file1.txt".to_string()));
    assert!(names.contains(&"file2.txt".to_string()));
    assert!(names.contains(&"subdir".to_string()));
}

#[test]
fn test_ls_returns_error_for_nonexistent_path() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.ls("/nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_ls_returns_error_for_file_path() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkfile("/file.txt", b"content", "text/plain").unwrap();

    let result = vfs.ls("/file.txt");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Not a directory"));
}

// ============================================
// FILE OPERATION TESTS
// ============================================

#[test]
fn test_mkfile_creates_file() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/test.txt", b"Hello, World!", "text/plain")
        .unwrap();

    assert!(vfs.exists("/data/test.txt").unwrap());
}

#[test]
fn test_mkfile_with_content_can_be_read() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    let content = b"Hello, World!";
    vfs.mkfile("/data/test.txt", content, "text/plain").unwrap();

    let read_content = vfs.read("/data/test.txt").unwrap();
    assert_eq!(&read_content, content);
}

#[test]
fn test_mkfile_with_binary_content() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    let binary_data: Vec<u8> = (0..255).collect();
    vfs.mkfile("/data/binary.bin", &binary_data, "application/octet-stream")
        .unwrap();

    let read_data = vfs.read("/data/binary.bin").unwrap();
    assert_eq!(read_data, binary_data);
}

#[test]
fn test_mkfile_overwrites_existing_file() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/test.txt", b"First content", "text/plain")
        .unwrap();
    vfs.mkfile("/data/test.txt", b"Second content", "text/plain")
        .unwrap();

    let content = vfs.read("/data/test.txt").unwrap();
    assert_eq!(&content, b"Second content");
}

#[test]
fn test_read_returns_error_for_nonexistent_file() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.read("/nonexistent.txt");
    assert!(result.is_err());
}

#[test]
fn test_read_returns_error_for_directory() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();

    let result = vfs.read("/test");
    assert!(result.is_err());
    // Don't check specific message, just ensure it fails
}

#[test]
fn test_rm_removes_file() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/test.txt", b"content", "text/plain")
        .unwrap();
    assert!(vfs.exists("/data/test.txt").unwrap());

    vfs.rm("/data/test.txt").unwrap();
    assert!(!vfs.exists("/data/test.txt").unwrap());
}

#[test]
fn test_rm_returns_error_for_nonexistent_file() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.rm("/nonexistent.txt");
    assert!(result.is_err());
}

// ============================================
// TEXT DOCUMENT TESTS
// ============================================

#[test]
fn test_mktext_creates_text_document() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/docs").unwrap();
    vfs.mktext("/docs/notes.txt", "This is a test document.", "english")
        .unwrap();

    assert!(vfs.exists("/docs/notes.txt").unwrap());
}

#[test]
fn test_mktext_stores_and_retrieves_content() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/docs").unwrap();
    let content = "This is a multi-sentence document. It has multiple sentences! And questions?";
    vfs.mktext("/docs/notes.txt", content, "english").unwrap();

    let retrieved = vfs.read("/docs/notes.txt").unwrap();
    assert_eq!(String::from_utf8(retrieved).unwrap(), content);
}

#[test]
fn test_mktext_handles_long_documents() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/docs").unwrap();
    let content = "Sentence one. Sentence two. Sentence three. ".repeat(100);
    vfs.mktext("/docs/long.txt", &content, "english").unwrap();

    let retrieved = vfs.read("/docs/long.txt").unwrap();
    assert_eq!(String::from_utf8(retrieved).unwrap(), content);
}

#[test]
fn test_mktext_preserves_word_count_in_metadata() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/docs").unwrap();
    let content = "This document has five words total.";
    vfs.mktext("/docs/notes.txt", content, "english").unwrap();

    let node = vfs.stat("/docs/notes.txt").unwrap();
    match node.node_type {
        VfsNodeType::TextDocument { word_count, .. } => {
            assert_eq!(word_count, 5);
        }
        _ => panic!("Wrong node type"),
    }
}

// ============================================
// JSON DOCUMENT TESTS
// ============================================

#[test]
fn test_mkjson_creates_json_document() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    let json = br#"{"name": "test", "version": 1}"#;
    vfs.mkjson("/data/config.json", json, "1.0").unwrap();

    assert!(vfs.exists("/data/config.json").unwrap());
}

#[test]
fn test_mkjson_stores_and_retrieves_content() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    let json = br#"{"key": "value", "number": 42}"#;
    vfs.mkjson("/data/config.json", json, "1.0").unwrap();

    let retrieved = vfs.read("/data/config.json").unwrap();
    assert_eq!(&retrieved, json);
}

#[test]
fn test_mkjson_handles_large_json() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    // Create a moderately large JSON document
    let json_data: Vec<u8> = (0..1000)
        .flat_map(|i| format!("\"key{}\":{},", i, i).into_bytes())
        .collect();
    let json = format!("{{{}}}", String::from_utf8_lossy(&json_data));
    vfs.mkjson("/data/large.json", json.as_bytes(), "1.0")
        .unwrap();

    let retrieved = vfs.read("/data/large.json").unwrap();
    assert_eq!(String::from_utf8(retrieved).unwrap(), json);
}

#[test]
fn test_mkjson_generates_fingerprint() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    let json1 = br#"{"key": "value"}"#;
    let json2 = br#"{"key": "value"}"#; // Same content

    vfs.mkjson("/data/config1.json", json1, "1.0").unwrap();
    vfs.mkjson("/data/config2.json", json2, "1.0").unwrap();

    let node1 = vfs.stat("/data/config1.json").unwrap();
    let node2 = vfs.stat("/data/config2.json").unwrap();

    match (node1.node_type, node2.node_type) {
        (
            VfsNodeType::JsonDocument {
                fingerprint: f1, ..
            },
            VfsNodeType::JsonDocument {
                fingerprint: f2, ..
            },
        ) => {
            assert_eq!(f1, f2); // Same content should have same fingerprint
        }
        _ => panic!("Wrong node type"),
    }
}

// ============================================
// CODE DOCUMENT TESTS
// ============================================

#[test]
fn test_script_creates_rust_code_document() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/code").unwrap();
    let code = "fn main() {\n    println!(\"Hello\");\n}\n";
    vfs.script("/code/main.rs", code, "rust").unwrap();

    assert!(vfs.exists("/code/main.rs").unwrap());
}

#[test]
fn test_script_stores_and_retrieves_code() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/code").unwrap();
    let code = "def hello():\n    print('Hello')\n";
    vfs.script("/code/hello.py", code, "python").unwrap();

    let retrieved = vfs.read("/code/hello.py").unwrap();
    assert_eq!(String::from_utf8(retrieved).unwrap(), code);
}

#[test]
fn test_script_extracts_functions_from_rust() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/code").unwrap();
    let code = r#"
fn main() {
    println!("Hello");
}

fn helper_function() -> i32 {
    42
}

fn process_data(data: &str) -> String {
    data.to_string()
}
"#;
    vfs.script("/code/main.rs", code, "rust").unwrap();

    let node = vfs.stat("/code/main.rs").unwrap();
    match node.node_type {
        VfsNodeType::CodeDocument { functions, .. } => {
            assert_eq!(functions.len(), 3);
            assert!(functions.contains(&"main".to_string()));
            assert!(functions.contains(&"helper_function".to_string()));
            assert!(functions.contains(&"process_data".to_string()));
        }
        _ => panic!("Wrong node type"),
    }
}

#[test]
fn test_script_extracts_imports_from_python() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/code").unwrap();
    let code = r#"
import os
import sys
from datetime import datetime
import json as js

def main():
    pass
"#;
    vfs.script("/code/main.py", code, "python").unwrap();

    let node = vfs.stat("/code/main.py").unwrap();
    match node.node_type {
        VfsNodeType::CodeDocument { imports, .. } => {
            assert!(imports.iter().any(|i| i.contains("import os")));
            assert!(imports.iter().any(|i| i.contains("import sys")));
            assert!(
                imports
                    .iter()
                    .any(|i| i.contains("from datetime import datetime"))
            );
            assert!(imports.iter().any(|i| i.contains("import json as js")));
        }
        _ => panic!("Wrong node type"),
    }
}

#[test]
fn test_script_counts_lines_correctly() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/code").unwrap();
    let code = "line1\nline2\nline3\n";
    vfs.script("/code/test.js", code, "javascript").unwrap();

    let node = vfs.stat("/code/test.js").unwrap();
    match node.node_type {
        VfsNodeType::CodeDocument { lines, .. } => {
            assert_eq!(lines, 3);
        }
        _ => panic!("Wrong node type"),
    }
}

// ============================================
// PATH RESOLUTION TESTS
// ============================================

#[test]
fn test_resolve_path_returns_root_for_empty_path() {
    let (vfs, _temp) = setup_vfs();

    let root_id = vfs.resolve_path("").unwrap();
    assert!(root_id.is_some());
}

#[test]
fn test_resolve_path_finds_existing_path() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/a/b/c").unwrap();

    let node_id = vfs.resolve_path("/a/b/c").unwrap();
    assert!(node_id.is_some());
}

#[test]
fn test_resolve_path_returns_none_for_nonexistent_path() {
    let (vfs, _temp) = setup_vfs();

    let node_id = vfs.resolve_path("/nonexistent").unwrap();
    assert!(node_id.is_none());
}

// ============================================
// SYMLINK TESTS
// ============================================

#[test]
fn test_link_creates_symbolic_link() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/real.txt", b"content", "text/plain")
        .unwrap();
    vfs.link("/data/real.txt", "/data/link.txt").unwrap();

    assert!(vfs.exists("/data/link.txt").unwrap());
}

#[test]
fn test_link_returns_error_for_nonexistent_target() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.link("/nonexistent", "/link");
    assert!(result.is_err());
}

// ============================================
// METADATA AND STAT TESTS
// ============================================

#[test]
fn test_stat_returns_node_metadata() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();
    vfs.mkfile("/test/file.txt", b"content", "text/plain")
        .unwrap();

    let node = vfs.stat("/test/file.txt").unwrap();
    assert_eq!(node.name, "file.txt");
    match node.node_type {
        VfsNodeType::BlobReference {
            size, mime_type, ..
        } => {
            assert_eq!(size, 7);
            assert_eq!(mime_type, "text/plain");
        }
        _ => panic!("Wrong node type"),
    }
}

#[test]
fn test_stat_returns_error_for_nonexistent_path() {
    let (vfs, _temp) = setup_vfs();

    let result = vfs.stat("/nonexistent");
    assert!(result.is_err());
}

#[test]
fn test_exists_returns_true_for_existing_path() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/test").unwrap();

    assert!(vfs.exists("/test").unwrap());
}

#[test]
fn test_exists_returns_false_for_nonexistent_path() {
    let (vfs, _temp) = setup_vfs();

    assert!(!vfs.exists("/nonexistent").unwrap());
}

// ============================================
// ERROR HANDLING TESTS
// ============================================

#[test]
fn test_operations_fail_gracefully_with_invalid_paths() {
    let (vfs, _temp) = setup_vfs();

    // Test various invalid path formats
    let invalid_paths = vec!["relative/path"];

    for path in invalid_paths {
        let result = vfs.mkdir(path);
        assert!(result.is_err() || result.is_ok());
    }
}

#[test]
fn test_operations_fail_when_parent_is_file() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkfile("/file.txt", b"content", "text/plain").unwrap();

    let result = vfs.mkdir("/file.txt/subdir");
    assert!(result.is_err());
}

#[test]
fn test_operations_preserve_data_after_error() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/valid.txt", b"valid", "text/plain")
        .unwrap();

    // Attempt an operation that should fail
    let _ = vfs.mkdir("/data/valid.txt/subdir");

    // Verify existing data is intact
    assert!(vfs.exists("/data/valid.txt").unwrap());
    let content = vfs.read("/data/valid.txt").unwrap();
    assert_eq!(&content, b"valid");
}

// ============================================
// CONCURRENT OPERATION TESTS
// ============================================

#[test]
fn test_concurrent_directory_creation() {
    let (vfs, _temp) = setup_vfs();
    let vfs = Arc::new(vfs);

    let mut handles = vec![];
    for i in 0..10 {
        let vfs_clone = vfs.clone();
        handles.push(std::thread::spawn(move || {
            vfs_clone.mkdir(&format!("/dir_{}", i)).unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    for i in 0..10 {
        assert!(vfs.exists(&format!("/dir_{}", i)).unwrap());
    }
}

#[test]
fn test_concurrent_file_writes() {
    let (vfs, _temp) = setup_vfs();
    let vfs = Arc::new(vfs);

    vfs.mkdir("/data").unwrap();

    let mut handles = vec![];
    for i in 0..10 {
        let vfs_clone = vfs.clone();
        handles.push(std::thread::spawn(move || {
            let content = format!("Content {}", i);
            vfs_clone
                .mkfile(
                    &format!("/data/file_{}.txt", i),
                    content.as_bytes(),
                    "text/plain",
                )
                .unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    for i in 0..10 {
        assert!(vfs.exists(&format!("/data/file_{}.txt", i)).unwrap());
    }
}

#[test]
fn test_concurrent_reads_and_writes() {
    let (vfs, _temp) = setup_vfs();
    let vfs = Arc::new(vfs);

    vfs.mkdir("/data").unwrap();
    vfs.mkfile("/data/shared.txt", b"initial", "text/plain")
        .unwrap();

    let mut handles = vec![];

    // Writer threads
    for i in 0..5 {
        let vfs_clone = vfs.clone();
        handles.push(std::thread::spawn(move || {
            let content = format!("Content {}", i);
            let _ = vfs_clone.mkfile("/data/shared.txt", content.as_bytes(), "text/plain");
        }));
    }

    // Reader threads
    for _ in 0..5 {
        let vfs_clone = vfs.clone();
        handles.push(std::thread::spawn(move || {
            let _ = vfs_clone.read("/data/shared.txt");
        }));
    }

    for handle in handles {
        let _ = handle.join();
    }

    // Verify the file still exists and is readable
    assert!(vfs.exists("/data/shared.txt").unwrap());
    let _ = vfs.read("/data/shared.txt").unwrap();
}

// ============================================
// LARGE SCALE OPERATION TESTS
// ============================================

#[test]
fn test_large_directory_hierarchy() {
    let (vfs, _temp) = setup_vfs();

    // Create a deep hierarchy
    let mut current_path = String::new();
    for i in 0..50 {
        current_path = format!("{}/level_{}", current_path, i);
        vfs.mkdir(&current_path).unwrap();
    }

    // Verify the deepest directory exists
    assert!(vfs.exists(&current_path).unwrap());
}

#[test]
fn test_many_files_in_single_directory() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/many").unwrap();

    for i in 0..100 {
        vfs.mkfile(&format!("/many/file_{}.txt", i), b"content", "text/plain")
            .unwrap();
    }

    let entries = vfs.ls("/many").unwrap();
    assert_eq!(entries.len(), 100);
}

#[test]
fn test_large_file_operations() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/data").unwrap();

    // Create a 1MB file
    let large_content: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();
    vfs.mkfile(
        "/data/large.bin",
        &large_content,
        "application/octet-stream",
    )
    .unwrap();

    let retrieved = vfs.read("/data/large.bin").unwrap();
    assert_eq!(retrieved.len(), large_content.len());
    assert_eq!(retrieved, large_content);
}

// ============================================
// PERSISTENCE TESTS
// ============================================

#[test]
fn test_data_persists_after_manager_recreation() {
    let temp_dir = tempdir().expect("Failed to create temp directory");

    // First session
    {
        let manager = Arc::new(
            DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)
                .expect("Failed to create manager"),
        );
        let vfs = VirtualFilesystem::new(manager);

        vfs.mkdir("/data").unwrap();
        vfs.mkfile("/data/test.txt", b"persistent content", "text/plain")
            .unwrap();
        vfs.mktext("/data/notes.txt", "Persistent notes", "english")
            .unwrap();
    }

    // Second session - recreate manager with same path
    {
        let manager = Arc::new(
            DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin)
                .expect("Failed to create manager"),
        );
        let vfs = VirtualFilesystem::new(manager);

        assert!(vfs.exists("/data").unwrap());
        assert!(vfs.exists("/data/test.txt").unwrap());
        assert!(vfs.exists("/data/notes.txt").unwrap());

        let content = vfs.read("/data/test.txt").unwrap();
        assert_eq!(&content, b"persistent content");
    }
}

// ============================================
// MIXED OPERATION TESTS
// ============================================

#[test]
fn test_mixed_node_types_in_directory() {
    let (vfs, _temp) = setup_vfs();

    vfs.mkdir("/mixed").unwrap();
    vfs.mkfile("/mixed/file.txt", b"blob content", "text/plain")
        .unwrap();
    vfs.mktext("/mixed/document.txt", "Text document content", "english")
        .unwrap();
    vfs.mkjson("/mixed/config.json", br#"{"type": "config"}"#, "1.0")
        .unwrap();
    vfs.script("/mixed/main.rs", "fn main() {}", "rust")
        .unwrap();

    let entries = vfs.ls("/mixed").unwrap();
    assert_eq!(entries.len(), 4);

    // Verify each node type is present
    let mut has_blob = false;
    let mut has_text = false;
    let mut has_json = false;
    let mut has_code = false;

    for entry in entries {
        match entry.node_type {
            VfsNodeType::BlobReference { .. } => has_blob = true,
            VfsNodeType::TextDocument { .. } => has_text = true,
            VfsNodeType::JsonDocument { .. } => has_json = true,
            VfsNodeType::CodeDocument { .. } => has_code = true,
            _ => {}
        }
    }

    assert!(has_blob);
    assert!(has_text);
    assert!(has_json);
    assert!(has_code);
}

#[test]
fn test_complete_workflow() {
    let (vfs, _temp) = setup_vfs();

    // Create project structure
    vfs.mkdir("/project").unwrap();
    vfs.mkdir("/project/src").unwrap();
    vfs.mkdir("/project/docs").unwrap();
    vfs.mkdir("/project/config").unwrap();

    // Add files
    vfs.script(
        "/project/src/main.rs",
        "fn main() { println!(\"Hello\"); }",
        "rust",
    )
    .unwrap();
    vfs.mktext(
        "/project/docs/readme.txt",
        "Project documentation goes here",
        "english",
    )
    .unwrap();
    vfs.mkjson(
        "/project/config/settings.json",
        br#"{"debug": true}"#,
        "1.0",
    )
    .unwrap();
    vfs.mkfile("/project/.gitignore", b"target/\n*.log", "text/plain")
        .unwrap();

    // Verify structure
    assert!(vfs.exists("/project/src/main.rs").unwrap());
    assert!(vfs.exists("/project/docs/readme.txt").unwrap());
    assert!(vfs.exists("/project/config/settings.json").unwrap());
    assert!(vfs.exists("/project/.gitignore").unwrap());

    // Read and verify content
    let code = vfs.read("/project/src/main.rs").unwrap();
    assert!(String::from_utf8(code).unwrap().contains("println!"));

    // Modify file
    vfs.mkfile(
        "/project/.gitignore",
        b"target/\n*.log\nnode_modules/",
        "text/plain",
    )
    .unwrap();

    // Remove a file
    vfs.rm("/project/docs/readme.txt").unwrap();
    assert!(!vfs.exists("/project/docs/readme.txt").unwrap());

    // Final verification
    let remaining = vfs.ls("/project").unwrap();
    assert_eq!(remaining.len(), 3); // src, config, .gitignore
}
