use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use std::collections::HashMap;

// Define the table structure
const BLOBS: TableDefinition<&str, &[u8]> = TableDefinition::new("blobs");

// Wrapper struct to manage the database operations
pub struct BlobStore {
    db: Database,
}

impl BlobStore {
    /// Create or open a new blob store at the given path
    pub fn open(path: &str) -> Result<Self, redb::Error> {
        let db = Database::create(path)?;
        Ok(BlobStore { db })
    }

    /// Store a blob with a given key
    pub fn put(&mut self, key: &str, data: &[u8]) -> Result<(), redb::Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BLOBS)?;
            table.insert(key, data)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Retrieve a blob by key
    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;

        match table.get(key)? {
            Some(value) => {
                let bytes: &[u8] = value.value();
                Ok(Some(bytes.to_vec()))
            }
            None => Ok(None),
        }
    }

    /// Delete a blob by key
    pub fn delete(&mut self, key: &str) -> Result<(), redb::Error> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(BLOBS)?;
            table.remove(key)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Check if a key exists
    pub fn exists(&self, key: &str) -> Result<bool, redb::Error> {
        Ok(self.get(key)?.is_some())
    }

    /// List all keys in the store
    pub fn list_keys(&self) -> Result<Vec<String>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;

        let mut keys = Vec::new();
        for result in table.iter()? {
            let (key, _): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            keys.push(String::from_utf8_lossy(key.value().as_bytes()).to_string());
        }

        Ok(keys)
    }

    /// Get all blobs as a HashMap
    pub fn get_all(&self) -> Result<HashMap<String, Vec<u8>>, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;

        let mut map = HashMap::new();
        for result in table.iter()? {
            let (key, value): (redb::AccessGuard<&str>, redb::AccessGuard<&[u8]>) = result?;
            map.insert(
                String::from_utf8_lossy(key.value().as_bytes()).to_string(),
                value.value().to_vec(),
            );
        }

        Ok(map)
    }

    /// Get the number of blobs in the store
    pub fn len(&self) -> Result<usize, redb::Error> {
        let read_txn = self.db.begin_read()?;
        let table = read_txn.open_table(BLOBS)?;
        let count = table.len()?;
        Ok(count as usize)
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> Result<bool, redb::Error> {
        Ok(self.len()? == 0)
    }
}
