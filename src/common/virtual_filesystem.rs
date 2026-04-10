use crate::DataDistributionManager;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

pub type VfsResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum VfsNodeType {
    VirtualFolder,
    BlobReference {
        blob_key: String,
        size: u64,
        mime_type: String,
    },
    TextDocument {
        document_id: String,
        chunk_count: usize,
        word_count: usize,
        language: String,
    },
    JsonDocument {
        document_id: String,
        fingerprint: String,
        schema_version: String,
        size_bytes: usize,
    },
    CodeDocument {
        blob_key: String,
        language: String,
        lines: usize,
        functions: Vec<String>,
        imports: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VfsNode {
    pub id: String,
    pub name: String,
    pub node_type: VfsNodeType,
    pub parent_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
    pub children: HashMap<String, String>,
    pub target_id: Option<String>,
}

impl VfsNode {
    pub fn new(
        id: String,
        name: String,
        node_type: VfsNodeType,
        parent_id: Option<String>,
    ) -> Self {
        Self {
            id,
            name,
            node_type,
            parent_id,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: None,
        }
    }
    fn generate_id(prefix: &str, name: &str) -> String {
        format!(
            "{}_{}_{}",
            prefix,
            name,
            Utc::now().timestamp_nanos_opt().unwrap_or(0)
        )
    }

    pub fn new_folder(name: &str, parent_id: Option<String>) -> Self {
        Self {
            id: if name == "/" {
                "root".to_string()
            } else {
                Self::generate_id("folder", name)
            },
            name: name.to_string(),
            node_type: VfsNodeType::VirtualFolder,
            parent_id,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: None,
        }
    }
}

pub struct VirtualFilesystem {
    manager: Arc<DataDistributionManager>,
}

impl VirtualFilesystem {
    pub fn new(manager: Arc<DataDistributionManager>) -> Self {
        let vfs = Self { manager };

        // Ensure root exists immediately
        if let Ok(None) = vfs.manager.get(&Self::get_node_key("root")) {
            let root = VfsNode::new_folder("/", None);
            // We use a direct save here to avoid chicken-and-egg logic loops
            if let Ok(data) = serde_json::to_vec(&root) {
                let _ = vfs.manager.put(&Self::get_node_key("root"), &data, None);
            }
        }

        vfs
    }

    fn get_node_key(id: &str) -> String {
        format!("vfs/node/{}", id)
    }
    fn save_node_internal(&self, node: &VfsNode) -> VfsResult<()> {
        let key = Self::get_node_key(&node.id);
        let data = serde_json::to_vec(node).map_err(|e| e.to_string())?;
        self.manager.put(&key, &data, None)?;
        let shard_name = self
            .manager
            .get_target_shard(&key, None)
            .map_err(|e| e.to_string())?;

        // 3. Sync that specific shard
        self.manager
            .sync_shard(&shard_name)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn save_node(&self, node: &VfsNode) -> VfsResult<()> {
        let _lock = self.manager.global_lock.lock().map_err(|_| "Poisoned")?;
        self.save_node_internal(node)
    }

    pub fn get_node(&self, id: &str) -> VfsResult<VfsNode> {
        let key = Self::get_node_key(id);

        // Ensure we resolve the shard name fresh every time
        let _shard_name = self
            .manager
            .get_target_shard(&key, None)
            .map_err(|e| e.to_string())?;

        // Pull the store fresh from the manager
        let data = self
            .manager
            .get(&key)?
            .ok_or_else(|| format!("Node not found: {}", id))?;

        let node: VfsNode = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
        Ok(node)
    }
    pub fn get_node_by_path(&self, path: &str) -> VfsResult<VfsNode> {
        let clean = path.trim().trim_matches('/');
        if clean.is_empty() {
            return self.get_node("root");
        }

        let mut current_node = self.get_node("root")?;
        for part in clean.split('/') {
            let next_id = current_node
                .children
                .get(part)
                .ok_or_else(|| format!("Path not found: {}", path))?;
            current_node = self.get_node(next_id)?;
        }
        Ok(current_node)
    }
    fn add_child_internal(&self, parent_id: &str, name: &str, child_id: &str) -> VfsResult<()> {
        // Note: This assumes the caller ALREADY holds the lock
        let mut parent = self.get_node(parent_id)?;
        parent
            .children
            .insert(name.to_string(), child_id.to_string());
        parent.modified_at = chrono::Utc::now();
        self.save_node_internal(&parent)?;
        self.manager
            .sync_shard("shard_0")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn add_child(&self, parent_id: &str, name: &str, child_id: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "Failed to acquire lock")?;
        let mut parent = self.get_node(parent_id)?;
        parent
            .children
            .insert(name.to_string(), child_id.to_string());
        parent.modified_at = chrono::Utc::now();
        self.save_node(&parent)
    }

    pub fn resolve_path(&self, path: &str) -> VfsResult<Option<String>> {
        let clean = path.trim();

        // Normalize root path
        if clean == "/" || clean.is_empty() {
            return Ok(Some("root".to_string()));
        }

        let mut current_id = "root".to_string();
        let parts = clean
            .trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty());

        for part in parts {
            let node = self.get_node(&current_id)?;
            match node.children.get(part) {
                Some(id) => current_id = id.clone(),
                None => return Ok(None),
            }
        }
        Ok(Some(current_id))
    }

    pub fn mkdir_p_internal(&self, path: &str) -> VfsResult<String> {
        let clean = path.trim().trim_start_matches('/');
        if clean.is_empty() {
            return Ok("root".to_string());
        }
        let mut current_id = "root".to_string();

        for part in clean.split('/').filter(|s| !s.is_empty()) {
            let node = self.get_node(&current_id)?;
            if let Some(id) = node.children.get(part) {
                current_id = id.clone();
                println!(
                    "Thread {:?} checked folder {} and found {} children",
                    std::thread::current().id(),
                    current_id,
                    node.children.len()
                );
            } else {
                let new_dir = VfsNode::new_folder(part, Some(current_id.clone()));
                self.save_node_internal(&new_dir)?; // Use internal
                self.add_child_internal(&current_id, part, &new_dir.id)?; // Use internal
                current_id = new_dir.id;
                println!(
                    "Thread {:?} checked folder {} and found {} children",
                    std::thread::current().id(),
                    current_id,
                    node.children.len()
                );
            }
        }
        Ok(current_id)
    }
    pub fn mkdir_p(&self, path: &str) -> VfsResult<String> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "Lock poisoned")?;
        self.mkdir_p_internal(path)
    }

    // Fix for Error E0308: mkdir now consumes the String from mkdir_p and returns ()
    pub fn mkdir(&self, path: &str) -> VfsResult<()> {
        // 1. Guard against root or invalid paths
        if path.is_empty() || !path.starts_with('/') || path == "/" || path.ends_with('/') {
            return Err("Invalid path: must be absolute and not end with a slash".into());
        }

        // 2. Check if it already exists
        if self.resolve_path(path)?.is_some() {
            return Err("Directory already exists".into());
        }

        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid name")?;
        let parent_path = path_obj.parent().and_then(|p| p.to_str()).unwrap_or("/");

        // 3. Verify parent exists AND is a folder
        let parent_id = self
            .resolve_path(parent_path)?
            .ok_or("Parent path not found")?;
        let parent_node = self.get_node(&parent_id)?;

        if !matches!(parent_node.node_type, VfsNodeType::VirtualFolder) {
            return Err("Parent is not a directory".into());
        }

        // 4. Create and link
        let new_dir = VfsNode::new_folder(name, Some(parent_id.clone()));
        self.save_node(&new_dir)?;
        self.add_child(&parent_id, name, &new_dir.id)
    }
    pub fn mktext(&self, path: &str, content: &str, lang: &str) -> VfsResult<()> {
        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;
        let parent_id = self.mkdir_p(path_obj.parent().and_then(|p| p.to_str()).unwrap_or("/"))?;

        let node = VfsNode {
            id: VfsNode::generate_id("text", name),
            name: name.to_string(),
            node_type: VfsNodeType::TextDocument {
                document_id: Uuid::new_v4().to_string(),
                chunk_count: 1,
                word_count: content.split_whitespace().count(),
                language: lang.to_string(),
            },
            parent_id: Some(parent_id.clone()),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: None,
        };
        // We store the text as bytes in a blob
        let blob_key = format!("vfs/blob/{}", Uuid::new_v4());
        self.manager.put(&blob_key, content.as_bytes(), None)?;

        self.save_node(&node)?;
        self.add_child(&parent_id, name, &node.id)
    }
    pub fn script(&self, path: &str, code: &str, lang: &str) -> VfsResult<()> {
        let lines = code.lines().count();
        let mut functions = Vec::new();
        if lang == "rust" {
            // Simple parser: look for "fn name"
            for line in code.lines() {
                if let Some(idx) = line.find("fn ") {
                    let rest = &line[idx + 3..];
                    let name = rest
                        .split(|c: char| !c.is_alphanumeric() && c != '_')
                        .next();
                    if let Some(n) = name {
                        if !n.is_empty() {
                            functions.push(n.to_string());
                        }
                    }
                }
            }
        }

        let blob_key = format!("vfs/blob/{}", Uuid::new_v4());
        self.manager.put(&blob_key, code.as_bytes(), None)?;

        let path_obj = std::path::Path::new(path);
        let name = path_obj.file_name().and_then(|s| s.to_str()).unwrap();
        let parent_id = self.mkdir_p(path_obj.parent().and_then(|p| p.to_str()).unwrap_or("/"))?;

        let node = VfsNode {
            id: VfsNode::generate_id("code", name),
            name: name.to_string(),
            node_type: VfsNodeType::CodeDocument {
                blob_key,
                language: lang.to_string(),
                lines,
                functions,
                imports: vec![],
            },
            parent_id: Some(parent_id.clone()),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: None,
        };
        self.save_node(&node)?;
        self.add_child(&parent_id, name, &node.id)
    }
    pub fn mkfile(&self, path: &str, content: &[u8], mime: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "Lock poisoned")?;
        let parent_id = self.mkdir_p_internal(
            std::path::Path::new(path)
                .parent()
                .unwrap()
                .to_str()
                .unwrap(),
        )?;

        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;

        let blob_key = format!("vfs/blob/{}", Uuid::new_v4());
        self.manager.put(&blob_key, content, None)?;

        let node = VfsNode {
            id: VfsNode::generate_id("blob", name),
            name: name.to_string(),
            node_type: VfsNodeType::BlobReference {
                blob_key,
                size: content.len() as u64,
                mime_type: mime.to_string(),
            },
            parent_id: Some(parent_id.clone()),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: None,
        };

        // CRITICAL: Use the internal version that DOES NOT try to lock again!
        self.save_node_internal(&node)?;
        self.add_child_internal(&parent_id, name, &node.id)
    }

    // Fix for Error E0061: Updated mklink, mkjson, and mkcode to supply all 3 arguments to add_child
    pub fn mklink(&self, path: &str, target_id: &str) -> VfsResult<()> {
        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;
        let parent_id = self.mkdir_p(path_obj.parent().and_then(|p| p.to_str()).unwrap_or("/"))?;

        let link_node = VfsNode {
            id: VfsNode::generate_id("link", name),
            name: name.to_string(),
            node_type: VfsNodeType::VirtualFolder, // Or a specific Link type
            parent_id: Some(parent_id.clone()),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: Some(target_id.to_string()),
        };
        self.save_node(&link_node)?;
        self.add_child(&parent_id, name, &link_node.id)
    }

    pub fn mkjson(
        &self,
        path: &str,
        doc_id: &str,
        fp: &str,
        ver: &str,
        size: usize,
    ) -> VfsResult<()> {
        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;
        let parent_id = self.mkdir_p(path_obj.parent().and_then(|p| p.to_str()).unwrap_or("/"))?;

        let json_node = VfsNode {
            id: VfsNode::generate_id("json", name),
            name: name.to_string(),
            node_type: VfsNodeType::JsonDocument {
                document_id: doc_id.to_string(),
                fingerprint: fp.to_string(),
                schema_version: ver.to_string(),
                size_bytes: size,
            },
            parent_id: Some(parent_id.clone()),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: None,
        };
        self.save_node(&json_node)?;
        self.add_child(&parent_id, name, &json_node.id)
    }

    pub fn mkcode(&self, path: &str, key: &str, lang: &str, lines: usize) -> VfsResult<()> {
        let path_obj = std::path::Path::new(path);
        let name = path_obj
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Invalid filename")?;
        let parent_id = self.mkdir_p(path_obj.parent().and_then(|p| p.to_str()).unwrap_or("/"))?;

        let code_node = VfsNode {
            id: VfsNode::generate_id("code", name),
            name: name.to_string(),
            node_type: VfsNodeType::CodeDocument {
                blob_key: key.to_string(),
                language: lang.to_string(),
                lines,
                functions: vec![],
                imports: vec![],
            },
            parent_id: Some(parent_id.clone()),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: HashMap::new(),
            target_id: None,
        };
        self.save_node(&code_node)?;
        self.add_child(&parent_id, name, &code_node.id)
    }

    pub fn rm(&self, path: &str) -> VfsResult<()> {
        if path == "/" || path.is_empty() {
            return Err("Permission denied: cannot remove root directory".into());
        }
        let node_id = self.resolve_path(path)?.ok_or("Path not found")?;
        let node = self.get_node(&node_id)?;

        if !node.children.is_empty() {
            return Err("Directory not empty".into());
        }

        if let Some(ref pid) = node.parent_id {
            let mut parent = self.get_node(pid)?;
            parent.children.remove(&node.name);
            self.save_node(&parent)?;
        }

        if let VfsNodeType::BlobReference { ref blob_key, .. } = node.node_type {
            let _ = self.manager.delete(blob_key);
        }

        self.manager.delete(&Self::get_node_key(&node_id))?;
        Ok(())
    }

    pub fn ls(&self, path: &str) -> VfsResult<Vec<VfsNode>> {
        // Must be 'pub'
        let id = self.resolve_path(path)?.ok_or("Path not found")?;
        let node = self.get_node(&id)?;

        let mut results = Vec::with_capacity(node.children.len());
        for child_id in node.children.values() {
            results.push(self.get_node(child_id)?);
        }
        Ok(results)
    }
}
