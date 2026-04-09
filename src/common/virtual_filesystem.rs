//! Virtual Filesystem abstraction layer atop DataDistributionManager

use crate::DataDistributionManager;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub type VfsResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub children: Vec<String>,
    pub target_id: Option<String>,
}

impl VfsNode {
    pub fn new_folder(name: &str, parent_id: Option<String>) -> Self {
        Self {
            id: format!(
                "folder_{}_{}",
                name,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            name: name.to_string(),
            node_type: VfsNodeType::VirtualFolder,
            parent_id,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: Vec::new(),
            target_id: None,
        }
    }

    pub fn new_blob(
        name: &str,
        parent_id: Option<String>,
        blob_key: &str,
        size: u64,
        mime_type: &str,
    ) -> Self {
        Self {
            id: format!(
                "blob_{}_{}",
                name,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            name: name.to_string(),
            node_type: VfsNodeType::BlobReference {
                blob_key: blob_key.to_string(),
                size,
                mime_type: mime_type.to_string(),
            },
            parent_id,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: Vec::new(),
            target_id: None,
        }
    }

    pub fn new_text(
        name: &str,
        parent_id: Option<String>,
        document_id: &str,
        chunk_count: usize,
        word_count: usize,
        language: &str,
    ) -> Self {
        Self {
            id: format!(
                "text_{}_{}",
                name,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            name: format!("{}.txt", name),
            node_type: VfsNodeType::TextDocument {
                document_id: document_id.to_string(),
                chunk_count,
                word_count,
                language: language.to_string(),
            },
            parent_id,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: Vec::new(),
            target_id: None,
        }
    }

    pub fn new_json(
        name: &str,
        parent_id: Option<String>,
        document_id: &str,
        fingerprint: &str,
        schema_version: &str,
        size_bytes: usize,
    ) -> Self {
        Self {
            id: format!(
                "json_{}_{}",
                name,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            name: format!("{}.json", name),
            node_type: VfsNodeType::JsonDocument {
                document_id: document_id.to_string(),
                fingerprint: fingerprint.to_string(),
                schema_version: schema_version.to_string(),
                size_bytes,
            },
            parent_id,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: Vec::new(),
            target_id: None,
        }
    }

    pub fn new_code(
        name: &str,
        parent_id: Option<String>,
        blob_key: &str,
        language: &str,
        lines: usize,
        functions: Vec<String>,
        imports: Vec<String>,
    ) -> Self {
        let extension = match language {
            "rust" => "rs",
            "python" => "py",
            "javascript" => "js",
            "go" => "go",
            _ => "txt",
        };
        Self {
            id: format!(
                "code_{}_{}",
                name,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            name: format!("{}.{}", name, extension),
            node_type: VfsNodeType::CodeDocument {
                blob_key: blob_key.to_string(),
                language: language.to_string(),
                lines,
                functions,
                imports,
            },
            parent_id,
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: Vec::new(),
            target_id: None,
        }
    }
}

pub struct VirtualFilesystem {
    manager: Arc<DataDistributionManager>,
}

impl VirtualFilesystem {
    pub fn new(manager: Arc<DataDistributionManager>) -> Self {
        Self { manager }
    }

    pub fn init_root(&self) -> VfsResult<()> {
        let root_key = "vfs/root";
        if self.manager.get(root_key)?.is_none() {
            let root = VfsNode::new_folder("/", None);
            let data = serde_json::to_vec(&root)?;
            self.manager.put(root_key, &data, None)?;
            self.save_node(&root)?;
        }
        Ok(())
    }

    fn get_root_node(&self) -> VfsResult<VfsNode> {
        self.init_root()?;
        let root_key = "vfs/root";
        let data = self.manager.get(root_key)?.ok_or("Root not found")?;
        let node: VfsNode = serde_json::from_slice(&data)?;
        Ok(node)
    }

    fn save_node(&self, node: &VfsNode) -> VfsResult<()> {
        let key = format!("vfs/node/{}", node.id);
        let data = serde_json::to_vec(node)?;
        Ok(self.manager.put(&key, &data, None)?)
    }

    fn get_node(&self, id: &str) -> VfsResult<VfsNode> {
        let key = format!("vfs/node/{}", id);
        if let Some(data) = self.manager.get(&key)? {
            let node: VfsNode = serde_json::from_slice(&data)?;
            Ok(node)
        } else {
            Err(format!("Node not found: {}", id).into())
        }
    }

    fn add_child(&self, parent_id: &str, child_id: &str) -> VfsResult<()> {
        let mut parent = self.get_node(parent_id)?;
        if !parent.children.contains(&child_id.to_string()) {
            parent.children.push(child_id.to_string());
            parent.modified_at = Utc::now();
            // This save_node call is critical - it must persist the updated parent
            self.save_node(&parent)?;
        }
        Ok(())
    }

    fn remove_child(&self, parent_id: &str, child_id: &str) -> VfsResult<()> {
        let mut parent = self.get_node(parent_id)?;
        parent.children.retain(|c| c != child_id);
        parent.modified_at = Utc::now();
        self.save_node(&parent)?;
        Ok(())
    }

    pub fn resolve_path(&self, path: &str) -> VfsResult<Option<String>> {
        self.init_root()?;

        if path == "/" || path.is_empty() {
            let root = self.get_root_node()?;
            return Ok(Some(root.id));
        }

        let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
        let root = self.get_root_node()?;
        let mut current_id = root.id;

        for part in parts {
            let current_node = self.get_node(&current_id)?;

            match current_node.node_type {
                VfsNodeType::VirtualFolder => {
                    let mut found = None;
                    for child_id in &current_node.children {
                        let child = self.get_node(child_id)?;
                        if child.name == part {
                            found = Some(child_id.clone());
                            break;
                        }
                    }

                    match found {
                        Some(child_id) => {
                            current_id = child_id;
                        }
                        None => return Ok(None),
                    }
                }
                _ => return Ok(None),
            }
        }

        Ok(Some(current_id))
    }

    // ============================================
    // PUBLIC FILESYSTEM OPERATIONS
    // ============================================

    pub fn mkdir(&self, path: &str) -> VfsResult<()> {
        self.init_root()?;

        if path == "/" || path.is_empty() {
            return Ok(());
        }

        // Check if already exists - use resolve_path directly
        if self.resolve_path(path)?.is_some() {
            return Ok(());
        }

        let path_obj = std::path::Path::new(path);
        let dir_name = path_obj
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        if dir_name.is_empty() {
            return Err("Invalid directory name".into());
        }

        let parent_path = path_obj.parent().unwrap_or(std::path::Path::new("/"));
        let parent_path_str = parent_path.to_str().unwrap_or("/");

        // Get parent ID
        let parent_id = if parent_path_str == "/" {
            let root = self.get_root_node()?;
            root.id
        } else {
            // Ensure parent exists
            if self.resolve_path(parent_path_str)?.is_none() {
                self.mkdir(parent_path_str)?;
            }
            match self.resolve_path(parent_path_str)? {
                Some(id) => id,
                None => return Err(format!("Parent path not found: {}", parent_path_str).into()),
            }
        };

        // Create the directory
        let dir_node = VfsNode::new_folder(dir_name, Some(parent_id.clone()));
        self.save_node(&dir_node)?;
        self.add_child(&parent_id, &dir_node.id)?;

        // Verify creation
        debug_assert!(
            self.resolve_path(path)?.is_some(),
            "Directory was not created successfully"
        );

        Ok(())
    }

    pub fn rmdir(&self, path: &str) -> VfsResult<()> {
        let node_id = match self.resolve_path(path)? {
            Some(id) => id,
            None => return Err(format!("Path not found: {}", path).into()),
        };

        let node = self.get_node(&node_id)?;
        match node.node_type {
            VfsNodeType::VirtualFolder => {
                if !node.children.is_empty() {
                    return Err(format!("Directory not empty: {}", path).into());
                }

                if let Some(parent_id) = node.parent_id {
                    self.remove_child(&parent_id, &node_id)?;
                }

                let key = format!("vfs/node/{}", node_id);
                self.manager.delete(&key)?;
                Ok(())
            }
            _ => Err(format!("Not a directory: {}", path).into()),
        }
    }

    pub fn mkfile(&self, path: &str, content: &[u8], mime_type: &str) -> VfsResult<()> {
        self.init_root()?;

        let path_obj = std::path::Path::new(path);
        let file_name = path_obj
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        if file_name.is_empty() {
            return Err("Invalid file name".into());
        }

        let parent_path = path_obj.parent().unwrap_or(std::path::Path::new("/"));
        let parent_path_str = parent_path.to_str().unwrap_or("/");

        // Ensure parent exists
        let parent_id = if parent_path_str == "/" {
            let root = self.get_root_node()?;
            root.id
        } else {
            if self.resolve_path(parent_path_str)?.is_none() {
                self.mkdir(parent_path_str)?;
            }
            self.resolve_path(parent_path_str)?.unwrap()
        };

        // Remove existing if present
        if self.resolve_path(path)?.is_some() {
            self.rm(path)?;
        }

        let blob_key = format!("vfs/blob/{}", uuid::Uuid::new_v4());
        self.manager.put(&blob_key, content, None)?;

        let file_node = VfsNode::new_blob(
            file_name,
            Some(parent_id.clone()),
            &blob_key,
            content.len() as u64,
            mime_type,
        );
        self.save_node(&file_node)?;
        self.add_child(&parent_id, &file_node.id)?;

        Ok(())
    }

    pub fn mktext(&self, path: &str, content: &str, language: &str) -> VfsResult<()> {
        self.init_root()?;

        let path_obj = std::path::Path::new(path);
        let text_name = path_obj
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        if text_name.is_empty() {
            return Err("Invalid text document name".into());
        }

        let parent_path = path_obj.parent().unwrap_or(std::path::Path::new("/"));
        let parent_path_str = parent_path.to_str().unwrap_or("/");

        let parent_id = if parent_path_str == "/" {
            let root = self.get_root_node()?;
            root.id
        } else {
            if self.resolve_path(parent_path_str)?.is_none() {
                self.mkdir(parent_path_str)?;
            }
            self.resolve_path(parent_path_str)?.unwrap()
        };

        let doc_id = uuid::Uuid::new_v4().to_string();
        let chunk_count = self.store_chunked_document(&doc_id, content, language)?;
        let word_count = content
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()))
            .filter(|w| !w.is_empty())
            .count();

        let doc_node = VfsNode::new_text(
            text_name,
            Some(parent_id.clone()),
            &doc_id,
            chunk_count,
            word_count,
            language,
        );
        self.save_node(&doc_node)?;
        self.add_child(&parent_id, &doc_node.id)?;

        Ok(())
    }

    pub fn mkjson(&self, path: &str, json_content: &[u8], schema_version: &str) -> VfsResult<()> {
        self.init_root()?;

        let path_obj = std::path::Path::new(path);
        let json_name = path_obj
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        if json_name.is_empty() {
            return Err("Invalid JSON document name".into());
        }

        let parent_path = path_obj.parent().unwrap_or(std::path::Path::new("/"));
        let parent_path_str = parent_path.to_str().unwrap_or("/");

        let parent_id = if parent_path_str == "/" {
            let root = self.get_root_node()?;
            root.id
        } else {
            if self.resolve_path(parent_path_str)?.is_none() {
                self.mkdir(parent_path_str)?;
            }
            self.resolve_path(parent_path_str)?.unwrap()
        };

        let json_id = uuid::Uuid::new_v4().to_string();
        let fingerprint = self.store_json_document(&json_id, json_content)?;

        let json_node = VfsNode::new_json(
            json_name,
            Some(parent_id.clone()),
            &json_id,
            &fingerprint,
            schema_version,
            json_content.len(),
        );
        self.save_node(&json_node)?;
        self.add_child(&parent_id, &json_node.id)?;

        Ok(())
    }

    pub fn script(&self, path: &str, code: &str, language: &str) -> VfsResult<()> {
        self.init_root()?;

        let path_obj = std::path::Path::new(path);
        let code_name = path_obj
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        if code_name.is_empty() {
            return Err("Invalid code file name".into());
        }

        let parent_path = path_obj.parent().unwrap_or(std::path::Path::new("/"));
        let parent_path_str = parent_path.to_str().unwrap_or("/");

        let parent_id = if parent_path_str == "/" {
            let root = self.get_root_node()?;
            root.id
        } else {
            if self.resolve_path(parent_path_str)?.is_none() {
                self.mkdir(parent_path_str)?;
            }
            self.resolve_path(parent_path_str)?.unwrap()
        };

        let code_id = uuid::Uuid::new_v4().to_string();
        let (lines, functions, imports) = self.index_code(&code_id, code, language)?;
        let blob_key = format!("vfs/code/{}", code_id);
        self.manager.put(&blob_key, code.as_bytes(), None)?;

        let code_node = VfsNode::new_code(
            code_name,
            Some(parent_id.clone()),
            &blob_key,
            language,
            lines,
            functions,
            imports,
        );
        self.save_node(&code_node)?;
        self.add_child(&parent_id, &code_node.id)?;

        Ok(())
    }

    pub fn rm(&self, path: &str) -> VfsResult<()> {
        let node_id = match self.resolve_path(path)? {
            Some(id) => id,
            None => return Err(format!("Path not found: {}", path).into()),
        };

        let node = self.get_node(&node_id)?;

        if let VfsNodeType::VirtualFolder = node.node_type {
            if !node.children.is_empty() {
                return Err(format!(
                    "Directory not empty. Use rmdir for empty directories: {}",
                    path
                )
                .into());
            }
        }

        if let Some(parent_id) = node.parent_id {
            self.remove_child(&parent_id, &node_id)?;
        }

        let key = format!("vfs/node/{}", node_id);
        self.manager.delete(&key)?;
        Ok(())
    }

    pub fn ls(&self, path: &str) -> VfsResult<Vec<VfsNode>> {
        let node_id = match self.resolve_path(path)? {
            Some(id) => id,
            None => return Err(format!("Path not found: {}", path).into()),
        };

        let node = self.get_node(&node_id)?;
        match node.node_type {
            VfsNodeType::VirtualFolder => {
                let mut children = Vec::new();
                for child_id in &node.children {
                    if let Ok(child) = self.get_node(child_id) {
                        children.push(child);
                    }
                }
                Ok(children)
            }
            _ => Err(format!("Not a directory: {}", path).into()),
        }
    }

    pub fn read(&self, path: &str) -> VfsResult<Vec<u8>> {
        let node_id = match self.resolve_path(path)? {
            Some(id) => id,
            None => return Err(format!("Path not found: {}", path).into()),
        };

        let node = self.get_node(&node_id)?;
        match node.node_type {
            VfsNodeType::BlobReference { blob_key, .. } => {
                Ok(self.manager.get(&blob_key)?.unwrap_or_default())
            }
            VfsNodeType::TextDocument { document_id, .. } => {
                Ok(self.get_chunked_document(&document_id)?.unwrap_or_default())
            }
            VfsNodeType::JsonDocument { document_id, .. } => {
                Ok(self.get_json_document(&document_id)?.unwrap_or_default())
            }
            VfsNodeType::CodeDocument { blob_key, .. } => {
                Ok(self.manager.get(&blob_key)?.unwrap_or_default())
            }
            VfsNodeType::VirtualFolder => Err("Cannot read from directory".into()),
        }
    }

    pub fn stat(&self, path: &str) -> VfsResult<VfsNode> {
        let node_id = match self.resolve_path(path)? {
            Some(id) => id,
            None => return Err(format!("Path not found: {}", path).into()),
        };
        Ok(self.get_node(&node_id)?)
    }

    pub fn exists(&self, path: &str) -> VfsResult<bool> {
        self.init_root()?;
        if path == "/" || path.is_empty() {
            return Ok(true);
        }
        // Call resolve_path directly without any caching
        let result = self.resolve_path(path)?;
        Ok(result.is_some())
    }

    pub fn link(&self, target_path: &str, link_path: &str) -> VfsResult<()> {
        self.init_root()?;

        let target_id = match self.resolve_path(target_path)? {
            Some(id) => id,
            None => return Err(format!("Target path not found: {}", target_path).into()),
        };

        let path_obj = std::path::Path::new(link_path);
        let link_name = path_obj
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("");

        if link_name.is_empty() {
            return Err("Invalid link name".into());
        }

        let parent_path = path_obj.parent().unwrap_or(std::path::Path::new("/"));
        let parent_path_str = parent_path.to_str().unwrap_or("/");

        let parent_id = if parent_path_str == "/" {
            let root = self.get_root_node()?;
            root.id
        } else {
            if self.resolve_path(parent_path_str)?.is_none() {
                self.mkdir(parent_path_str)?;
            }
            self.resolve_path(parent_path_str)?.unwrap()
        };

        let link_node = VfsNode {
            id: format!(
                "link_{}_{}",
                link_name,
                Utc::now().timestamp_nanos_opt().unwrap_or(0)
            ),
            name: link_name.to_string(),
            node_type: VfsNodeType::VirtualFolder,
            parent_id: Some(parent_id.clone()),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            metadata: HashMap::new(),
            children: Vec::new(),
            target_id: Some(target_id),
        };

        self.save_node(&link_node)?;
        self.add_child(&parent_id, &link_node.id)?;
        Ok(())
    }

    // ============================================
    // INTERNAL STORAGE HELPERS
    // ============================================

    fn store_chunked_document(
        &self,
        doc_id: &str,
        content: &str,
        _language: &str,
    ) -> VfsResult<usize> {
        let doc_key = format!("vfs/docs/document/{}", doc_id);
        self.manager.put(&doc_key, content.as_bytes(), None)?;

        let chunks: Vec<&str> = content
            .split(|c| c == '.' || c == '!' || c == '?' || c == '\n')
            .filter(|c| !c.is_empty())
            .collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_key = format!("vfs/docs/chunk/{}/{}", doc_id, i);
            self.manager.put(&chunk_key, chunk.as_bytes(), None)?;
        }

        Ok(chunks.len())
    }

    fn get_chunked_document(&self, doc_id: &str) -> VfsResult<Option<Vec<u8>>> {
        let doc_key = format!("vfs/docs/document/{}", doc_id);
        Ok(self.manager.get(&doc_key)?)
    }

    fn store_json_document(&self, doc_id: &str, json_data: &[u8]) -> VfsResult<String> {
        let key = format!("vfs/json/document/{}", doc_id);
        self.manager.put(&key, json_data, None)?;

        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        json_data.hash(&mut hasher);
        let fingerprint = format!("{:x}", hasher.finish());

        let fingerprint_key = format!("vfs/json/fingerprint/{}", fingerprint);
        self.manager.put(&fingerprint_key, json_data, None)?;

        Ok(fingerprint)
    }

    fn get_json_document(&self, doc_id: &str) -> VfsResult<Option<Vec<u8>>> {
        let key = format!("vfs/json/document/{}", doc_id);
        Ok(self.manager.get(&key)?)
    }

    fn index_code(
        &self,
        code_id: &str,
        code: &str,
        language: &str,
    ) -> VfsResult<(usize, Vec<String>, Vec<String>)> {
        let code_key = format!("vfs/code/raw/{}", code_id);
        self.manager.put(&code_key, code.as_bytes(), None)?;

        let lines: Vec<&str> = code.lines().collect();
        let mut functions = Vec::new();
        let mut imports = Vec::new();

        for line in &lines {
            let line = line.trim();
            match language {
                "rust" => {
                    if line.starts_with("fn ") && line.contains('(') {
                        if let Some(fn_name) = line.split_whitespace().nth(1) {
                            functions.push(fn_name.split('(').next().unwrap_or("").to_string());
                        }
                    }
                    if line.starts_with("use ") || line.starts_with("pub use ") {
                        imports.push(line.to_string());
                    }
                }
                "python" => {
                    if line.starts_with("def ") && line.contains('(') {
                        if let Some(fn_name) = line.split_whitespace().nth(1) {
                            functions.push(fn_name.split('(').next().unwrap_or("").to_string());
                        }
                    }
                    if line.starts_with("import ") || line.starts_with("from ") {
                        imports.push(line.to_string());
                    }
                }
                "javascript" => {
                    if line.contains("function ") || line.contains("=>") {
                        if let Some(fn_name) = line.split("function").nth(1) {
                            functions
                                .push(fn_name.split('(').next().unwrap_or("").trim().to_string());
                        }
                    }
                    if line.starts_with("import ") || line.starts_with("require(") {
                        imports.push(line.to_string());
                    }
                }
                _ => {}
            }
        }

        Ok((lines.len(), functions, imports))
    }
}
