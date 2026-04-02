use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

// Define table structures
const BLOBS: TableDefinition<&str, &[u8]> = TableDefinition::new("blobs");
const METADATA: TableDefinition<&str, &[u8]> = TableDefinition::new("metadata");

/// Metadata for each stored blob
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BlobMetadata {
    pub key: String,
    pub size: usize,
    pub created_at: u64,
    pub modified_at: u64,
    pub checksum: u64, // Simple XOR checksum for integrity
    pub prefix: Option<String>,
}

impl BlobMetadata {
    fn new(key: String, data: &[u8], prefix: Option<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64; // Use milliseconds instead of seconds

        BlobMetadata {
            key,
            size: data.len(),
            created_at: now,
            modified_at: now,
            checksum: calculate_checksum(data),
            prefix,
        }
    }

    fn update_modified(&mut self, data: &[u8]) {
        self.modified_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64; // Use milliseconds instead of seconds
        self.size = data.len();
        self.checksum = calculate_checksum(data);
    }
}

/// Calculate simple XOR checksum for integrity checking
fn calculate_checksum(data: &[u8]) -> u64 {
    data.iter().fold(0u64, |acc, &b| acc ^ (b as u64))
}

/// Query options for listing blobs
#[derive(Debug, Clone)]
pub struct QueryOptions {
    pub prefix: Option<String>,
    pub pattern: Option<String>, // Simple wildcard pattern with *
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            prefix: None,
            pattern: None,
            limit: None,
            offset: None,
        }
    }
}

/// Wrapper struct to manage the database operations
pub struct BlobStore {
    db: Database,
}

impl BlobStore {
    /// Create or open a new blob store at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, redb::Error> {
        let db = Database::create(path)?;

        // Ensure tables exist by opening them in a write transaction
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(BLOBS)?;
            let _ = write_txn.open_table(METADATA)?;
        }
        write_txn.commit()?;

        Ok(BlobStore { db })
    }

    // ============ Core Operations ============

    /// Store a blob with a given key and optional prefix
    pub fn put(&mut self, key: &str, data: &[u8], prefix: Option<&str>) -> Result<(), redb::Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BLOBS)?;
            table.insert(key, data)?;

            let mut metadata_table = write_txn.open_table(METADATA)?;
            let metadata = BlobMetadata::new(key.to_string(), data, prefix.map(String::from));
            let serialized = bincode::serialize(&metadata)
                .map_err(|e| redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            metadata_table.insert(key, serialized.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Update an existing blob (preserves creation time)
    pub fn update(
        &mut self,
        key: &str,
        data: &[u8],
        prefix: Option<&str>,
    ) -> Result<(), redb::Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BLOBS)?;
            let mut metadata_table = write_txn.open_table(METADATA)?;

            let mut metadata = if let Some(serialized) = metadata_table.get(key)? {
                let mut meta: BlobMetadata =
                    bincode::deserialize(serialized.value()).map_err(|e| {
                        redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;
                meta.update_modified(data);
                meta
            } else {
                BlobMetadata::new(key.to_string(), data, prefix.map(String::from))
            };

            // Update prefix if provided
            if let Some(p) = prefix {
                metadata.prefix = Some(p.to_string());
            }

            table.insert(key, data)?;
            let serialized = bincode::serialize(&metadata)
                .map_err(|e| redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
            metadata_table.insert(key, serialized.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Retrieve a blob by key
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;

        match table.get(key)? {
            Some(value) => Ok(Some(value.value().to_vec())),
            None => Ok(None),
        }
    }

    /// Retrieve blob with metadata
    pub fn get_with_metadata(
        &self,
        key: &str,
    ) -> Result<Option<(Vec<u8>, BlobMetadata)>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;
        let metadata_table = read_txn.open_table(METADATA)?;

        match (table.get(key)?, metadata_table.get(key)?) {
            (Some(data), Some(meta)) => {
                let metadata: BlobMetadata = bincode::deserialize(meta.value()).map_err(|e| {
                    redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?;
                Ok(Some((data.value().to_vec(), metadata)))
            }
            _ => Ok(None),
        }
    }

    /// Remove blob by key
    pub fn remove(&mut self, key: &str) -> Result<bool, redb::Error> {
        let write_txn = self.db.begin_write()?;
        let existed = {
            let mut table = write_txn.open_table(BLOBS)?;
            let mut metadata_table = write_txn.open_table(METADATA)?;

            let existed = table.remove(key)?.is_some();
            metadata_table.remove(key)?;
            existed
        }; // table and metadata_table are dropped here

        write_txn.commit()?;
        Ok(existed)
    }

    /// Delete a blob by key (alias for remove)
    pub fn delete(&mut self, key: &str) -> Result<bool, redb::Error> {
        self.remove(key)
    }

    /// Check if a key exists
    pub fn exists(&self, key: &str) -> Result<bool, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;
        Ok(table.get(key)?.is_some())
    }

    /// Get metadata for a blob
    pub fn get_metadata(&self, key: &str) -> Result<Option<BlobMetadata>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let metadata_table = read_txn.open_table(METADATA)?;

        match metadata_table.get(key)? {
            Some(data) => {
                let metadata: BlobMetadata = bincode::deserialize(data.value()).map_err(|e| {
                    redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?;
                Ok(Some(metadata))
            }
            None => Ok(None),
        }
    }

    /// Verify blob integrity using checksum
    pub fn verify_integrity(&self, key: &str) -> Result<bool, redb::Error> {
        if let Some((data, metadata)) = self.get_with_metadata(key)? {
            let checksum = calculate_checksum(&data);
            Ok(checksum == metadata.checksum)
        } else {
            Ok(false)
        }
    }

    // ============ Query Operations ============

    /// List all keys (without metadata)
    pub fn list_keys(&self) -> Result<Vec<String>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;

        let mut keys = Vec::new();
        for result in table.iter()? {
            let (key, _): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            keys.push(key.value().to_string());
        }
        Ok(keys)
    }

    /// Query blobs by prefix
    pub fn query_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, BlobMetadata)>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let metadata_table = read_txn.open_table(METADATA)?;

        let mut results = Vec::new();
        for result in metadata_table.iter()? {
            let (key, data): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            let key_str = key.value();

            if key_str.starts_with(prefix) {
                let metadata: BlobMetadata = bincode::deserialize(data.value()).map_err(|e| {
                    redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?;
                results.push((key_str.to_string(), metadata));
            }
        }

        Ok(results)
    }

    /// Query blobs by metadata prefix (efficient for grouped keys)
    pub fn query_by_metadata_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, BlobMetadata)>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let metadata_table = read_txn.open_table(METADATA)?;

        let mut results = Vec::new();
        for result in metadata_table.iter()? {
            let (key, data): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            let metadata: BlobMetadata = bincode::deserialize(data.value())
                .map_err(|e| redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            if let Some(blob_prefix) = &metadata.prefix {
                if blob_prefix == prefix {
                    results.push((key.value().to_string(), metadata));
                }
            }
        }

        Ok(results)
    }

    /// Advanced query with multiple options (prefix, pattern, limit, offset)
    pub fn query(&self, options: QueryOptions) -> Result<Vec<(String, BlobMetadata)>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let metadata_table = read_txn.open_table(METADATA)?;

        let mut results = Vec::new();

        // Collect all metadata
        for result in metadata_table.iter()? {
            let (key, data): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            let key_str = key.value();
            let metadata: BlobMetadata = bincode::deserialize(data.value())
                .map_err(|e| redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

            // Apply filters
            let mut matches = true;

            if let Some(ref prefix) = options.prefix {
                if !key_str.starts_with(prefix) {
                    matches = false;
                }
            }

            if let Some(ref pattern) = options.pattern {
                if !matches_pattern(key_str, pattern) {
                    matches = false;
                }
            }

            if matches {
                results.push((key_str.to_string(), metadata));
            }
        }

        // Apply offset and limit
        let start = options.offset.unwrap_or(0);
        let end = if let Some(limit) = options.limit {
            (start + limit).min(results.len())
        } else {
            results.len()
        };

        Ok(results[start..end].to_vec())
    }

    /// Get all blobs with their metadata
    pub fn get_all_with_metadata(
        &self,
    ) -> Result<HashMap<String, (Vec<u8>, BlobMetadata)>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;
        let metadata_table = read_txn.open_table(METADATA)?;

        let mut map = HashMap::new();
        for result in table.iter()? {
            let (key, data): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            let key_str = key.value().to_string();

            if let Some(meta_data) = metadata_table.get(key.value())? {
                let metadata: BlobMetadata =
                    bincode::deserialize(meta_data.value()).map_err(|e| {
                        redb::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;
                map.insert(key_str, (data.value().to_vec(), metadata));
            }
        }

        Ok(map)
    }

    /// Get all blobs as a HashMap (without metadata)
    pub fn get_all(&self) -> Result<HashMap<String, Vec<u8>>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;

        let mut map = HashMap::new();
        for result in table.iter()? {
            let (key, value): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            map.insert(key.value().to_string(), value.value().to_vec());
        }
        Ok(map)
    }

    // ============ Utility Operations ============

    /// Get the number of blobs in the store
    pub fn len(&self) -> Result<usize, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;
        Ok(table.len()? as usize)
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> Result<bool, redb::Error> {
        Ok(self.len()? == 0)
    }

    /// Clear all blobs from the store
    pub fn clear(&mut self) -> Result<(), redb::Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BLOBS)?;
            let mut metadata_table = write_txn.open_table(METADATA)?;

            // Collect all keys first
            let keys: Vec<String> = self.list_keys()?;
            for key in keys {
                table.remove(key.as_str())?;
                metadata_table.remove(key.as_str())?;
            }
        }
        write_txn.commit()?;
        Ok(())
    }
}

/// Simple pattern matching with * wildcard
fn matches_pattern(s: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        let mut start = 0;

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }

            if i == 0 {
                // First part must be at the beginning
                if !s.starts_with(part) {
                    return false;
                }
                start = part.len();
            } else if i == parts.len() - 1 {
                // Last part must be at the end
                return s[start..].ends_with(part);
            } else {
                // Middle parts must appear in order
                if let Some(pos) = s[start..].find(part) {
                    start += pos + part.len();
                } else {
                    return false;
                }
            }
        }
        true
    } else {
        s == pattern
    }
}
