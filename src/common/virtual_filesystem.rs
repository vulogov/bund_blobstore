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
    pub manager: Arc<DataDistributionManager>,
}

impl VirtualFilesystem {
    pub fn new(manager: Arc<DataDistributionManager>) -> Self {
        let vfs = Self { manager };
        // Attempt to initialize root.
        // Note: In a production distributed system, you'd handle the lock here
        // or assume the root is provisioned during cluster setup.
        let _ = vfs.ensure_root();
        vfs
    }

    /// Helper to bootstrap the root directory if it's missing
    fn ensure_root(&self) -> VfsResult<()> {
        let key = Self::get_node_key("root");
        if self.manager.get(&key)?.is_none() {
            let root_node = VfsNode::new_folder("/", None);
            // We use the internal save to avoid double-locking if called from a locked context
            self.save_node_internal(&root_node)?;
        }
        Ok(())
    }

    // ============================================================
    // PUBLIC API (Locked)
    // ============================================================

    pub fn mkdir(&self, path: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        self.mkdir_internal(path)
    }

    pub fn mkfile(&self, path: &str, content: &[u8], mime: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        self.mkfile_internal(path, content, mime)
    }

    pub fn mktext(&self, path: &str, content: &str, lang: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        let words = content.split_whitespace().count();
        let node_type = VfsNodeType::TextDocument {
            word_count: words,
            language: lang.to_string(),
            chunk_count: 0,
            document_id: Uuid::new_v4().to_string(),
        };
        self.create_node_at_path_internal(path, node_type, content.as_bytes())
    }

    pub fn mkjson(
        &self,
        path: &str,
        doc_id: &str,
        fp: &str,
        ver: &str,
        size: usize,
    ) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        let node_type = VfsNodeType::JsonDocument {
            document_id: doc_id.to_string(),
            fingerprint: fp.to_string(),
            schema_version: ver.to_string(),
            size_bytes: size,
        };
        self.create_node_at_path_internal(path, node_type, &[])
    }

    pub fn script(&self, path: &str, code: &str, lang: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        let lines = code.lines().count();
        let functions = code
            .lines()
            .filter(|l| l.contains("fn "))
            .map(|l| l.replace("fn ", "").replace("()", "").trim().to_string())
            .collect();

        let node_type = VfsNodeType::CodeDocument {
            lines,
            language: lang.to_string(),
            functions,
            blob_key: Uuid::new_v4().to_string(),
            imports: Vec::new(),
        };
        self.create_node_at_path_internal(path, node_type, code.as_bytes())
    }

    pub fn mklink(&self, path: &str, target_id: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;

        let p = std::path::Path::new(path);
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let parent_path = p.parent().unwrap().to_str().unwrap();

        let parent_id = self
            .resolve_path_internal(parent_path)?
            .ok_or("Parent directory not found")?;

        let mut node = VfsNode::new(
            Uuid::new_v4().to_string(),
            name,
            VfsNodeType::VirtualFolder,
            Some(parent_id.clone()),
        );
        node.target_id = Some(target_id.to_string());

        self.save_node_internal(&node)?;
        self.add_child_internal(&parent_id, node.name.clone(), &node.id)
    }

    pub fn rm(&self, path: &str) -> VfsResult<()> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        if path == "/" {
            return Err("Cannot remove root".into());
        }

        let id = self.resolve_path_internal(path)?.ok_or("Path not found")?;
        let node = self.get_node_internal(&id)?;

        if !node.children.is_empty() {
            return Err("Directory not empty".into());
        }

        let p = std::path::Path::new(path);
        let parent_path = p.parent().unwrap().to_str().unwrap();
        let parent_id = self.resolve_path_internal(parent_path)?.unwrap();

        let mut parent = self.get_node_internal(&parent_id)?;
        parent.children.remove(&node.name);

        self.save_node_internal(&parent)?;
        self.manager.delete(&Self::get_node_key(&id))?;
        Ok(())
    }

    pub fn ls(&self, path: &str) -> VfsResult<Vec<String>> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        let id = self.resolve_path_internal(path)?.ok_or("Not found")?;
        let node = self.get_node_internal(&id)?;
        Ok(node.children.keys().cloned().collect())
    }

    pub fn get_node(&self, id: &str) -> VfsResult<VfsNode> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        self.get_node_internal(id)
    }

    pub fn get_node_by_path(&self, path: &str) -> VfsResult<VfsNode> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        let id = self.resolve_path_internal(path)?.ok_or("Path not found")?;
        self.get_node_internal(&id)
    }

    pub fn resolve_path(&self, path: &str) -> VfsResult<Option<String>> {
        let _lock = self
            .manager
            .global_lock
            .lock()
            .map_err(|_| "VFS Lock Poisoned")?;
        self.resolve_path_internal(path)
    }

    // ============================================================
    // INTERNAL (No Locking)
    // ============================================================

    fn get_node_internal(&self, id: &str) -> VfsResult<VfsNode> {
        let key = Self::get_node_key(id);
        let data = self
            .manager
            .get(&key)?
            .ok_or_else(|| format!("Node {} not found", id))?;
        Ok(serde_json::from_slice(&data).map_err(|e| e.to_string())?)
    }

    fn resolve_path_internal(&self, path: &str) -> VfsResult<Option<String>> {
        // 1. Strict Validation: Absolute path check
        if !path.starts_with('/') {
            return Err("VFS paths must be absolute".into());
        }

        // 2. Strict Validation: Disallow trailing slashes (except for root)
        if path.len() > 1 && path.ends_with('/') {
            return Err("Trailing slashes not allowed in paths".into());
        }

        self.ensure_root()?;

        let clean = path.trim_matches('/');
        if clean.is_empty() {
            return Ok(Some("root".to_string()));
        }

        let mut current_id = "root".to_string();
        for part in clean.split('/') {
            if part.is_empty() {
                continue;
            }

            let node = self.get_node_internal(&current_id)?;
            match node.children.get(part) {
                Some(id) => current_id = id.clone(),
                None => return Ok(None),
            }
        }
        Ok(Some(current_id))
    }

    fn mkfile_internal(&self, path: &str, content: &[u8], mime: &str) -> VfsResult<()> {
        let node_type = VfsNodeType::BlobReference {
            mime_type: mime.to_string(),
            size: content.len() as u64,
            blob_key: Uuid::new_v4().to_string(),
        };
        self.create_node_at_path_internal(path, node_type, content)
    }

    fn mkdir_internal(&self, path: &str) -> VfsResult<()> {
        // Use resolve_path_internal first to catch trailing slash/absolute errors
        if self.resolve_path_internal(path)?.is_some() {
            return Err("Directory already exists".into());
        }

        let p = std::path::Path::new(path);
        let parent_path = p.parent().ok_or("Invalid path")?.to_str().unwrap_or("/");

        // Ensure parent is actually a directory before proceeding
        let parent_id = self
            .resolve_path_internal(parent_path)?
            .ok_or_else(|| format!("Parent directory {} does not exist", parent_path))?;

        let parent_node = self.get_node_internal(&parent_id)?;
        if !matches!(parent_node.node_type, VfsNodeType::VirtualFolder) {
            return Err("Cannot create directory inside a file".into());
        }

        let name = p.file_name().ok_or("Invalid name")?.to_str().unwrap();
        let node = VfsNode::new_folder(name, Some(parent_id.clone()));

        self.save_node_internal(&node)?;
        self.add_child_internal(&parent_id, node.name.clone(), &node.id)
    }

    fn create_node_at_path_internal(
        &self,
        path: &str,
        node_type: VfsNodeType,
        data: &[u8],
    ) -> VfsResult<()> {
        if !path.starts_with('/') {
            return Err("VFS paths must be absolute".into());
        }

        let p = std::path::Path::new(path);
        let parent_path = p
            .parent()
            .ok_or("Invalid path structure")?
            .to_str()
            .unwrap_or("/");
        let name = p
            .file_name()
            .ok_or("Missing filename")?
            .to_str()
            .unwrap()
            .to_string();

        // This will now throw an error if parent_path contains a file segment
        let parent_id = self.mkdir_p_internal(parent_path)?;

        let node = VfsNode::new(
            Uuid::new_v4().to_string(),
            name,
            node_type,
            Some(parent_id.clone()),
        );

        if !data.is_empty() {
            self.manager.put(&format!("blob/{}", node.id), data, None)?;
        }
        self.save_node_internal(&node)?;
        self.add_child_internal(&parent_id, node.name.clone(), &node.id)
    }

    fn mkdir_p_internal(&self, path: &str) -> VfsResult<String> {
        // Note: mkdir_p is usually more lenient, but it still must respect node types
        let clean = path.trim_matches('/');
        if clean.is_empty() {
            return Ok("root".to_string());
        }

        let mut current_id = "root".to_string();
        for part in clean.split('/') {
            if part.is_empty() {
                continue;
            }

            let node = self.get_node_internal(&current_id)?;

            // Validate that we aren't trying to traverse through a file
            if !matches!(node.node_type, VfsNodeType::VirtualFolder) {
                return Err(
                    format!("Path segment '{}' is a file, not a directory", node.name).into(),
                );
            }

            if let Some(next_id) = node.children.get(part) {
                current_id = next_id.clone();
            } else {
                let new_dir = VfsNode::new_folder(part, Some(current_id.clone()));
                self.save_node_internal(&new_dir)?;
                self.add_child_internal(&current_id, part.to_string(), &new_dir.id)?;
                current_id = new_dir.id;
            }
        }
        Ok(current_id)
    }

    fn add_child_internal(&self, parent_id: &str, name: String, child_id: &str) -> VfsResult<()> {
        let mut parent = self.get_node_internal(parent_id)?;
        parent.children.insert(name, child_id.to_string());
        parent.modified_at = Utc::now();
        self.save_node_internal(&parent)
    }

    fn save_node_internal(&self, node: &VfsNode) -> VfsResult<()> {
        let key = Self::get_node_key(&node.id);
        let data = serde_json::to_vec(node).map_err(|e| e.to_string())?;
        self.manager.put(&key, &data, None)?;

        let shard = self
            .manager
            .get_target_shard(&key, None)
            .map_err(|e| e.to_string())?;
        self.manager.sync_shard(&shard).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn get_node_key(id: &str) -> String {
        format!("vfs/node/{}", id)
    }
}
